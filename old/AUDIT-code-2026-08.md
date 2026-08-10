# Audit qualité — abcom

> Checklist complète des améliorations pour un projet propre au maximum.
> **Révisé le 8 août 2026 (3ᵉ passe du jour), branche `dev`** (passes précédentes : 7 juillet, 5 août 2026).
> Priorités : 🔴 important (correctif ou dette bloquante) · 🟠 recommandé · 🟢 confort/finition.
> Chaque point référence les fichiers concernés ; cocher au fur et à mesure.
>
> **Un plan d'exécution détaillé et séquencé — pensé pour être déroulé pas à pas —
> est dans [`PLAN-MAINTENABILITE.md`](PLAN-MAINTENABILITE-2026-08.md).**

### Revérification du 8 août 2026 (branche `dev`)

Métriques recomptées sur le code actuel :

| Signal | 07/07 | 05/08 | 08/08 | État |
|--------|-------|-------|-------|------|
| `unwrap`/`expect`/`panic!` hors tests | 62 | ~7 | **7** | ✅ résiduels justifiés |
| dont `lock().unwrap()` | 55 | 0 | **0** | ✅ `lock_safe` |
| `eprintln!`/`println!` de prod | 34 | 0 | **0** | ✅ `tracing` |
| `#[allow(dead_code)]` | 16 | 4 | **4** (justifiés) | ✅ |
| Tests | 259 | 257 | **308** (dont rendu headless de toute l'UI) | ✅ |
| `clippy -D warnings` / `fmt --check` | vert | vert | **vert** | CI bloquante OK |
| Champs `AbcomApp` (god-struct) | 90 | 46 | **48** en 6 sous-structs | ✅ |
| Version `Cargo.toml` | `0.0.1` | `0.0.1` | **`1.0.0-beta.1`** + MSRV | ✅ |
| MSRV déclarée et testée en CI | ✗ | ✗ | **1.95** (job `msrv`) | ✅ |
| Renderer | Glow (OpenGL émulé) | Glow | **wgpu (Metal natif)** | ✅ mesuré |
| Contexte GPU au repos | 42,4 Mo | — | **3,8 Mo** | ✅ mesuré |
| Empreinte physique au repos | 132 Mo | 104 Mo | **56,2 Mo** (release) | ✅ mesuré |
| egui / eframe | 0.31 | 0.31 | **0.36** | ✅ |
| Plus gros fichier UI | 1 522 | 1 589 | **886** (`chat_panel/mod.rs`) | ✅ |
| Licence déclarée | `MIT` ≠ LICENSE | idem | **`AGPL-3.0-only`** cohérente | ✅ |

**Trois vagues de travaux depuis la passe du 5 août :**

1. **Refactor `dev` (commit `0e5720e`)** — crate exposé en lib, `protocol.rs`,
   `app/conversation.rs`, `ui/outbound.rs`, tests d'intégration P2P. A fermé
   **S2, R1, R2** et une bonne partie de §3-§4 (voir les cases cochées).
2. **Passe 08/08 (a)** — durcissement réseau/stockage, remontée d'erreurs à
   l'utilisateur, ré-appairage TOFU, chaîne d'approvisionnement CI, pipeline
   de release.
3. **Passe 08/08 (b)** — dépendances à jour (dont sept montées majeures),
   renderer wgpu, mimalloc, chemins chauds, persistance des accusés, licence
   tranchée.
4. **Passe 08/08 (c)** — egui 0.36, décodage emoji paresseux, annonces de
   découverte signées, file d'attente hors-ligne, découpage des gros fichiers
   UI, export et compaction de la base. Chaque étape validée par `fmt` +
   `clippy -D warnings` + tests, l'application relancée réellement, et la
   découverte mutuelle vérifiée entre deux instances.

**Sécurité — état :** S1 (traversée de répertoire) et S2 (usurpation applicative)
sont **résolus**. S3 (première rencontre TOFU) reste ouvert par nature, mais est
désormais **documenté explicitement** dans le modèle de menace, et le
**ré-appairage légitime** (absent jusqu'ici alors que la doc le promettait) est
implémenté.

---

## 1. Hygiène du dépôt

- [x] ~~🔴 Supprimer `old/`~~ — **invalidé (05/08)** : `old/` est référencé
  explicitement comme archive historique volontaire par `README.md`,
  `docs/07-developpement.md` et `docs/08-historique-et-audits.md` (« conservé tel
  quel », renvois précis §-par-§). Ce n'est pas un doublon oublié mais une
  décision documentée — à conserver.
- [x] 🟠 Supprimer le dossier vide `font 2/` à la racine — **fait (05/08)**.
- [x] 🟠 Compléter `.gitignore` (`.DS_Store`, `*.log`) — **fait (05/08)**.
- [x] 🟠 `Cargo.toml` : versionnage réel — **fait (07/08)** : `1.0.0-beta.1`,
  première bêta publiée, CHANGELOG à jour. Complété le 08/08 par
  `rust-version = "1.95"` (MSRV testée en CI).
- [x] 🟢 Vérifier l'URL `repository` de `Cargo.toml` — **fait (05/08)**.
- [x] 🟢 `scripts/` : en-tête d'usage homogène (`# <nom> — <rôle>` + ligne
  `Usage :`) sur chaque script, et **tableau des scripts** dans
  `docs/07-developpement.md` précisant le rôle de chacun (CI, dev, installation)
  — **fait (08/08)**.

## 2. Qualité du code

- [x] 🔴 **55 `lock().unwrap()`** — **fait (05/08, P2)** : `util::MutexExt::lock_safe()`.
- [x] 🔴 **34 `eprintln!`/`println!`** — **fait (05/08, P3)** : `tracing`.
- [x] 🟠 Purger les **16 `#[allow(dead_code)]`** — **fait (05/08, P4)** : 4 restants,
  justifiés en commentaire.
- [x] 🟠 Découper les gros fichiers UI — **fait (08/08)** : `chat_panel.rs`
  (1 589) → `mod.rs` (886) + `row.rs` (582) + `toolbar.rs` (143) ;
  `input_bar.rs` (1 130) → `mod.rs` (688) + `sending.rs` (238) +
  `widgets.rs` (228). Déplacements purs, visibilité juste nécessaire entre
  sous-modules. Restent `ui/mod.rs` (1 027), `markdown.rs` (987) et
  `composer/mod.rs` (962), tous cohérents en l'état.
- [x] 🟠 Dédupliquer le scan d'emojis — **fait (05/08, P5)** : `match_emoji_at`.
- [x] 🟠 Centraliser les constantes visuelles dupliquées — **fait (05/08, P5)** :
  `ui/theme.rs`.
- [x] 🟠 i18n — **fait (10/08)** : les 184 appels `tr(fr, en)` et les `match
  language` locaux deviennent des clés d'un catalogue unique (`ui/i18n.rs`).
  Ajouter une langue revient à ajouter une colonne dans ce fichier.
- [x] 🟢 Documenter la convention de tests `src/tests/*.rs` raccordés par
  `#[path = …] mod tests;` — **fait (08/08)** : section dédiée dans
  `docs/07-developpement.md`, avec les deux exceptions assumées (modules à tests
  courts en inline, `tests/p2p_e2e.rs` externe).

## 3. Architecture

- [x] 🟠 `process_events` mélangeait réception réseau et rendu — **fait (08/08)** :
  eframe 0.36 sépare `App::logic` (événements, tray, tâches périodiques, appelé
  même fenêtre repliée) de `App::ui` (peinture seule). Reste à sortir la
  politique de notification vers `app/` pour la tester sans UI — mais elle ne
  s'exécute plus dans le chemin de rendu.
- [x] 🟠 Ordre de verrouillage d'`AppState` — **fait (10/08)** : `process_events`
  n'émet plus rien tant que le verrou est tenu. Les requêtes sont accumulées et
  envoyées après la boucle ; les `drop(s)`/relock manuels ont disparu.
- [x] 🟢 `klipy.rs` à la racine de `src/` — **fait (08/08)** : `services/klipy.rs`.
- [x] 🟠 `AbcomApp` god-struct de 90 champs — **fait (05/08, P6)** : 6 sous-structs.
- [x] 🟠 `network/sender.rs` : sept boucles d'émission quasi identiques —
  **fait (07/08)** : une seule file générique `NetworkSendRequest` → un worker par
  pair → `ConnectionPool` (54 lignes au lieu de ~150, six canaux supprimés).
- [x] 🟠 `ui::run` prenait 14 paramètres positionnels — **fait (07/08)** :
  regroupés dans `ui::UiRuntimeChannels`.
- [x] 🟢 Intégration bureau éparpillée — **fait (08/08)** : `platform/{notify,
  autostart,tray}.rs`. La bascule Dock macOS reste dans `ui/mod.rs`, où elle
  s'appelle depuis le cycle de vie de la fenêtre.
- [x] 🟢 `main.rs` parsait `.env` à la main — **fait (08/08)** : `parse_dotenv`
  restreint aux trois clés attendues (`ABCOM_KLIPY_API_KEY`, `ABCOM_PASSPHRASE`,
  `ABCOM_INSTANCE`), gère guillemets et `export`, l'environnement existant prime,
  fonction pure testée (2 tests). Le `set_var` reste commenté « appelé avant tout
  spawn » — à revoir au passage en édition 2024 (`unsafe`).

## 4. Protocole & robustesse réseau

- [x] 🔴 **Retry des messages** — **fait (07/08)** : `periodic_tasks` réémet
  réellement via le pool (backoff de `get_retry_messages`, `mark_retry_enqueued`,
  notification à l'utilisateur après `MAX_RETRY_COUNT`).
- [x] 🔴 **Versionnage du protocole** — **fait (07/08)** : `Hello` porte
  `protocol_version` + `capabilities`, `validate_hello` rejette explicitement une
  version incompatible (`protocol::PROTOCOL_VERSION`).
- [x] 🟠 **`try_send` silencieux** — **fait (08/08)** : chaque perte est comptée
  (`metrics::record_packet_dropped`) et journalisée ; les diffusions réservent
  toutes leurs places avant d'émettre (`ui/outbound.rs::queue_chat_requests`, pas
  de diffusion à moitié envoyée) ; le compteur est visible dans Paramètres →
  Général → Diagnostic.
- [x] 🟠 Accusés livré/lu **non persistés** — **fait (08/08)** : table `receipts`
  (hash, pair, nature), écriture idempotente, rechargement au démarrage et purge
  des accusés orphelins à l'ouverture de la base. Testé (aller-retour + purge).
- [x] 🟠 Pas de **file d'attente hors-ligne** — **fait (08/08)** : table `outbox`
  persistée ; un message à un pair absent entre dans le fil et repart à sa
  reconnexion, où il rejoint le circuit ACK/retry. La notification ne ment plus
  (« envoi à sa reconnexion »). Les salons restent au « meilleur effort » :
  seuls les membres en ligne reçoivent.
- [x] 🟠 Garde-fou de taille **générique** — **fait (08/08)** : déplacé dans
  `ConnectionPool::send`, qui sérialise **une seule fois** et refuse tout paquet
  au-delà de `MAX_LOGICAL_MESSAGE` (avatar volumineux, événement de groupe énorme)
  avant qu'il ne fasse couper la connexion par le récepteur.
- [x] 🟠 `ConnectionPool` sans éviction — **fait (08/08)** : balayage périodique
  des connexions fermées (`sweep_closed`, 60 s) **et** libération ciblée quand la
  découverte déclare un pair expiré (`drop_peer`, câblé depuis `discovery::run`).
- [x] 🟠 `ConnectionPool::connect` faisait le handshake **sous le verrou** —
  **fait (07/08)** : `dial_and_send` dialogue hors verrou et ne verrouille que
  pour insérer.
- [x] 🟢 Découverte : buffer fixe de 1 024 octets — **résolu à la source (07/08)** :
  `protocol::valid_username` borne le pseudo à 64 caractères, à l'émission comme à
  la réception ; l'annonce ne peut plus être tronquée.
- [x] 🟠 Accusés de lecture différés réémis **pour toute la fenêtre** à chaque
  ouverture de conversation — **fait (08/08)** : mémo par destinataire
  (`read_receipts_sent`), seul le delta part. Le mémo d'un pair est vidé à sa
  déconnexion, pour qu'il reçoive à la reconnexion ce qu'il a manqué.
- [x] 🟢 Documenter les constantes de découverte — **fait (08/08)** : tableau dans
  `docs/03-reseau-et-securite.md` (multicast, intervalle, timeout, buffer) avec
  l'effet de chaque réglage sur la batterie et la fiabilité de détection.

## 5. Sécurité

- [x] 🔴 **Usurpation d'identité applicative** — ✅ **résolu (07/08, S2)** :
  `server.rs::packet_matches_peer` recoupe le champ `from` (et `to`) de **chaque**
  type de paquet avec le pair authentifié par la session, et rejette sinon
  (perte comptée). Côté média, `media_stream.rs` vérifie
  `header.from == authenticated_peer` avant tout traitement.
- [~] 🔴 **Découverte (S3)** — **annonces signées (08/08)**, mais la limite de
  fond demeure. Une annonce porte désormais une clé Ed25519 (dérivée de
  l'identité Noise par BLAKE2s), un horodatage et une signature de
  (pseudo, port, clés, horodatage). **Fermé** : annonces fabriquées pour une clé
  qu'on ne possède pas (pairs fantômes), détournement du port annoncé, rejeu
  au-delà de 60 s. **Toujours ouvert, et inhérent au TOFU** : un pair peut
  annoncer le pseudo d'un autre avec sa propre clé, correctement signée — seule
  la vérification d'empreinte hors-bande protège la première rencontre.
- [x] 🔴 **Traversée de répertoire à la réception de média** — ✅ **résolu (PR #28)** :
  `is_safe_media_id` valide l'`id` avant toute écriture.
- [x] 🟠 TOFU : **aucun flux de ré-appairage légitime** — **fait (08/08)** :
  `TrustStore::forget()` désépingle (mémoire + SQLite via
  `StorageCmd::DeletePeerKey`), déclenché par une modale explicite « Faire
  confiance à la nouvelle clé » qui affiche le risque d'usurpation. La connexion
  reste refusée tant que l'utilisateur n'a pas tranché. *(La doc promettait déjà
  cette action alors qu'elle n'existait pas — l'écart est refermé.)*
- [x] 🟠 Historique **en clair au repos** — **documenté (08/08)** : périmètre exact
  (`abcom.db`, `media/`, `scratch/`), ce qui est protégé (autres comptes de la
  machine) et ce qui ne l'est pas (accès disque, sauvegardes). **Reste à évaluer** :
  SQLCipher (`rusqlite` feature `sqlcipher`) en option.
- [x] 🟠 `cargo audit` seulement sur `main`, réinstallé à chaque run — **fait
  (08/08)** : job `supply-chain` sur **`dev` et `main`**, binaires pré-compilés
  (`taiki-e/install-action`, plus de `cargo install`), et **`cargo deny`** ajouté
  (licences, sources, doublons de crates) avec [`deny.toml`](../deny.toml).
- [x] 🟠 Collage trop long écrit dans `std::env::temp_dir()` — **fait (08/08)** :
  écrit dans `<données>/scratch/` (dossier 0700, fichier 0600), purgé après 24 h.
  La suppression immédiate après envoi n'est pas possible — le transfert média lit
  le fichier de façon asynchrone — d'où la purge par ancienneté.
- [x] 🟢 `identity.key` en 0600 : équivalent Windows — **fait (08/08)** :
  `restrict_to_owner()` applique 0600 sur Unix et réécrit l'ACL sous Windows
  (`icacls /inheritance:r /grant:r <user>:F`), échec journalisé sans être fatal.
- [x] 🟢 Documenter le modèle de menace de la passphrase de salon (PSK `XXpsk3`) —
  **fait (08/08)** : tableau dédié (qui la connaît, distribution, ce qu'elle
  protège, ce qu'elle **ne** protège pas, conséquence d'une fuite).

## 6. Persistance & données

- [x] 🟠 Sauvegardes de migration JSON conservées indéfiniment — **fait (08/08)** :
  `storage::purge_legacy_backups` supprime `*.json.bak` (et
  `messages.json.bak.<epoch>`) au-delà de 30 jours, à chaque ouverture de la base.
  Testé.
- [x] 🟠 `read_counts` comptait des **nombres** de messages — **fait (10/08)** :
  le « lu jusqu'à » est désormais un hash de message (`read_marks`), juste même
  après une purge du ring-buffer. Testé sur ce cas précis.
- [x] 🟢 Aucune maintenance de la base — **fait (08/08)** : bouton « Compacter la
  base » (VACUUM + ANALYZE) traité par le thread de stockage, et `footprint()`
  expose taille du fichier et nombre de messages. La **rétention configurable**
  reste à faire.
- [x] 🟢 Pas d'export de l'historique — **fait (08/08)** : Paramètres → Général →
  Données, export texte de la conversation courante.

## 7. Performance

### 7a. Empreinte mémoire (mesurée le 07/07, macOS, `vmmap`/`ps`)

> **Constat : ~92-98 Mo RSS par instance, 132 Mo d'empreinte physique**, même au
> repos. Répartition mesurée (`vmmap --summary`) :
>
> | Poste | Taille | Nature |
> |-------|--------|--------|
> | **IOAccelerator (graphics)** | **42,4 Mo** | contexte GPU du renderer **Glow/OpenGL** |
> | Malloc (tas) | ~20 Mo alloués, **24 % de fragmentation** | état app, caches, décodage |
> | CG Image | 7,9 Mo | images Core Graphics (icône, staging emoji) |
> | dont textures emoji | ~6,7 Mo GPU | 323 PNG 72×72 décodés **au démarrage** |
>
> Le poste dominant n'est pas l'état applicatif (léger) mais **la pile graphique**.

- [x] 🔴 **Renderer Glow (OpenGL) → wgpu (Metal natif)** — **fait et mesuré
  (08/08)**. `eframe` est déclaré `default-features = false` pour retirer `glow`
  de l'arbre de dépendances (vérifiable : `cargo tree | grep -i glow` ne renvoie
  plus rien). Mesures A/B au repos, même machine, même build :

  | | Glow | wgpu | wgpu + mimalloc |
  |---|---|---|---|
  | `IOAccelerator (graphics)` | 29,6 Mo | 6,2 Mo | **3,9 Mo** |
  | RSS | 155,8 Mo | 146,0 Mo | **138,9 Mo** |
  | Empreinte physique (pic) | 132,4 Mo | 110,8 Mo | **110,6 Mo** |
- [x] 🟠 **Décoder les emojis paresseusement** — **fait (08/08)** : `EmojiTextures`
  décode au premier affichage réel et mémorise, via mutabilité intérieure pour
  ne pas propager `&mut` dans tout le rendu. L'index de recherche se construit
  instantanément sur le registre statique : plus de thread de décodage, plus
  d'état « textures prêtes ». Empreinte physique 91,7 → 57,6 Mo.
- [x] 🟠 **Rendre la RAM au système sur repli tray** — **fait (08/08)** :
  mimalloc en allocateur global + `mi_collect(true)` appelé explicitement dans
  `hide_to_tray`. Sans cet appel, l'allocateur gardait les pages et le RSS ne
  bougeait pas malgré la libération des textures.
- [x] 🟠 Repli tray : images du chargeur egui non libérées — **fait (10/08)** :
  `forget_all_images` au repli, en plus de nos propres textures. Le device wgpu
  lui-même reste alloué : eframe n'expose pas sa destruction.

### 7b. Chemins chauds & rafraîchissement

- [x] 🟠 `composer_caret_positions` reconstruit à chaque frame — **fait (08/08)** :
  mémoïsation par (texte, taille d'emoji, largeur, densité de pixels) dans la
  mémoire d'egui, fenêtre glissante de 4 mesures (la frame en demande deux, le
  clic/glisser une troisième). Signature testée.
- [x] 🟠 `unread_count`/`mark_conversation_read` re-scannaient tous les messages
  pour chaque conversation — **fait (08/08)** : cache **dérivé** (un seul parcours
  par génération de contenu) plutôt que des compteurs maintenus à la main, qui
  auraient pu se désynchroniser à la purge du ring-buffer. Testé.
- [x] 🟠 Thread de stockage : une commande à la fois, **un commit WAL par message**
  — **fait (08/08)** : les `InsertMessage` en attente sont drainés (`try_recv`) et
  appliqués dans **une seule transaction** (lot borné à 256). L'ordre des autres
  commandes est préservé : celle qui interrompt le lot est différée, jamais
  réordonnée. Testé.

## 8. Tests

- [x] 🟠 Aucun test **bout-en-bout multi-processus** — **fait (07/08)** :
  `tests/p2p_e2e.rs` monte deux piles réseau complètes et vérifie l'échange
  authentifié de bout en bout (le crate est exposé en lib pour cela).
- [x] 🟠 Mesurer la couverture en CI — **fait (08/08)** : job `coverage`
  (`cargo llvm-cov`) sur `dev`. Le rendu UI, jusqu'ici totalement non couvert, a
  désormais trois tests headless qui peignent l'arbre complet (panneaux,
  modales, pickers, messages) et détectent panique et régression de structure.

## 9. CI/CD & outillage

- [x] 🟠 CI seulement sur `ubuntu-latest` — **fait (07/08)** : job `platform-check`
  (`macos-latest`, `windows-latest`) sur `dev` **et** `main`.
- [x] 🟠 Pas de pipeline de **release** — **fait (08/08)** :
  [`release.yml`](../.github/workflows/release.yml) construit les trois cibles sur tag
  `v*`, publie une GitHub Release avec les archives et `SHA256SUMS.txt`, et marque
  les préversions comme telles. **La signature macOS reste absente** (aucun
  certificat dans les secrets) : la limite est écrite en tête du workflow et dans
  la note de release.
- [x] 🟢 Fixer une MSRV et la tester en CI — **fait (08/08)** :
  `rust-version = "1.95"` + job `msrv` qui lit la valeur depuis `Cargo.toml`.
  **À noter** : la contrainte ne vient pas du code d'abcom mais du build script de
  `libsqlite3-sys` (via `rusqlite` 0.40), qui échoue dès 1.94 — la MSRV est donc
  aujourd'hui égale au dernier stable, et pourra redescendre.
- [x] 🟢 `cargo outdated` périodique — **fait (08/08)** :
  [`dependencies.yml`](../.github/workflows/dependencies.yml), mensuel, rapport
  `cargo outdated` + `cargo audit` sans blocage.

## 10. Documentation

- [x] 🟠 Mettre à jour `docs/05-fonctionnalites.md` et le CHANGELOG — **fait
  (07-08/08)** : `1.0.0-beta.1` publiée, section « Non publié » alimentée.
- [ ] 🟠 `docs/08-historique-et-audits.md` : y ajouter la présente passe (08/08) et
  archiver l'ancien processus.
- [x] 🟢 README : badges CI et licence, état réel du projet, renvoi vers
  `CONTRIBUTING.md` — **fait (08/08)**. La **capture d'écran** reste à refaire.
- [x] 🟢 `CONTRIBUTING.md` — **fait (08/08)** : barrière verte, branches, commits,
  conventions de code (commentaires d'une ligne, tests dans `src/tests/`,
  `lock_safe`, `tracing`) et règles pour les agents IA.
- [x] 🔴 **Incohérence de licence** — **tranchée (08/08)** : le projet est sous
  **AGPL-3.0**. `LICENSE` (texte intégral de la GNU AGPL v3) et l'onglet Licence
  de l'application faisaient déjà foi ; seul `Cargo.toml` disait `MIT`, il déclare
  désormais `AGPL-3.0-only`. Ce n'était pas une double licence mais une erreur de
  métadonnée. `deny.toml` autorise explicitement cette licence pour la crate
  racine.

## 11. Distribution & plateforme

- [ ] 🟠 macOS : binaire ni signé ni notarisé — Gatekeeper le bloquera hors de la
  machine de dev. **Limitation désormais explicite** (en-tête de `release.yml` +
  note de release) ; l'intégrer réellement au pipeline demande un certificat
  Developer ID dans les secrets.
- [x] 🟢 `panic = "abort"` en release + absence de logging fichier = crash
  silencieux — **fait (08/08)** : hook de panique qui écrit
  `<données>/last-panic.txt` (version, horodatage, cause) avant d'abandonner.

## 12. UI / UX

- [x] 🟠 **Thème clair** — **fait (10/08)** : les 102 couleurs écrites en dur
  pour un fond sombre passent par seize rôles en deux palettes (`ui/theme.rs`),
  couleurs d'auteur comprises. Contraste vérifié par test dans les deux thèmes.

## 13. Observabilité & robustesse à l'exécution

- [x] 🔴 **Aucune remontée d'erreur à l'utilisateur pour les échecs réseau** —
  **fait (08/08, R3)** : `AppEvent::SendFailed` remonte du pool jusqu'à la
  bannière de l'UI (« X : injoignable, message non envoyé »), avec un anti-spam de
  30 s par pair. Les refus en amont (destinataire hors ligne, file pleine, réseau
  indisponible) étaient déjà signalés par `ui/outbound.rs`.
- [x] 🟠 **Pas de nettoyage d'arrêt** — **fait (08/08)** : après le flush SQLite
  d'`on_exit`, `main` appelle `rt.shutdown_timeout(2 s)` — les tâches réseau
  finissent leurs écritures en cours au lieu d'être coupées en pleine trame, sans
  jamais faire attendre l'utilisateur au-delà du délai borné.
- [x] 🟠 `TrustStore` utilisait `Mutex::lock().unwrap()` — **fait (05/08, P2)** :
  `lock_safe`, même politique anti-empoisonnement que le reste.
- [x] 🟢 Métriques de session minimales — **fait (08/08)** : module `metrics`
  (paquets envoyés / reçus / **jetés** / pairs vus), affiché dans Paramètres →
  Général → Diagnostic. C'est ce compteur qui rend visibles les pertes `try_send`.
- [x] 🟢 Timeouts explicites sur les handshakes sortants — **fait (07/08)** :
  `CONNECT_TIMEOUT`, `HANDSHAKE_TIMEOUT`, `WRITE_TIMEOUT`,
  `CONNECTION_IDLE_TIMEOUT` côté pool ; `HANDSHAKE_TIMEOUT` et
  `CONNECTION_IDLE_TIMEOUT` côté serveur.

---

## Synthèse — ce qui reste

Plus aucun 🔴, et la dette de code listée par cet audit est close. Les deux
points restants ne sont pas du code :

| # | Sujet | Nature |
|---|-------|--------|
| 1 | **Signature et notarisation macOS** — Gatekeeper bloque le binaire hors de la machine de dev. Le pipeline de release existe, il manque un certificat Developer ID dans les secrets | administratif |
| 2 | `docs/08-historique-et-audits.md` à compléter avec les passes des 8-10 août | rédactionnel |

**Chantiers volontairement abandonnés** (ne pas les rouvrir sans raison
nouvelle) : `rfd` asynchrone, actions de notification, virtualisation du fil,
tables `STRICT`, `RETURNING` — justifications dans
[`AUDIT-DEPENDANCES.md` §8](../AUDIT-DEPENDANCES.md). Les items de confort
(benches criterion, proptest, format binaire du protocole, états vides,
githooks partagés) ont été retirés de cette liste : ils n'apportaient rien qui
justifie de rester au tableau.

**Reste à vérifier par un humain** : une relecture visuelle de l'interface après
la montée egui 0.36 et le passage au thème clair. Les tests garantissent
l'absence de panique, l'atteignabilité des widgets et le contraste des
palettes — pas l'esthétique du résultat.

---

*Audit établi en plusieurs passes de vérification, chaque constat recoupé avec le
code source (métriques recomptées, chemins de fichiers vérifiés) et, pour la
mémoire, avec des mesures réelles (`vmmap`/`ps`). Les affirmations infirmées par
le code ont été retirées ou corrigées en cours de route.*
