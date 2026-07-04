> [🏠 Accueil](../README.md) > [🔁🔐 Seconde passe & sécurisation](08-seconde-passe-et-securisation.md)

> 📅 **Généré le** : 2026-07-04 — après l'implémentation des phases A/B/C du [plan d'optimisation](07-plan-optimisation.md)
> 🔖 **Base auditée** : code post-optimisation (réveil par événement, caches dérivés, fenêtrage, éviction textures, GC média, persistance débouncée)
> 🔄 **À régénérer si** : migration SQLite réalisée, passage aux connexions persistantes, implémentation du chiffrement

# Seconde passe d'audit & plan de sécurisation du transport

Ce document fait trois choses :
1. **§2 — le reste à faire** : tout ce qui a été détecté dans les audits
   précédents ([06](06-audit-performance.md), [07](07-plan-optimisation.md))
   et qui n'est **pas encore implémenté** ;
2. **§3 — les nouvelles détections** de cette seconde passe (dont des effets
   secondaires des optimisations de la passe 1) ;
3. **§4 — le plan de chiffrement du transport** : sécuriser les données
   pendant le transfert entre pairs (messages, réactions, accusés, médias).

---

## 1. État après la passe 1 (rappel des mesures)

| Axe | Avant | Après passe 1 |
|---|---|---|
| CPU au repos | 22 % | ~0,6 % (mesuré **sans pair connecté**, cf. N1) |
| GPU | 9,7 % | à re-mesurer (attendu ~0) |
| RSS au repos | 443 Mo | ~156 Mo |
| Threads | 15 | 8 |
| Binaire release | — | 10,6 Mo (strip + LTO) |
| Écriture disque / message | historique complet, thread UI | aucune (débounce 2 s) |
| Disque `media/` | illimité | orphelins purgés + plafond 2 Go |

---

## 2. Déjà détecté, pas encore fait

Par ordre d'impact décroissant. Les références renvoient aux constats de
[06-audit-performance.md](06-audit-performance.md).

| # | Sujet | Référence | Impact | Effort |
|---|---|---|---|---|
| R1 | **Migration SQLite** : remplace les 6 fichiers JSON, supprime le ring-buffer 500, avatars en BLOB (fin du JSON ~3,7×), GC média transactionnel, pagination sur l'historique complet. Le thread de persistance débouncée créé en passe 1 est son point d'accueil. Schéma prêt (06 §7). | P0-1 (définitif), P1-6, P1-7, §7, §9.4 | Robustesse + disque + historique complet | L |
| R2 | **Connexion TCP persistante par pair** (framing longueur-préfixée, une connexion au lieu d'une par paquet). Décision n°5 : pas de compat à préserver. **Prérequis du chiffrement §4** — les deux se font ensemble. | P2-3 | CPU/syscalls + latence + porte d'entrée du chiffrement | L |
| R3 | **Bug avatar > 64 Ko** (constaté en vérification passe 1) : les annonces d'avatar dépassent `MAX_PACKET_SIZE` (`server.rs`, 64 Ko) et sont silencieusement rejetées — un PNG 256×256 sérialisé en tableau JSON dépasse la limite. Résolu naturellement par R2 (framing sans plafond arbitraire) + R1 (BLOB). | découverte passe 1 | Fonctionnel (avatars jamais reçus) | inclus R1/R2 |
| R4 | **Atlas emoji** : 323 PNG décodés synchroneusement au premier frame (~gel perceptible) et 323 textures GPU individuelles (~7 Mo + bindings). Une seule texture atlas + UVs. | P2-1 | Démarrage + RAM + GPU | M |
| R5 | **`resvg` optionnel** : prévu en A1, non fait. La dépendance (lourde) ne sert qu'à l'import d'avatar SVG. Feature Cargo `avatar-svg` désactivée par défaut, ou suppression du support. | P1-4 | Binaire + temps de compilation | S |
| R6 | **Fusion des 7 tâches `run_sender_*`** en un canal unique `(SocketAddr, NetworkPacket)`. Absorbé par R2 de toute façon. | P1-4 | Simplification | S |
| R7 | **Phase E** : test renderer `wgpu`/Metal vs Glow (`powermetrics`), mode barre de menus (fenêtre fermée = zéro rendu — le plus gros levier pour une app toujours ouverte), notifications système natives (supprime rodio/le thread audio). | 06 §10 A/B/C | GPU/CPU en arrière-plan | M–L |
| R8 | Micro restants : hover-toolbar (clone des emojis récents + recherche linéaire des textures par frame de survol), `restore_peers_from_history` (clone de `my_username` par message, démarrage uniquement). | P2-4 | Marginal | S |

---

## 3. Nouvelles détections (seconde passe)

### N1 · La découverte réveille l'UI toutes les 3 s par pair connecté ⚠️

- **Où** : `src/discovery.rs:119-123` — `PeerDiscovered` est émis **à chaque
  annonce reçue** (toutes les 3 s par pair), et depuis la passe 1 chaque
  événement relayé déclenche un `request_repaint` (`src/notify.rs`).
- **Impact** : le « ~0,6 % CPU au repos » a été mesuré **sans pair**. Avec
  N pairs en ligne, l'UI se réveille N fois toutes les 3 s — on retombe sur
  un repaint quasi périodique (léger, mais contraire à l'objectif zéro-réveil).
- **Correction** : la tâche discovery connaît déjà l'état des pairs
  (`peer_timestamps`) et émet déjà `PeerDisconnected`. N'émettre
  `PeerDiscovered` que sur **changement** (nouveau pair, adresse changée,
  retour après déconnexion). La fraîcheur (`last_seen`) devient interne à la
  tâche discovery ; `AppState::cleanup_inactive_peers` (10 s, thread UI)
  disparaît au profit des seuls événements `PeerDisconnected` — ce qui
  allonge aussi le fallback de repaint (plus besoin du tick 5 s pour ça).
- **Effort** : S. **Vérification** : deux instances au repos → zéro repaint
  hors réception de message (log de frames temporaire).

### N2 · La frappe d'un pair invalide le cache du fil ~1×/s

- **Où** : `set_user_typing`/`clear_typing_if_old` font `bump_generation()`
  (`src/app/typing.rs`), or `ChatCache::refresh` se reconstruit sur **toute**
  génération (`src/ui/snapshot.rs`) : pendant qu'un pair tape, les lignes du
  fil (jusqu'à 500 messages clonés + réactions copiées) sont reconstruites à
  chaque battement de frappe, alors que seul l'indicateur « écrit… » change.
  Le markdown, lui, reste memoïsé (pas de re-parse).
- **Correction** : scinder le compteur en `content_generation` (messages,
  réactions, accusés, avatars, alias) et `presence_generation` (pairs
  online/offline, frappe). `ChatCache` ne dépend que du premier,
  `SidebarCache` des deux.
- **Effort** : S.

### N3 · Picker GIF ouvert : clone du feed par frame + repaint forcé 300 ms

- **Où** : `src/ui/gif_picker.rs:92-94` — `st.items.clone()` (24–72 items ×
  3 Strings) + `status.clone()` à chaque frame tant que le picker est
  affiché ; `:326` — `request_repaint_after(300 ms)` inconditionnel tant que
  le picker est ouvert (l'anti-rebond de la recherche ne devrait demander un
  repaint que lorsqu'une requête est **en attente** de déclenchement).
- **Correction** : exposer le feed en `Arc<Vec<GifItem>>` (clone d'Arc) ;
  conditionner le repaint 300 ms à `gif_query` non vide **et** différente de
  la dernière requête envoyée. Noter que les aperçus animés visibles imposent
  de toute façon un repaint continu pendant l'ouverture — le gain porte sur
  les allocations, pas sur la cadence.
- **Effort** : S.

### N4 · Divers mineurs

- `src/ui/group_modal.rs:14` : `peers.clone()` + verrou par frame tant que la
  modale de création de groupe est ouverte (rare, borné — à brancher sur
  `sidebar_cache` par cohérence).
- `src/ui/emoji_picker.rs:164` : la liste des shortcodes filtrés est
  reconstruite (avec clones) à chaque frame quand le menu `:alias` est
  ouvert ; ne la recalculer que lorsque la requête change.
- `AppState::message_hash` recalculé pour chaque message à chaque
  reconstruction du `ChatCache` (rebuild seulement sur changement, donc coût
  borné) — mémoïsable en stockant le hash sur `ChatMessage` à l'insertion
  (champ `#[serde(skip)]`), utile aussi pour R1.
- **Fenêtrage** (introduit en passe 1) : en tête de fenêtre tronquée, le
  séparateur de date n'est pas affiché si la coupure tombe en milieu de
  journée (cosmétique) ; la compensation d'offset au chargement de 100
  messages supplémentaires mérite une passe de QA manuelle (interaction avec
  `stick_to_bottom` en cas de message entrant simultané).
- **Mesures à refaire** (le protocole de 06 §6 reste la référence) : GPU au
  repos, RSS après navigation GIF intensive picker fermé (attendu : retour
  proche de la baseline grâce à `forget_image`), débit d'un transfert > 1 Go
  (attendu : nettement supérieur depuis le throttle de progression).

---

## 4. Plan de chiffrement du transport

### 4.1 Objectif et modèle de menace

Aujourd'hui **tout circule en clair** sur le LAN : messages TCP (JSON),
médias streamés (port chat+1), annonces UDP de découverte. N'importe quelle
machine du réseau peut :
- **lire** les conversations et fichiers (sniffing passif) ;
- **usurper** un nom d'utilisateur (l'identité n'est qu'une chaîne déclarée) ;
- **altérer/rejouer** des paquets (aucune intégrité, aucun anti-rejeu).

Objectif : confidentialité + intégrité + authentification mutuelle des pairs
+ anti-rejeu, **pendant le transfert**. (Le chiffrement au repos —
`abcom.db`/fichiers locaux — est explicitement hors scope ici, cf. §4.6.)

### 4.2 Choix technique : Noise XX (crate `snow`)

| Option | Pour | Contre |
|---|---|---|
| **Noise XX (`snow`)** ✅ | Conçu pour le P2P sans autorité centrale ; clés statiques = identités ; forward secrecy (éphémères) ; dépendance légère et pure Rust ; s'adosse naturellement aux connexions persistantes (R2) | Modèle de confiance à gérer soi-même (TOFU, §4.4) |
| TLS 1.3 (`rustls`) | Mature, outillé | Pensé pour un modèle certificats/PKI : en P2P il faut générer des certificats auto-signés par pair et épingler quand même (on refait du TOFU avec plus de machinerie et ~1,5 Mo de binaire en plus) |
| Passphrase partagée (PSK, ChaCha20-Poly1305) | Très simple | Pas d'identité par pair, secret unique à distribuer, pas de forward secrecy — insuffisant seul |

**Recommandation : Noise `XX` + `snow`** (motif `Noise_XX_25519_ChaChaPoly_BLAKE2s`).
Le motif XX échange les clés statiques pendant le handshake (3 messages,
1,5 RTT — négligeable sur LAN), fournit l'authentification mutuelle et la
forward secrecy. Option durcissement ultérieur : variante `XXpsk3` avec une
passphrase de « salon » pour restreindre qui peut même tenter un handshake.

**Prérequis structurel : R2 (connexions persistantes).** Chiffrer le modèle
actuel « une connexion TCP par paquet » imposerait un handshake par message.
Le passage aux connexions persistantes et le chiffrement se livrent ensemble :
une session Noise par paire de pairs, établie une fois, réutilisée pour tout
(messages, réactions, accusés, frappe, et flux médias).

### 4.3 Architecture cible

```
Identité      : paire X25519 statique, générée au premier lancement,
                stockée dans le répertoire de données (permissions 0600).
                Empreinte = BLAKE2s(clé publique), affichable dans Paramètres.

Découverte    : l'annonce UDP (inchangée : présence en clair) transporte en
(UDP 9001)      plus l'empreinte de clé publique → permet de lier
                username ↔ clé avant toute connexion TCP.

Session       : connexion TCP persistante (R2) → handshake Noise XX →
(TCP 9000)      canal chiffré. Frames : u32 longueur + ciphertext AEAD.
                Un message Noise ≤ 65 535 octets → payloads ≤ 65 519.
                Tous les NetworkPacket (JSON) passent dedans, plus de
                MAX_PACKET_SIZE applicatif (le framing borne naturellement).

Médias        : même mécanique sur le port média (chat+1), ou — préférable —
(TCP 9001)      multiplexage sur la connexion unique : un octet de type de
                frame (0 = paquet JSON, 1 = chunk média) suffit, le
                streaming découpe en chunks de 60 Ko chiffrés (throttle de
                progression déjà en place). Permet de supprimer le second
                listener et le protocole d'en-tête média séparé.

Confiance     : TOFU (Trust On First Use) — à la première connexion d'un
                pair, le couple (username, clé publique) est enregistré
                (table peers de R1). Connexion suivante : la clé reçue au
                handshake DOIT correspondre. Sinon : connexion refusée +
                bandeau UI « la clé de X a changé » avec action explicite
                « faire confiance à la nouvelle clé ».
```

### 4.4 Étapes d'implémentation

| Étape | Contenu | Dépend de | Effort |
|---|---|---|---|
| S1 | Identité : génération/stockage de la paire X25519, empreinte dans l'annonce discovery, écran Paramètres (afficher son empreinte) | — | S |
| S2 | **R2** : connexion persistante par pair, framing `u32 + payload`, reconnexion avec backoff, fusion des senders (R6) | — | L |
| S3 | Handshake Noise XX sur la connexion S2 (initiateur = celui qui connecte), chiffrement de toutes les frames, suppression de `MAX_PACKET_SIZE` (corrige R3 au passage) | S1, S2 | M |
| S4 | Multiplexage du flux média sur la session chiffrée (chunks 60 Ko), suppression du listener média et de son protocole d'en-tête dédié | S3 | M |
| S5 | TOFU : persistance username↔clé (R1 si dispo, sinon JSON provisoire), refus sur mismatch, bandeau « clé changée » + action de ré-appairage | S3 | M |
| S6 | (Option) `XXpsk3` : passphrase de salon configurable dans Paramètres | S3 | S |

**Tests** : handshake aller-retour en mémoire (unitaires `snow`) ; intégration
deux endpoints réels (messages + média + reconnexion + clé changée → refus) ;
test de non-régression du débit média (~chiffrement ChaCha20-Poly1305 :
plusieurs Go/s par cœur, non limitant sur LAN).

**Vérification finale** : capture `tcpdump`/Wireshark sur le port de chat
pendant un échange → aucun texte en clair ; tentative de connexion d'un
client non-Noise → rejetée proprement.

### 4.5 Coûts

- Dépendance : `snow` (+ `blake2`/`chacha20poly1305` transitifs), ~0,3 Mo de
  binaire. CPU : négligeable (AEAD par frame). RAM : un état de session par
  pair (~1 Ko). Latence : +1,5 RTT à l'établissement d'une session, une fois.
- Aucune migration : pas de release publiée (décision n°5) — l'ancien
  protocole clair est simplement supprimé.

### 4.6 Hors scope (assumé, à décider plus tard)

- **Chiffrement au repos** : `abcom.db`, `media/`, avatars restent en clair
  sur le disque local. Levier futur : SQLCipher ou chiffrement fichier par
  clé dérivée de la session utilisateur.
- **Métadonnées de découverte** : l'annonce UDP (username + empreinte) reste
  visible sur le LAN — c'est la fonction même de la découverte. `XXpsk3`
  (S6) empêche toutefois un inconnu d'établir une session.
- **Groupes** : le chiffrement est par lien pair-à-pair (chaque membre reçoit
  sur sa session) — pas de clé de groupe partagée ; suffisant tant que les
  messages de groupe sont relayés par l'émetteur à chaque membre.

---

## 5. Ordre d'exécution consolidé (mise à jour du plan 07)

| Étape | Contenu | Réf. | Effort |
|---|---|---|---|
| 1 | N1 (découverte silencieuse hors changement) + N2 (générations scindées) — verrouille le « zéro réveil au repos » avec pairs connectés | §3 | S |
| 2 | N3 + N4 (picker GIF, modale groupe, shortcodes) + R5 (`resvg` optionnel) + R8 | §3, §2 | S |
| 3 | **R1 — SQLite** (persistance, avatars BLOB, pagination base, GC transactionnel) | §2 | L |
| 4 | S1 (identité) puis **S2/R2 — connexions persistantes** | §4 | L |
| 5 | **S3 — chiffrement Noise** + S5 (TOFU) — corrige R3 au passage | §4 | M |
| 6 | S4 — média multiplexé chiffré | §4 | M |
| 7 | R4 (atlas emoji), R7 (wgpu / mode tray / notifications natives) | §2 | M–L |
| 8 | S6 (passphrase de salon, option) | §4 | S |

Les étapes 1–2 sont des finitions immédiates de la passe 1. La 3 (SQLite)
peut avancer en parallèle de la 4 (réseau). La sécurité (4–6) forme un bloc
cohérent livrable en une série de PR sur une branche `feature/secure-transport`.
