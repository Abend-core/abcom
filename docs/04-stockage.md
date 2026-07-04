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

Les évolutions de schéma se font par `ALTER TABLE` tolérant (l'erreur « duplicate column » est ignorée sur les bases déjà à jour) — pas de système de migrations versionnées à ce stade.

### Chargement et pagination

Au démarrage, seule une fenêtre récente est chargée en mémoire (500 messages, `INITIAL_WINDOW`). Le fil en affiche une centaine ; remonter dans l'historique déclenche des requêtes keyset (`WHERE id < :oldest ORDER BY id DESC LIMIT 100`) qui étendent la fenêtre. L'historique complet reste en base sans limite de taille — l'ancien plafond arbitraire de 500 messages a disparu avec les fichiers JSON.

### Migration depuis les fichiers JSON

Si `abcom.db` n'existe pas au démarrage et que d'anciens fichiers JSON sont présents (`messages.json`, `reactions.json`, `read_counts.json`, `groups.json`, `peer_records.json`, `peer_avatars.json`), leur contenu est importé puis les fichiers sont renommés en `.bak`. La migration a été vérifiée sur un historique réel de 401 messages. Les hashes de messages sont conservés tels quels (ce sont les identifiants réseau).

## Préférences (table `kv`)

| Clé | Valeurs | Défaut |
|---|---|---|
| `notif_preview` | `1` aperçu du message / `0` discret | `1` |
| `autostart` | `1` / `0` | `1`, posé au premier lancement d'un build release |
| thème, langue, sourdines par conversation | — | selon réglages |

## Le dossier `media/`

Chaque fichier ou image transféré est stocké sous un identifiant unique dans `media/`. Pour que l'empreinte disque reste bornée :

- au démarrage, un thread détaché supprime les **orphelins** (fichiers que plus aucun message ne référence) ;
- puis applique un **plafond de 2 Go** en supprimant les fichiers les plus anciens (mtime) au-delà.

Les GIF Klipy ne sont jamais stockés : ils voyagent par URL et chaque pair les charge depuis le CDN.
