> [🏠 Accueil](../README.md) > [🛠 Plan d'optimisation](2026-07-plan-optimisation.md)

> 📅 **Généré le** : 2026-07-04 — plan d'exécution de l'audit [06-audit-performance.md](2026-07-audit-performance.md)
> 🔖 **Baseline** : RSS 443 Mo · **CPU 22 % au repos · GPU 9,7 % · 15 threads** (Moniteur d'activité, capture utilisateur)
> 🔄 **Décision produit** : les GIFs restent en **HD dans le fil** (pas de variante md) — le bornage mémoire passe par le gel hors écran + l'éviction, pas par la résolution.

# Plan d'optimisation — exécution

Phases A/B/C : implémentées dans la PR courante. Phase D (SQLite + pagination
sur base) : PR suivante, prérequis posés ici. Phase E : optionnelle, à mesurer.

## ✅ Résultats mesurés après implémentation A/B/C (2026-07-04, build release)

| Axe | Avant | Après | Méthode |
|---|---|---|---|
| CPU au repos | 22 % | **~0,6 %** (0,19 s de CPU sur 30 s, pcpu instantané 0,0 %) | `ps -o time` delta |
| RSS au repos | 443 Mo | **~156 Mo** (fenêtre ouverte, emojis chargés, sans GIF affiché) | `ps -o rss` |
| Threads | 15 | **8** | `ps -M` |
| Écriture disque / message | historique complet, thread UI | aucune (débounce 2 s, thread dédié, flush à la fermeture) | code |
| Coût par frame | O(historique) parse+hash+verrous | O(visible), zéro verrou au repos | code |
| Disque `media/` | illimité | orphelins purgés + plafond 2 Go au démarrage | code |

Reste à mesurer en usage réel : RSS après navigation GIF intensive (attendu :
borné par le gel hors écran + `forget_image`), débit d'un transfert > 1 Go.

➡️ Suite : [08-seconde-passe-et-securisation.md](2026-07-seconde-passe-et-securisation.md)
— seconde passe d'audit (reste à faire, nouvelles détections) et plan de
chiffrement du transport.

⚠️ Constat annexe relevé pendant la vérification (pré-existant, hors scope) :
les annonces d'avatar peuvent dépasser `MAX_PACKET_SIZE` (64 Ko) et sont alors
rejetées par le serveur (`[network] Paquet trop volumineux (65537 bytes)`) —
un avatar PNG 256×256 sérialisé en tableau JSON d'octets dépasse facilement
64 Ko. À corriger avec la Phase D (avatars en BLOB / framing dédié).

---

## Phase A — Extinction du CPU/GPU au repos (quick wins)

### A1. Profil de build & dépendances
- `Cargo.toml` : ajouter `[profile.release]` → `lto="thin"`,
  `codegen-units=1`, `strip=true`, `panic="abort"`.
- `tokio` : remplacer `features=["full"]` par
  `["rt-multi-thread","net","time","sync","io-util","fs","macros"]`.
- `src/main.rs` : `worker_threads(2)` sur le builder du runtime.

### A2. Réveil de l'UI par événement (supprime le repaint 500 ms)
- Nouveau module `src/notify.rs` : `UiSender<T>` = `mpsc::Sender<T>` +
  `Arc<OnceLock<egui::Context>>` ; `send()/try_send()` fait suivre puis
  `ctx.request_repaint()`. `set_context()` appelé à la création de l'app
  (closure `eframe::run_native`).
- Signatures migrées vers `UiSender<AppEvent>` : `discovery::run`,
  `network::run_server`, `network::run_media_sender`,
  `network::run_media_server` (+ `UiSender<MediaStreamOffer>` pour les offres).
- `src/ui/mod.rs::update()` : remplacer `request_repaint_after(500ms)` par un
  **fallback adaptatif** calculé : expiration de la notification (3 s),
  indicateur de frappe actif (3 s), sinon 5 s (tick `periodic_tasks`).
  Les animations GIF continuent de fonctionner : les loaders egui demandent
  eux-mêmes le repaint pour les images animées **peintes**.

### A3. Progression média : throttle
- `src/network/media_stream.rs` : émettre `MediaProgressed` au plus toutes
  les **100 ms** (Instant par transfert) + toujours l'événement final/échec.
  Supprime ~16 000 événements/Go et le plafonnement du débit par la boucle
  de rendu (canal mpsc 256).

### A4. Micro-fixes UI
- `apply_theme_preference` : `set_visuals` uniquement quand la préférence
  effective change (mémoriser le dernier mode appliqué).
- `update()` : lire `ctx.input(focused)` une seule fois.
- `add_message` : suppression du `msg.clone()` inutile.

### A5. Audio pérenne
- `src/ui/sound.rs` : thread audio unique (lancé au premier bip) qui garde
  l'`OutputStream` ouvert et rejoue sur commande via un canal `std::mpsc` —
  plus de création de thread + périphérique par notification.

## Phase B — Coût par frame indépendant de l'historique

### B1. Compteur de génération
- `AppState.data_generation: u64`, incrémenté par : `add_message`,
  `toggle_reaction`, `apply_reaction_event`, `mark_message_read/acked/sent`,
  `mark_conversation_read`, `set_peer_avatar`, `set_peer_alias`, `add_peer`,
  `cleanup_inactive_peers` (si changement), `set_user_typing`,
  `clear_typing_if_old` (si changement), `remove_media_message`,
  `clear_conversation_history`.

### B2. Cache du fil (`src/ui/chat_panel.rs`)
- Struct `ChatCache { generation, conversation, today, rows: Vec<ChatRow> }`
  reconstruite **uniquement** quand (génération | conversation | jour) change.
- `ChatRow` pré-calcule : message, hash, `starts_group`, libellé de séparateur
  de jour, heure d'en-tête, couleur du nom, accusés (✓/✓✓), citation résolue
  (auteur + snippet + media id), réactions (copie), nom d'affichage.
- **Markdown memoïsé** dans `HashMap<u64, Arc<[MarkdownBlock]>>` persistant
  (clé = hash) : une reconstruction de cache ne re-parse pas les messages
  déjà vus. `render_message_markdown` scindé en parse (caché) + rendu.
- Le rendu par frame ne prend plus **aucun verrou** `AppState` (hors clic).

### B3. Cache latéral & saisie
- `SidebarCache { generation, peers, unread, aliases, groups }` — les
  compteurs non-lus ne sont recalculés qu'au changement de génération.
- `input_bar` : présence/typing lus depuis le même snapshot.

### B4. Fenêtrage du fil (pagination Discord en mémoire)
- Rendu limité aux `visible_count` derniers messages (100 au départ).
- Scroll proche du haut (< 400 px) → `visible_count += 100` avec
  **compensation d'offset** (delta de hauteur de contenu réappliqué à la
  frame suivante) : pas de saut visuel, pas de bouton.
- Réinitialisé au changement de conversation. Prépare la Phase D (la source
  deviendra une requête SQLite au lieu du Vec en mémoire).

## Phase C — Bornage mémoire & disque

### C1. GIFs (HD conservé — décision produit)
- **Gel hors écran** : la ligne réserve son rectangle (dimensions API) ; le
  widget `Image::from_uri` n'est émis que si le rect **intersecte** le
  viewport → un GIF s'anime dès son premier pixel visible, et un GIF hors
  écran ne décode rien, n'anime rien, ne déclenche aucun repaint.
- Même traitement dans la grille du **picker**.
- **Éviction** : `ctx.forget_image()` sur les aperçus à la fermeture du
  picker et sur les anciens items quand une recherche remplace le feed
  (`GifFeed::fire(replace=true)`).

### C2. Images reçues
- `load_media_texture` : downscale à **1024 px max** (côté long) avant
  création de texture (le fil affiche ≤ 320×260).
- `media_textures` : **LRU 32 entrées** (ordre d'accès, éviction du plus
  ancien ; les handles droppés libèrent la texture GPU).
- Visionneuse : texture **pleine résolution dédiée**, chargée à l'ouverture
  et libérée à la fermeture.

### C3. GC du cache disque `media/`
- Au démarrage (thread détaché) : suppression des fichiers **orphelins**
  (non référencés par l'historique), puis application d'un **plafond 2 Go**
  (suppression par ancienneté mtime).

### C4. Purge des maps d'état
- Au drain du ring-buffer (`add_message` > 500) : retirer de `reactions`,
  `read_receipts`, `pending_messages` les hash disparus ; compaction de
  `reactions.json` au même moment.

### C5. Persistance débouncée (interim avant SQLite)
- `add_message`/réactions/accusés ne font plus d'I/O : flag `dirty_*`.
- `periodic_tasks` (≥ 2 s) : snapshot (clone des structures, petites) puis
  **écriture dans un thread détaché**, JSON **compact** (`to_string`).
  Flush final à la fermeture (`eframe::App::on_exit` → `save_all_now`).
- Perte max 2 s en cas de crash : acceptable en attendant la Phase D.

## Phase D — SQLite + pagination sur base (PR suivante)

Schéma et bénéfices : voir [06-audit-performance.md §7](2026-07-audit-performance.md).
1. Dépendance `rusqlite` (feature `bundled`), ouverture WAL dans le thread de
   persistance créé en C5 (le canal d'écriture existe déjà).
2. Migration au premier lancement : import des JSON → `abcom.db`, JSON
   renommés `.bak`.
3. Remplacement des `save_*/load_*` par des requêtes ; suppression du
   ring-buffer 500 ; `unread` par requête indexée (cache B3 conservé).
4. B4 branché sur la base : keyset pagination
   (`WHERE (ts_epoch,id) < … LIMIT 100`) + élagage de la fenêtre mémoire
   (~400) avec `forget_image` des GIFs élagués.
5. Avatars en BLOB (suppression de `peer_avatars.json`), table `media`
   (GC LRU transactionnel remplaçant C3).

## Phase E — Optionnel, à mesurer après A/B/C
- Renderer `wgpu` (Metal) vs Glow : trancher au `powermetrics` une fois le
  repaint permanent éliminé (A2).
- Mode « barre de menus » : fenêtre fermée = zéro rendu.
- Connexion TCP persistante par pair (framing longueur-préfixée déjà utilisé
  par `media_stream`).
- Atlas emoji (une texture au lieu de 323).

---

## Critères d'acceptation (protocole complet : audit §6)

| Axe | Avant | Cible après A/B/C |
|---|---|---|
| CPU repos (focalisé, sans GIF visible) | 22 % | ~0 % |
| GPU repos | 9,7 % | ~0 % |
| RSS après 3 pages de picker GIF fermé | croissance sans retour | retour proche baseline |
| Threads | 15 | ≤ 8 |
| Écriture disque par message reçu | réécriture historique complet | aucune (débounce ≥ 2 s) |
| Frame avec 500 messages | O(n) parse+hash+verrous | O(visible), 0 verrou |
| Disque `media/` | croissance illimitée | borné 2 Go + orphelins purgés |
