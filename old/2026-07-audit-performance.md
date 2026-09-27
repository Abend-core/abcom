> [🏠 Accueil](../README.md) > [⚡ Audit performance & mémoire](2026-07-audit-performance.md)

> 📅 **Généré le** : 2026-07-04 (passe 2 : vérification complète + décisions actées)
> 🔖 **Stack analysée** : Rust 2021, tokio 1, eframe/egui 0.31, egui_extras (image/http/gif/webp), image 0.25, resvg 0.47, rodio 0.19
> 🔄 **À régénérer si** : refonte du rendu du fil, changement de format de persistance, montée de version egui

# Audit performance & empreinte mémoire

Audit statique du code (branche `feature/reactions-reponses`).
**Aucune correction n'a été appliquée** : ce document est le plan d'exécution
d'une passe d'optimisation ultérieure. Chaque constat référence fichier:ligne,
décrit l'impact, la correction et la vérification attendue.

**Contexte produit** : Abcom est une messagerie LAN destinée à tourner **en
permanence** (toujours ouverte, toujours en route). L'objectif est donc une
empreinte minimale au repos sur les trois axes — CPU, GPU, RAM — plus le
disque, tout en restant parfaitement utilisable au premier plan.

---

## 0. Baseline mesurée & objectifs validés

| Mesure | Valeur | Source |
|---|---|---|
| RAM (RSS) | **443 Mo** | mesure utilisateur sur macOS, 2026-07-04 |
| CPU (repos apparent) | **22 %** | Moniteur d'activité, capture 2026-07-04 |
| GPU | **9,7 %** | idem |
| Threads | **15** | idem |
| Disque | valeur à reporter | — |

➡️ Plan d'exécution détaillé : [07-plan-optimisation.md](2026-07-plan-optimisation.md).

**Objectifs validés** (décision n°1) :
- CPU ~0 % au repos (aucun repaint sans événement) ;
- RSS < 150 Mo en usage courant (hors visionneuse plein écran) ;
- aucun à-coup > 16 ms à la réception d'un message ;
- s'y ajoutent : GPU au repos ~0 (pas de frame rendue sans changement) et
  **empreinte disque bornée** (rétention des médias plafonnée, cf. C-2).

Ordre de grandeur : 443 Mo pour un chat LAN est ~4–8× trop. La ventilation la
plus probable (à confirmer avec Instruments → Allocations) : cache d'images
egui jamais purgé (frames de GIF/WebP décodées, P0-4), textures médias pleine
résolution (P0-4), textures emoji (P2-1), le reste étant la base
eframe/Glow (~80–120 Mo incompressibles environ).

## 0bis. Décisions actées (réponses du 2026-07-04)

1. **Objectifs chiffrés** : validés (ci-dessus).
2. **Persistance** : migration vers **SQLite** (vision SQL souhaitée) — plan
   détaillé en §7. Le débounce JSON de la passe 1 devient inutile : on va
   directement à SQLite.
3. **GIFs** : gel hors écran accepté, **mais** un GIF doit s'animer dès qu'il
   est même partiellement visible (aucune gêne d'utilisation). Question posée
   sur WebM : réponse en §8 — on reste sur WebP animé, WebM n'apporte rien ici.
4. **Fil de messages** : pagination **façon Discord** — pas de bouton ; quand
   l'utilisateur remonte près du haut, les 100 messages précédents se chargent
   automatiquement. Spécification en §9.
5. **Compatibilité protocole** : aucune release publiée → **on casse tout ce
   qu'on veut** (connexion persistante par pair possible sans migration).

---

## 1. Synthèse des problèmes structurels

1. **Rendu immédiat sans cache ni virtualisation** : à chaque frame, tout le
   fil est cloné depuis l'état partagé, re-parsé (markdown), re-haché,
   re-layouté — O(n messages) par frame — avec un repaint forcé toutes les
   500 ms même au repos et un repaint continu dès qu'un GIF est visible.
2. **La réception d'un message coûte O(historique) + I/O disque synchrone
   dans le thread UI** (réécriture JSON complète sous mutex).
3. **Aucun cache borné** : textures pleine résolution jamais libérées, frames
   de GIF conservées à vie, HashMaps d'accusés/réactions jamais purgées, et
   **dossier `media/` jamais nettoyé** (disque sans limite).
4. **Réglages absents** : pas de `[profile.release]`, tokio `full`, runtime et
   threads surdimensionnés.

---

## 2. Constats priorisés

Priorités : **P0** majeur · **P1** net · **P2** finition. Effort S/M/L.

### P0-1 · Réécriture complète de l'historique à chaque message reçu (thread UI, sous mutex)

- **Où** : `src/app/messages.rs:19` (`add_message` → `save_messages()`),
  `src/app/persistence.rs:70-76` (`to_string_pretty` de tous les messages +
  écriture synchrone). Idem `save_read_counts` à chaque
  `mark_conversation_read` (`src/app/messages.rs:29`), `save_reactions` à
  chaque réaction, le tout appelé depuis `process_events`
  (`src/ui/events.rs:55`) qui tient le verrou pendant toute la frame.
- **Impact** : coût par message reçu proportionnel à l'historique, à-coups UI,
  usure disque.
- **Correction** : remplacée par la **migration SQLite** (§7) : l'ajout d'un
  message devient un `INSERT` unique hors thread UI. Plus besoin de débounce
  JSON.
- **Vérification** : chronométrer `process_events` (< 50 µs par message) ;
  `fs_usage -w -f filesys <pid>` : plus d'écriture massive par message.
- **Effort** : couvert par §7 (L, mais remplace 7 fichiers JSON et leur code).

### P0-2 · Fil de messages entièrement re-rendu et re-cloné à chaque frame

- **Où** : `src/ui/chat_panel.rs:524-530` — clone profond de toute la
  conversation **à chaque frame** ; `src/app/messages.rs:33-49` clone
  `my_username` par message dans le filtre.
- **Par message et par frame** : hash FNV recalculé avec `format!` allouant
  (`chat_panel.rs:815` → `src/app/receipts.rs:26-33`) ; jusqu'à 3 verrous
  mutex (accusés `:819`, citation `:825-831`, `reactions_for(...).to_vec()`
  `:846`) ; `find_message_by_hash` (`src/app/reactions.rs:77`) = scan O(n)
  qui re-hache chaque message → **O(n²) par frame** dès qu'il y a des
  réponses ; maps `author_avatars`/`author_names` reconstruites par frame
  (`chat_panel.rs:717-736`, tri + dédup + verrou).
- **Pas de virtualisation** : `ScrollArea::vertical().show()`
  (`chat_panel.rs:757`) paie le layout des ~500 messages même hors écran.
- **Correction** (ordre suggéré) :
  1. **Cache de données dérivées par message**, invalidé par un compteur de
     génération dans `AppState` : hash, blocs markdown parsés, index de la
     citation résolue, regroupement Discord (starts_group/jour), noms/avatars
     d'auteurs. C'est le gros du gain CPU.
  2. Snapshot partagé (`Arc<[ChatMessage]>` régénéré sur changement) au lieu
     du clone par frame.
  3. **Virtualisation + pagination Discord** : voir §9 (remplace le
     ring-buffer de 500 par une fenêtre chargée depuis SQLite).
  4. Micro : sortir `my_username.clone()` des filtres ; `reactions_for` sans
     `to_vec()`.
- **Vérification** : frame-time indépendant de n (overlay §6).
- **Effort** : M pour 1+2+4 ; §9 pour la virtualisation.

### P0-3 · Markdown re-parsé à chaque frame pour chaque message

- **Où** : `src/ui/markdown.rs:351-360` — `parse_markdown` + `is_text_emoji_only`
  à chaque rendu de chaque message visible (allocations Vec/String par frame).
- **Correction** : parser une fois (à l'ajout ou en cache paresseux) ;
  s'intègre au cache P0-2.1.
- **Effort** : S une fois P0-2 en place.

### P0-4 · Caches d'images non bornés — cause n°1 probable des 443 Mo

- **Où** :
  - **GIF/WebP animés** : `src/ui/media.rs:181` (fil) et
    `src/ui/gif_picker.rs:128` (24 aperçus par page) via
    `egui::Image::from_uri` + loaders `egui_extras` (`src/ui/mod.rs:516`).
    Les loaders conservent **les octets téléchargés ET toutes les frames
    décodées** dans le cache du `Context`, **sans éviction** —
    `ctx.forget_image()` n'est appelé nulle part (vérifié). Chaque GIF affiché
    une fois (fil ou simple aperçu du picker) reste en RAM à vie. Un GIF HD de
    3 s ≈ 50–150 Mo décodés. Chaque « page suivante » du picker ajoute 24
    aperçus animés de plus, jamais libérés.
  - **Le fil affiche la variante HD** : `render_media_block` utilise
    `media.url` = `full_url` (variante **hd** Klipy, cf. `src/klipy.rs:85`)
    pour une boîte d'affichage de 360×300 max (`src/ui/media.rs:19-20`).
    Décodage et frames HD pour un rendu vignette → gâchis mémoire majeur.
  - **Images reçues** : `src/ui/media.rs:379-406` — décodage pleine
    résolution + texture GPU jamais libérée (`media_textures`, HashMap sans
    éviction, toutes conversations confondues). Une photo 12 Mpx ≈ 48 Mo.
- **Correction** :
  1. Fil : afficher la variante **md/sm** (≤ 360 px), réserver la HD à la
     visionneuse ; le message réseau peut continuer de transporter les deux URLs.
  2. Images reçues : **downscale avant `load_texture`** à ~2× la taille
     d'affichage (retina), pleine résolution uniquement dans la visionneuse,
     texture libérée à sa fermeture.
  3. **Éviction LRU** sur `media_textures` (~30 entrées) + purge au changement
     de conversation.
  4. GIFs : `ctx.forget_image(uri)` à la fermeture du picker et pour tout GIF
     sorti de la fenêtre de messages chargée ; stratégie d'animation en §8.
- **Vérification** : RSS après 3 pages de picker puis fermeture → doit
  redescendre ; RSS en conversation riche en GIFs → borné.
- **Effort** : M.

### P0-5 · 🆕 Progression média : un événement par tranche de 64 Ko

- **Où** : `src/network/media_stream.rs:109-119` (émission) et `:219-230`
  (réception) — `event_tx.send(progress(...)).await` **à chaque chunk de
  64 Ko**, dans un canal mpsc de 256 places (`src/main.rs:35`) que l'UI ne
  draine qu'au repaint (≤ 2 Hz au repos).
- **Impact double** :
  1. **Le débit de transfert est plafonné par la boucle de rendu** : une fois
     les 256 places pleines, `send().await` bloque le streaming jusqu'au
     prochain repaint — ~256 × 64 Ko par 500 ms ≈ **32 Mo/s max** théorique,
     pire si l'UI est occupée. Un fichier de 1 Go émet ~16 000 événements.
  2. Chaque événement traité = insertion HashMap + notification → CPU inutile
     pendant tout transfert.
- **Correction** : throttler l'émission de progression (au plus tous les 1 %
  **ou** toutes les 100 ms, dernière valeur gagnante) ; idéalement remplacer
  par un `Arc<AtomicU64>` partagé (octets transférés) que l'UI lit quand elle
  peint la barre — zéro événement, zéro backpressure.
- **Vérification** : débit d'un transfert de 1 Go entre deux machines LAN
  avant/après ; nombre d'événements reçus.
- **Effort** : S.

### P1-1 · Repaint permanent, même au repos

- **Où** : `src/ui/mod.rs:422` — `request_repaint_after(500ms)` à chaque
  `update()` : ≥ 2 repaints/s en permanence, uniquement pour **poller**
  `event_rx.try_recv()`. Tout GIF visible force en plus un repaint continu ;
  picker GIF ouvert = 24 animations + repaint 300 ms forcé
  (`src/ui/gif_picker.rs:306`).
- **Correction** :
  1. **Réveil par événement** : cloner l'`egui::Context` (Clone + Send) vers
     les tâches tokio ; `ctx.request_repaint()` après chaque `event_tx.send`
     (`src/klipy.rs:297` le fait déjà — généraliser à `discovery`,
     `network::server`, `media_stream`). Fallback périodique long (2–5 s)
     uniquement pour `periodic_tasks`.
  2. **Fenêtre non focalisée / masquée** : allonger encore le fallback (ex.
     10 s) et suspendre les animations GIF — pour une app « toujours
     ouverte », c'est l'état dominant ; le GPU doit être à ~0 quand la fenêtre
     est en arrière-plan. (Aller plus loin : mode « icône barre de menus »
     sans fenêtre = zéro rendu, cf. §10-G.)
- **Vérification** : Moniteur d'activité, app au premier plan sans activité
  60 s : ~0 % CPU, « GPU time » nul ; réveil uniquement sur événement.
- **Effort** : S (réveil) / M (gel animations, cf. §8).

### P1-2 · Barre latérale et barre de saisie : clones et O(n) par frame

- **Où** : `src/ui/sidebar.rs:17-30` — par frame : `peers.clone()`,
  `peer_records.clone()`, `unread_count()` **par pair** qui rescanne tous les
  messages (`src/app/messages.rs:60-71`, avec clone de `my_username` par
  message) ; `sidebar.rs:180` `groups.clone()`. 🆕 `src/ui/input_bar.rs` :
  3 verrous + `peers.clone()` **par frame** (`:419-426`, `:449`, `:611-614`).
- **Correction** : compteurs non-lus **incrémentaux** dans `AppState`
  (SQLite les fournit aussi par requête, mais le compteur mémoire évite la
  requête par frame) ; snapshot sidebar/input mis en cache sur le compteur de
  génération de P0-2.
- **Effort** : S.

### P1-3 · Réglages de build absents

- **Où** : `Cargo.toml` — aucun `[profile.release]`.
- **Correction** :
  ```toml
  [profile.release]
  lto = "thin"          # tester "fat" si temps de build acceptable
  codegen-units = 1
  strip = true
  panic = "abort"
  opt-level = 3         # ou "s" si la taille binaire prime
  ```
- **Effort** : S. **Vérification** : taille binaire + benchs §6.

### P1-4 · Runtime, threads et dépendances surdimensionnés

- **Où** :
  - `Cargo.toml:14` — `tokio features = ["full"]` : ne garder que
    `rt-multi-thread`, `net`, `time`, `sync`, `io-util`, `fs`, `macros`.
  - `src/main.rs:51-69` — runtime multi-thread par défaut (1 worker/cœur) +
    **11 tâches permanentes** dont 7 boucles `run_sender_*` identiques
    (`src/network/sender.rs`) fusionnables en un canal
    `mpsc<(SocketAddr, NetworkPacket)>`.
  - `src/ui/sound.rs:7` — **un thread + initialisation du périphérique audio
    par notification** (`OutputStream::try_default()` par bip). Garder un
    thread audio pérenne avec `OutputStream` unique et un canal de commandes.
  - 🆕 `resvg` (0.47, dépendance lourde) ne sert **qu'à importer un avatar
    SVG** (`src/ui/avatar.rs:52`). Feature Cargo optionnelle ou suppression
    du support SVG → binaire et temps de compilation réduits.
- **Correction** : `worker_threads(2)` (les transferts média sont du
  disque→réseau bufferisé, 2 workers suffisent), features explicites, senders
  fusionnés, audio pérenne, resvg optionnel.
- **Effort** : S–M. **Vérification** : nombre de threads (`ps -M <pid>`),
  taille binaire.

### P1-5 · HashMaps d'état jamais purgées (fuite lente)

- **Où** : `src/app/mod.rs:28-37` — `read_receipts`, `pending_messages`,
  `reactions` indexés par hash mais jamais nettoyés quand les messages sortent
  du ring-buffer (`src/app/messages.rs:16-18`).
- **Correction** : absorbé par SQLite (§7) — réactions/accusés deviennent des
  lignes liées au message, supprimées par contrainte ou requête d'entretien.
  En attendant : purge des trois maps lors du `drain`.
- **Effort** : S.

### P1-6 · 🆕 Empreinte disque non bornée : `media/` jamais nettoyé

- **Où** : `src/app/media.rs` — chaque fichier/image reçu est écrit dans
  `media/<id>` et **n'est supprimé que** sur transfert échoué/refusé
  (`remove_media_message`). Quand le ring-buffer élimine les vieux messages
  (`src/app/messages.rs:16-18`), leurs fichiers médias deviennent orphelins
  **pour toujours**. C'est la cause probable de la consommation disque
  constatée (capture utilisateur).
- **Correction** : politique de rétention — plafond configurable (ex. 2 Go) ;
  au démarrage et périodiquement, supprimer d'abord les orphelins (non
  référencés par l'historique), puis les plus anciens au-delà du plafond
  (LRU). Avec SQLite, une table `media` (id, taille, dernier accès) rend ce
  GC trivial et transactionnel.
- **Vérification** : `du -sh ~/…/abcom/media` stable dans le temps.
- **Effort** : S–M.

### P1-7 · 🆕 Avatars persistés en JSON de tableaux d'octets

- **Où** : `src/app/avatar.rs:80-87` — `peer_avatars.json` sérialise des
  `HashMap<String, Vec<u8>>` (PNG 256×256) via serde_json : chaque octet
  devient un nombre décimal + virgule ≈ **3,7× la taille réelle** sur disque,
  et parse/écriture coûteux. Réécrit intégralement à chaque avatar reçu.
- **Correction** : BLOBs SQLite (§7) ou fichiers `avatars/<user>.png`.
- **Effort** : S (absorbé par §7).

### P2-1 · Chargement emoji bloquant et coûteux au premier frame

- **Où** : `src/ui/events.rs:12-52` — 323 PNG décodés + 323 textures créées
  synchroneusement au premier `update()` (~6–7 Mo de textures, gel du premier
  frame ; 1,5 Mo embarqués dans le binaire, `src/emoji_registry.rs`).
- **Correction** : **atlas unique** (une texture ~1,5k×1,5k, UVs par emoji —
  réduit mémoire, bindings GPU et temps de démarrage), ou décodage paresseux
  par catégorie du picker.
- **Effort** : M.

### P2-2 · Thème réappliqué à chaque frame

- **Où** : `src/ui/mod.rs:327` → `src/ui/settings.rs:12-28` —
  `ctx.set_visuals(...)` reconstruit un `Visuals` complet **à chaque frame**.
- **Correction** : appliquer uniquement au changement. **Effort** : S.

### P2-3 · Une connexion TCP par paquet

- **Où** : `src/network/sender.rs:10-21` — connect/write/shutdown pour chaque
  message, ACK, accusé de lecture, réaction, frappe ; `server.rs` lit chaque
  connexion jusqu'à EOF.
- **Décision n°5** : pas de compatibilité à préserver → **connexion
  persistante par pair** avec framing longueur-préfixée (le `write_u32` +
  JSON de `media_stream.rs` fait déjà ce framing — l'étendre au chat). Réduit
  syscalls, latence, et permet de supprimer le timeout de 5 s par connexion.
- **Effort** : L (mais simplifie `run_sender_*` fusionnés, cf. P1-4).

### P2-4 · Divers micro-coûts par frame

- `src/ui/chat_panel.rs:402-406` : hover-toolbar — clone de la liste des
  emojis récents + recherche linéaire des textures à chaque frame de survol.
- `src/ui/mod.rs:328,337` : double lecture `ctx.input(|i| i.focused)`.
- `src/app/messages.rs:9-12` : `msg.clone()` complet dans `add_message` pour
  n'utiliser ensuite que `msg.from`.
- 🆕 `src/ui/gif_picker.rs:92-94` : `items.clone()` (Vec de GifItem avec 3
  Strings chacun) à chaque frame tant que le picker est ouvert.
- 🆕 `src/discovery.rs:119-123` : `PeerDiscovered` renvoyé toutes les 3 s par
  pair → `add_peer` + tentative d'annonce avatar à chaque tick (garde
  `avatar_sent_to` OK, mais l'événement pourrait n'être émis qu'au changement).

---

## 3. Chemin « réception d'un message » — avant / après

**Aujourd'hui** (O(historique) + I/O synchrone) :
```
TCP accept → read_to_end → parse JSON → mpsc
  → (au prochain repaint, ≤ 500 ms) process_events [verrou UI]
    → add_message : clone + serde pretty de TOUT l'historique + write disque [thread UI]
  → frame suivante : re-clone conversation, re-parse markdown, re-hash, 3 verrous/message
```

**Cible** (O(1) amorti) :
```
connexion persistante → frame décodée → mpsc → ctx.request_repaint() [réveil immédiat]
  → process_events : INSERT SQLite (hors thread UI) + maj caches dérivés
    (hash, markdown, compteur non-lus) + génération++
  → rendu : lecture du cache, seuls les messages visibles sont layoutés
```

---

## 4. Ordre d'exécution recommandé (mis à jour)

| Étape | Contenu | Constats | Effort |
|---|---|---|---|
| 1 | `[profile.release]` + features tokio + resvg optionnel | P1-3, P1-4 | S |
| 2 | Réveil par événement + fallback long + fallback très long si non focalisé | P1-1 | S |
| 3 | Throttle/atomic progression média | P0-5 | S |
| 4 | Thème au changement, micro-coûts, senders fusionnés, audio pérenne | P2-2, P2-4, P1-4 | S |
| 5 | **Migration SQLite** (messages, réactions, accusés, avatars, compteurs) | P0-1, P1-5, P1-7, §7 | L |
| 6 | Cache dérivé par message + compteurs incrémentaux | P0-2.1/2.4, P0-3, P1-2 | M |
| 7 | Textures : variante md dans le fil, downscale, LRU, `forget_image` | P0-4 | M |
| 8 | GC disque `media/` (rétention plafonnée) | P1-6 | S–M |
| 9 | Pagination Discord + virtualisation + gel GIFs hors écran | §9, §8, P0-2.3 | L |
| 10 | Connexion persistante par pair | P2-3 | L |
| 11 | Atlas emoji | P2-1 | M |

Étapes 1–4 : gains immédiats, sans risque, livrables en une PR chacune.
L'étape 5 (SQLite) est le prérequis naturel de la 9 (pagination). La 7 et la
8 sont indépendantes et attaquent directement les 443 Mo / le disque.

---

## 5. (fusionné en §6)

## 6. Protocole de mesure (avant ET après chaque étape)

1. **Baseline** : `cargo build --release`, deux instances locales.
   Référence actuelle : **RSS 443 Mo**.
2. **Mémoire** : `ps -o rss= -p <pid>` à 4 instants : démarrage, après 100
   messages, après 3 pages de recherche GIF, après 10 min de repos.
   Instruments → Allocations pour la ventilation.
3. **CPU repos** : 60 s sans activité au premier plan → % CPU moyen
   (objectif ~0 %). Refaire fenêtre en arrière-plan.
4. **GPU** : Moniteur d'activité (onglet Énergie / GPU) ou
   `sudo powermetrics --samplers gpu_power -n 5` : temps GPU nul au repos.
5. **Temps de frame** : overlay temporaire dans `update()` ou feature
   `eframe/puffin` pour ventiler par fonction.
6. **Message reçu** : chronométrage `process_events` + `fs_usage -w -f
   filesys <pid>` (aucune écriture par message après l'étape 5).
7. **Transfert** : débit d'un fichier de 1 Go avant/après l'étape 3.
8. **Disque** : `du -sh` du répertoire de données après une semaine d'usage
   simulé (objectif : borné par le plafond de rétention).
9. **Profiling CPU** : `cargo flamegraph` / Instruments → Time Profiler en
   scrollant 500 messages avec GIFs — `parse_markdown`, `message_hash`,
   `serde_json` doivent disparaître du profil.

---

## 7. Plan de migration SQLite (décision n°2)

**Crate** : `rusqlite` avec feature `bundled` (SQLite embarqué, +~0,8 Mo de
binaire, zéro dépendance système). Mode **WAL**, `synchronous=NORMAL` (adapté
à un chat local), connexion détenue par une **tâche dédiée** (thread ou tâche
tokio `spawn_blocking`) recevant les écritures par canal — le thread UI ne
touche jamais le disque.

**Schéma proposé** (`abcom.db` dans le répertoire de données) :

```sql
CREATE TABLE messages (
  id          INTEGER PRIMARY KEY,          -- rowid
  hash        INTEGER NOT NULL UNIQUE,      -- FNV actuel, conservé pour le réseau
  from_user   TEXT    NOT NULL,
  to_user     TEXT,                         -- NULL = broadcast, '#…' = groupe
  content     TEXT    NOT NULL,
  ts_epoch    INTEGER,                      -- secondes Unix
  media_id    TEXT,                         -- FK → media.id
  reply_to    INTEGER                       -- hash du message cité
);
CREATE INDEX idx_messages_conv ON messages (to_user, from_user, ts_epoch);

CREATE TABLE reactions (
  message_hash INTEGER NOT NULL,
  emoji        TEXT    NOT NULL,
  username     TEXT    NOT NULL,
  PRIMARY KEY (message_hash, emoji, username)
);

CREATE TABLE receipts (         -- ACK + lectures
  message_hash INTEGER NOT NULL,
  username     TEXT    NOT NULL,
  kind         INTEGER NOT NULL,            -- 0 = delivered, 1 = read
  PRIMARY KEY (message_hash, username, kind)
);

CREATE TABLE media (
  id           TEXT PRIMARY KEY,            -- nom du fichier dans media/
  filename     TEXT NOT NULL,
  kind         INTEGER NOT NULL,
  size_bytes   INTEGER NOT NULL,
  width        INTEGER, height INTEGER,
  url          TEXT,                        -- GIF Klipy (distant)
  last_access  INTEGER NOT NULL             -- pour le GC LRU (P1-6)
);

CREATE TABLE peers (
  username TEXT PRIMARY KEY,
  alias    TEXT,
  avatar   BLOB                             -- PNG 256×256 (remplace peer_avatars.json)
);

CREATE TABLE kv (k TEXT PRIMARY KEY, v TEXT);  -- read_counts, préférences, mon avatar
```

**Ce que ça remplace** : `messages.json`, `reactions.json`,
`read_counts.json`, `peer_records.json`, `peer_avatars.json`, `groups.json`
(table `groups` ou `kv`), soit tout `src/app/persistence.rs`.

**Bénéfices directs** :
- ajout d'un message = 1 INSERT O(1) (règle P0-1) ;
- compteurs non-lus par requête indexée ou maintenus en mémoire ;
- purge réactions/accusés automatique lors de la suppression d'un message ;
- GC des médias trivial (P1-6) : `SELECT id FROM media WHERE id NOT IN
  (SELECT media_id FROM messages ...)` + tri `last_access` ;
- pagination keyset pour le scroll infini (§9) :
  `... WHERE ts_epoch < :avant ORDER BY ts_epoch DESC, id DESC LIMIT 100` ;
- avatars en BLOB (règle P1-7) ;
- plus de ring-buffer arbitraire de 500 : l'historique complet vit en base,
  la RAM ne contient que la fenêtre affichée.

**Migration** : au premier démarrage, si `abcom.db` absent et fichiers JSON
présents → import transactionnel puis renommage des JSON en `.bak`.
`message_hash` est conservé tel quel (identifiant réseau inter-pairs).

---

## 8. Stratégie GIF (décision n°3) et réponse à la question WebM

**Réponse courte : non, WebM n'aiderait pas — on reste sur WebP animé.**

- Le coût mémoire des GIFs ne vient **pas du format téléchargé** mais des
  **frames décodées** : quel que soit le codec (GIF, WebP, WebM/VP9, AV1),
  une frame affichée devient un bitmap RGBA de `largeur × hauteur × 4` octets.
  Un format plus moderne réduit le réseau, pas la RAM de rendu.
- Klipy sert déjà des **variantes WebP animées** (transport ~3–5× plus léger
  que le GIF) — c'est ce que l'app utilise (`src/klipy.rs:99`), le bon choix
  est déjà fait côté réseau.
- **WebM est un conteneur vidéo** : egui ne sait pas le décoder ; il faudrait
  embarquer un décodeur vidéo (ffmpeg/GStreamer ou dav1d), soit des dizaines
  de Mo de binaire et de la complexité — l'inverse de l'objectif « application
  la plus petite possible ».

**Le vrai levier, c'est quand et à quelle taille on décode** :

1. ~~Variante md dans le fil~~ — **décision produit 2026-07-04 : le fil reste
   en HD.** Le bornage mémoire des GIFs repose donc entièrement sur les
   points 2 et 3 (gel hors écran + éviction), qui deviennent obligatoires.
2. **Animation liée à la visibilité** (exigence : animer dès qu'un pixel est
   visible) : au rendu de chaque ligne, tester
   `ui.clip_rect().intersects(row_rect)` —
   - **intersecte (même partiellement)** → `Image::from_uri` animée, normale ;
   - **hors écran** → ne pas émettre le widget animé (la ligne garde sa
     hauteur réservée). Aucun retard perceptible : le test est fait à la
     frame où le GIF entre dans le viewport, il s'anime donc dès son premier
     pixel visible. Les frames déjà décodées restent en cache tant que le GIF
     est dans la fenêtre chargée → ré-entrée instantanée.
3. **Éviction** : `ctx.forget_image(uri)` quand le message sort de la fenêtre
   de messages chargée (§9) ou à la fermeture du picker (tous les aperçus de
   la session de recherche). Optionnel : plafond global (ex. 40 URIs
   animées vivantes, éviction LRU).
4. **Fenêtre en arrière-plan** : suspendre toutes les animations (P1-1.2) —
   une messagerie toujours ouverte passe l'essentiel de sa vie non focalisée.

---

## 9. Pagination « façon Discord » (décision n°4)

Pas de bouton : le chargement est déclenché par la position de scroll.

**Modèle** : la RAM ne contient qu'une **fenêtre contiguë** de la conversation
(ex. 200 messages), chargée depuis SQLite (§7).

1. **État** : `Vec<ChatMessage>` (fenêtre) + `oldest_loaded_id` +
   `bottom_pinned: bool` (l'utilisateur est en bas → stick_to_bottom actif).
2. **Déclencheur** : pendant le rendu du `ScrollArea`, si
   `state.offset.y < SEUIL` (ex. 600 px) et qu'il reste de l'historique →
   requête keyset des **100 précédents** (`WHERE (ts_epoch,id) < … LIMIT
   100`). La requête part vers la tâche SQLite (asynchrone) ; un petit
   spinner discret en tête de fil pendant le vol (généralement < 1 frame sur
   une base locale).
3. **Prépendage sans saut visuel** : après insertion des 100 messages en tête,
   compenser l'offset du scroll de la hauteur ajoutée. Deux options :
   - mesurer la hauteur réelle ajoutée à la frame suivante (delta de
     `content_size`) et faire `offset.y += delta` — robuste avec hauteurs
     variables ;
   - ou hauteurs mises en cache par message (déjà utiles à la virtualisation).
4. **Borne supérieure** : si la fenêtre dépasse ~400 messages, **élaguer
   l'autre extrémité** (les plus récents si on remonte, les plus anciens si on
   redescend) et `forget_image` des GIFs élagués — RAM bornée dans les deux
   sens, l'historique complet restant accessible via la base.
5. **Retour en bas** : nouvel événement reçu + `bottom_pinned` → scroll bas ;
   si l'utilisateur est en haut de l'historique, afficher le badge « nouveaux
   messages ↓ » qui recharge la fenêtre la plus récente.
6. **Interaction avec le cache P0-2.1** : le cache dérivé (markdown, hash,
   groupes) vit dans la fenêtre — il se remplit à l'insertion des pages et
   s'élague avec elles.

Ceci **remplace** le ring-buffer de 500 (`src/app/messages.rs:16-18`) et le
`get_conversation_messages()` filtrant tout l'historique.

---

## 10. Autres pistes « tout est prenable » (exploratoires, à mesurer)

- **A. Renderer** : `eframe::Renderer::Glow` (`src/ui/mod.rs:505`) = OpenGL,
  déprécié par Apple et traduit par la couche compat. Tester
  `Renderer::Wgpu` (Metal natif) : souvent moins d'énergie GPU sur macOS,
  au prix d'un binaire plus gros — trancher à la mesure (powermetrics).
- **B. Mode barre de menus / tray** : pour « toujours en route », l'idéal est
  fenêtre fermée = **zéro rendu** : icône de statut + notifications système,
  la fenêtre ne se rouvre (et ne rend) qu'à la demande. eframe supporte
  la fermeture vers un état caché ; les tâches réseau tournent déjà hors UI.
- **C. Notifications système natives** (macOS `osascript`/`UNUserNotification`)
  au lieu du bip rodio → permet B et supprime le thread audio par bip.
- **D. Textures emoji en atlas** (P2-1) + `TextureOptions::NEAREST` pour les
  très petites tailles si le rendu reste correct.
- **E. Démarrage** : icône PNG 132 Ko décodée au boot (`src/ui/mod.rs:449`) —
  pré-réduire l'asset à la taille réellement utilisée ; différer
  `install_image_loaders` tant qu'aucun média distant n'est affiché.
- **F. `messages.json` pretty** : disparaît avec SQLite ; sinon `to_string`.
- **G. Compression du binaire** : après P1-3, `strip` + éventuellement
  `opt-level="s"` sur les crates image/resvg via
  `[profile.release.package."*"]`.
- **H. Étendre le serveur chat au framing préfixé** (P2-3) permet aussi de
  supprimer le timeout de lecture de 5 s et le `take(MAX+1)` par connexion.
- **I. Discovery** : n'émettre `PeerDiscovered` que sur changement d'état
  (nouveau pair / adresse changée), un simple `touch` de timestamp sinon.

---

## 11. Questions restantes

1. **Valeur disque de la baseline** : la capture d'écran n'est pas parvenue à
   l'audit — reporter la valeur en §0 pour objectiver le gain du GC média
   (P1-6).
2. **Plafond de rétention média** : 2 Go par défaut ? Configurable dans les
   Paramètres ?
3. **Renderer wgpu (§10-A)** : mener l'expérience de mesure, ou rester sur
   Glow tant que P1-1 n'est pas fait (le repaint permanent domine largement le
   choix du renderer) ? Recommandation : mesurer après l'étape 2.
4. **Mode tray (§10-B)** : souhaité pour la v1 « toujours ouverte », ou plus
   tard ? C'est le plus gros levier GPU/CPU pour une app d'arrière-plan.
