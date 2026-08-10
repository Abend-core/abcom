# 04 — Stockage local

## Répertoire de données

Toutes les données vivent dans le répertoire de données de la plateforme ([config.rs](../src/config.rs), via `dirs::data_dir()`) :

| Plateforme | Chemin |
|---|---|
| Linux | `~/.local/share/abcom/` |
| macOS | `~/Library/Application Support/abcom/` |
| Windows | `%APPDATA%\abcom\` |

En mode multi-instance (`ABCOM_INSTANCE=N`), chaque instance utilise `abcom-N/` pour ne pas partager ses données.

Contenu :

```
abcom/
├── abcom.db          # base SQLite : messages, groupes, pairs, préférences…
├── abcom.db-wal/-shm # fichiers du mode WAL
├── identity.key      # paire X25519 de la machine (0600)
├── media/            # fichiers et images reçus ou envoyés
└── *.json.bak        # anciens fichiers JSON, conservés après migration
```

## La base SQLite

Un seul fichier, `abcom.db`, ouvert en mode WAL avec `synchronous=NORMAL` (bon compromis durabilité/latence pour un chat local). Le crate est `rusqlite` en feature `bundled` : SQLite est embarqué dans le binaire, aucune dépendance système.

**Le thread UI ne touche jamais le disque.** Toutes les écritures passent par un canal vers un thread de stockage dédié ([app/storage.rs](../src/app/storage.rs), commandes `StorageCmd`). L'ajout d'un message coûte un `INSERT` hors du thread de rendu ; un flush final est effectué à la fermeture de l'application.

**Une écriture qui échoue est remontée.** Le `Flush` ne se contente pas d'accuser le passage : il rend la dernière erreur d'écriture survenue depuis le flush précédent. Un disque plein ou une base en lecture seule est donc journalisé à la fermeture (`historique non sauvegardé`) au lieu de laisser l'utilisateur quitter en croyant ses messages enregistrés.

**Un message n'est acquitté qu'une fois commité.** Après un lot d'insertions réussi, le thread de stockage émet `MessagesPersisted` ; c'est ce signal qui libère les accusés de réception retenus par l'interface. Une écriture qui échoue ne l'émet jamais, donc l'expéditeur ne reçoit rien et réémet — plutôt que de croire son message livré alors qu'il a disparu.

### Schéma

```sql
messages (id INTEGER PRIMARY KEY, hash, from_user, to_user, content,
          timestamp, ts_epoch, media, reply_to, nonce)
  -- to_user : NULL = fil « Tous », "bob" = privé, "#equipe" = salon
  -- hash    : identifiant réseau du message (FNV-1a), indexé
  -- media   : descripteur JSON du média attaché, le cas échéant
  -- reply_to: hash du message cité

reactions   (message_hash, emoji, username)      -- une ligne par réaction
read_counts (username PRIMARY KEY, count)        -- position de lecture par conversation
groups      (name PRIMARY KEY, data)             -- un groupe = son JSON complet
peers       (username PRIMARY KEY, alias, avatar BLOB, pubkey BLOB)
  -- avatar : PNG 256×256 ; pubkey : clé épinglée TOFU
kv          (k PRIMARY KEY, v)                   -- préférences
```

Les évolutions de schéma utilisent `PRAGMA user_version` et des migrations transactionnelles. Une migration échouée laisse la version et le schéma précédents intacts.

| Version | Contenu |
|---|---|
| 1 | Colonne `nonce` sur `messages` |
| 2 | Salons désignés par un identifiant immuable : `to_user` passe de `#<nom>` à `#<id>`, les hashs de messages concernés sont recalculés et `reactions`, `receipts` et `read_marks` sont reportés dessus |

### Chargement et pagination

Au démarrage, seule une fenêtre récente est chargée en mémoire (500 messages, `INITIAL_WINDOW`). Le fil en affiche une centaine ; remonter dans l'historique déclenche des requêtes keyset (`WHERE id < :oldest ORDER BY id DESC LIMIT 100`) qui étendent la fenêtre. L'historique complet reste en base sans limite de taille — l'ancien plafond arbitraire de 500 messages a disparu avec les fichiers JSON.

La fenêtre mémoire est elle-même bornée : au-delà, les plus anciens messages sont drainés et leurs rowids deviennent inconnus. Ce cas est distinct de « tout l'historique est déjà chargé » — les confondre arrêtait définitivement le chargement vers le haut, au moment précis où il restait le plus à charger. Le curseur perdu se redérive alors du hash du plus ancien message encore en mémoire (`HistoryCursor::BeforeHash`).

### Migration depuis les fichiers JSON

Si `abcom.db` n'existe pas au démarrage et que d'anciens fichiers JSON sont présents (`messages.json`, `reactions.json`, `read_counts.json`, `groups.json`, `peer_records.json`, `peer_avatars.json`), leur contenu est importé transactionnellement. Seuls les fichiers importés avec succès sont renommés en `.bak` ; une erreur conserve les sources actives et annule les écritures partielles.

## Préférences (table `kv`)

| Clé | Valeurs | Défaut |
|---|---|---|
| `notif_preview` | `1` aperçu du message / `0` discret | `1` |
| `autostart` | `1` / `0` | `1`, posé au premier lancement d'un build release |
| thème, langue, sourdines par conversation | — | selon réglages |

## Le dossier `media/`

Chaque fichier ou image transféré est stocké sous un identifiant unique dans `media/`. Pour que l'empreinte disque reste bornée :

- un thread détaché supprime les **orphelins** (fichiers que plus aucun message ne référence) ;
- puis applique un **plafond de 2 Go** en supprimant les fichiers les plus anciens (mtime) au-delà.

Ce nettoyage tourne au démarrage **et toutes les 15 minutes**. Le limiter au démarrage laissait une longue session dépasser durablement le plafond. Il passe par le thread de stockage : la liste des médias encore référencés est une requête SQL, et l'historique en mémoire n'en est qu'une fenêtre — s'en servir supprimerait les fichiers des messages plus anciens.

Un pair ne peut écrire sans confirmation que jusqu'au seuil d'acceptation (50 Mo, cf. [05](05-fonctionnalites.md)), ce qui borne aussi ce que le cache peut absorber entre deux passages.

Les GIF Klipy ne sont jamais stockés : ils voyagent par URL et chaque pair les charge depuis le CDN — dont l'hôte est restreint par une liste blanche, cf. [03](03-reseau-et-securite.md).

## Tables ajoutées le 8 août 2026

| Table | Contenu | Cycle de vie |
|---|---|---|
| `receipts` | Accusés nominatifs livré/lu (`message_hash`, `username`, `kind`) | Insertion idempotente. Effacer une conversation emporte immédiatement les accusés de ses messages — les laisser derrière les rendait orphelins jusqu'au prochain démarrage, où seule une purge de rattrapage les ramassait. Idem pour `reactions` et `read_marks` |
| `outbox` | Messages en attente d'un destinataire hors ligne (`hash`, `to_peer`, `message`) | Vidée pair par pair à leur reconnexion |

**Maintenance.** Paramètres → Général → Données propose la compaction
(`VACUUM` + `ANALYZE`) — la base ne rend jamais seule l'espace des
conversations effacées — et l'export texte de la conversation courante. Les
sauvegardes `*.json.bak` de la migration JSON → SQLite sont supprimées au-delà
de 30 jours.
