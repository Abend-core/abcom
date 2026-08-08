# Audit qualité — abcom

> Checklist complète des améliorations pour un projet propre au maximum.
> **Révisé le 8 août 2026, branche `dev`** (passes précédentes : 7 juillet, 5 août 2026).
> Priorités : 🔴 important (correctif ou dette bloquante) · 🟠 recommandé · 🟢 confort/finition.
> Chaque point référence les fichiers concernés ; cocher au fur et à mesure.
>
> **Un plan d'exécution détaillé et séquencé — pensé pour être déroulé pas à pas —
> est dans [`PLAN-MAINTENABILITE.md`](PLAN-MAINTENABILITE.md).**

### Revérification du 8 août 2026 (branche `dev`)

Métriques recomptées sur le code actuel :

| Signal | 07/07 | 05/08 | 08/08 | État |
|--------|-------|-------|-------|------|
| `unwrap`/`expect`/`panic!` hors tests | 62 | ~7 | **7** | ✅ résiduels justifiés |
| dont `lock().unwrap()` | 55 | 0 | **0** | ✅ `lock_safe` |
| `eprintln!`/`println!` de prod | 34 | 0 | **0** | ✅ `tracing` |
| `#[allow(dead_code)]` | 16 | 4 | **4** (justifiés) | ✅ |
| Tests | 259 | 257 | **299** (296 unitaires + 2 bin + 1 e2e) | ✅ |
| `clippy -D warnings` / `fmt --check` | vert | vert | **vert** | CI bloquante OK |
| Champs `AbcomApp` (god-struct) | 90 | 46 | **48** en 6 sous-structs | ✅ |
| Version `Cargo.toml` | `0.0.1` | `0.0.1` | **`1.0.0-beta.1`** + MSRV | ✅ |
| MSRV déclarée et testée en CI | ✗ | ✗ | **1.95** (job `msrv`) | ✅ |
| Renderer | Glow (OpenGL émulé) | Glow | **wgpu (Metal natif)** | ✅ mesuré |
| Contexte GPU au repos | 42,4 Mo | — | **3,9 Mo** | ✅ mesuré |
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
   tranchée. Chaque étape validée par `fmt` + `clippy -D warnings` + tests, et
   l'application relancée réellement après la bascule du renderer.

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
- [ ] 🟠 Découper les gros fichiers UI : `chat_panel.rs` (1 589 lignes),
  `input_bar.rs` (1 133), `ui/mod.rs` (1 007), `markdown.rs` (987),
  `composer/mod.rs` (908). Extraire le rendu d'une ligne de fil, la barre de
  survol, les modales (le modèle `composer/` est à répliquer). **Reste le plus
  gros chantier de maintenabilité ouvert.**
- [x] 🟠 Dédupliquer le scan d'emojis — **fait (05/08, P5)** : `match_emoji_at`.
- [x] 🟠 Centraliser les constantes visuelles dupliquées — **fait (05/08, P5)** :
  `ui/theme.rs`.
- [ ] 🟠 i18n : **172 appels `self.tr(fr, en)`** dispersés dans l'UI, et plusieurs
  rendus contournent `tr` avec des `match language` locaux
  (`chat_panel.rs::day_divider_label`, `render_message_body`). Centraliser les
  chaînes (table de clés) pour pouvoir ajouter une langue sans toucher 30 fichiers.
- [ ] 🟢 Homogénéiser la gestion d'erreurs : `anyhow` peu exploité hors
  `main.rs`/`klipy.rs` ; `app/`/`network/` mélangent `Option`, `std::io::Error` et
  silences (`let _ = …`). Définir une politique (erreurs typées dans `network`,
  `anyhow` au bord).
- [x] 🟢 Documenter la convention de tests `src/tests/*.rs` raccordés par
  `#[path = …] mod tests;` — **fait (08/08)** : section dédiée dans
  `docs/07-developpement.md`, avec les deux exceptions assumées (modules à tests
  courts en inline, `tests/p2p_e2e.rs` externe).

## 3. Architecture

- [ ] 🟠 `ui/events.rs::process_events` mélange encore réception réseau, mutation
  d'état et politique de notification (sons, tray, focus). `app/conversation.rs`
  et `ui/outbound.rs` ont sorti une partie de la logique d'envoi ; la partie
  réception/notification reste à extraire pour être testable sans UI.
- [ ] 🟠 Le mutex global `AppState` est verrouillé/déverrouillé plusieurs fois par
  frame et par événement (`drop(s)`/`relock` manuels dans `events.rs`). Envisager :
  file de commandes vers un unique propriétaire de l'état, ou au minimum
  documenter l'ordre de verrouillage.
- [ ] 🟢 `klipy.rs` (API externe) vit à la racine de `src/` à côté de `app/`,
  `network/`, `ui/` — le déplacer dans `services/` pour clarifier les couches.
- [x] 🟠 `AbcomApp` god-struct de 90 champs — **fait (05/08, P6)** : 6 sous-structs.
- [x] 🟠 `network/sender.rs` : sept boucles d'émission quasi identiques —
  **fait (07/08)** : une seule file générique `NetworkSendRequest` → un worker par
  pair → `ConnectionPool` (54 lignes au lieu de ~150, six canaux supprimés).
- [x] 🟠 `ui::run` prenait 14 paramètres positionnels — **fait (07/08)** :
  regroupés dans `ui::UiRuntimeChannels`.
- [ ] 🟢 Regrouper l'intégration bureau dans un module `platform/` : `notify.rs`
  et `autostart.rs` à la racine de `src/`, `tray.rs` dans `ui/`, bascule Dock
  macOS dans `ui/mod.rs`.
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
- [ ] 🟠 Pas de **file d'attente hors-ligne** : un message vers un pair hors ligne
  (ou un membre de salon absent) est perdu — seul le 1-à-1 a un `pending` avec
  retry, borné à la session. Définir la sémantique voulue (stocker et réémettre à
  la reconnexion ?) et l'implémenter ou la documenter.
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
- [ ] 🔴 **Le username n'est pas lié à la clé au niveau découverte (S3)** :
  `DiscoveryPacket` annonce `username` + `pubkey` en clair, sans preuve de
  possession. À la **première** rencontre (avant tout épinglage TOFU), la victime
  peut épingler la mauvaise clé. **Documenté (08/08)** dans le modèle de menace
  comme limite explicite, avec la parade (vérification d'empreinte hors-bande,
  passphrase de salon). **Reste à faire** : signer l'annonce avec la clé privée
  pour fermer réellement le trou.
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
  (licences, sources, doublons de crates) avec [`deny.toml`](deny.toml).
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
- [ ] 🟢 Messages identifiés par un hash **FNV-1a non cryptographique**
  (`app/receipts.rs::message_hash`) : un pair authentifié peut cibler un hash
  forgé. **Documenté (08/08)** comme limite ; évaluer un identifiant aléatoire
  porté par le message (le champ `nonce` existe déjà).

## 6. Persistance & données

- [x] 🟠 Sauvegardes de migration JSON conservées indéfiniment — **fait (08/08)** :
  `storage::purge_legacy_backups` supprime `*.json.bak` (et
  `messages.json.bak.<epoch>`) au-delà de 30 jours, à chaque ouverture de la base.
  Testé.
- [ ] 🟠 `read_counts` compte des **nombres de messages lus** : après
  `clear_conversation_history` ou la purge du ring-buffer, le compte peut désigner
  un ensemble différent de messages. Baser le « lu jusqu'à » sur un rowid/hash de
  dernier message lu.
- [ ] 🟢 Aucune maintenance de la base : ni `VACUUM` périodique, ni contrôle de
  taille (l'historique croît sans limite). Ajouter une commande de compaction et,
  optionnellement, une rétention configurable.
- [ ] 🟢 Pas d'export/sauvegarde de l'historique : une commande d'export
  JSON/texte par conversation serait cohérente avec l'esprit local-first.

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
- [ ] 🟠 **Décoder les emojis paresseusement** : les 323 PNG sont décodés et
  téléversés en GPU **au lancement** (`spawn_emoji_decoder`) même si le sélecteur
  n'est jamais ouvert — ~6,7 Mo GPU + 6,5 Mo CPU + latence de démarrage.
  *Non fait délibérément (08/08)* : le couple `emoji_map: HashMap<String, usize>`
  + `emoji_textures: &[(String, TextureHandle)]` traverse une vingtaine de sites
  de rendu en emprunt partagé ; le décodage à la demande impose une cache à
  mutabilité intérieure et la réécriture de ces signatures. Le rendu des emojis
  est très visible : à faire dans une passe dédiée, avec vérification à l'écran.
- [x] 🟠 **Rendre la RAM au système sur repli tray** — **fait (08/08)** :
  mimalloc en allocateur global + `mi_collect(true)` appelé explicitement dans
  `hide_to_tray`. Sans cet appel, l'allocateur gardait les pages et le RSS ne
  bougeait pas malgré la libération des textures.
- [ ] 🟠 Sur repli tray, envisager de **libérer aussi le contexte graphique**
  plutôt que de garder ~42 Mo d'`IOAccelerator` pendant que l'app veille le réseau.
- [ ] 🟢 Runtime tokio en **2 worker threads** (`main.rs`) pour une charge purement
  I/O : un runtime `current_thread` suffirait probablement — à mesurer.

### 7b. Chemins chauds & rafraîchissement

- [x] 🟠 `composer_caret_positions` reconstruit à chaque frame — **fait (08/08)** :
  mémoïsation par (texte, taille d'emoji, largeur, densité de pixels) dans la
  mémoire d'egui, fenêtre glissante de 4 mesures (la frame en demande deux, le
  clic/glisser une troisième). Signature testée.
- [x] 🟠 `unread_count`/`mark_conversation_read` re-scannaient tous les messages
  pour chaque conversation — **fait (08/08)** : cache **dérivé** (un seul parcours
  par génération de contenu) plutôt que des compteurs maintenus à la main, qui
  auraient pu se désynchroniser à la purge du ring-buffer. Testé.
- [ ] 🟢 Consigner dans la doc les mesures qui fondent les seuils de repli
  (`snapshot.rs::COLLAPSE_*`, `composer::MAX_INPUT_CHARS`) et les re-vérifier à
  chaque montée de version egui.
- [ ] 🟢 Le cache de textures médias n'a pas de borne d'éviction explicite hors
  GIFs — vérifier le comportement sur un long historique d'images.
- [x] 🟠 Thread de stockage : une commande à la fois, **un commit WAL par message**
  — **fait (08/08)** : les `InsertMessage` en attente sont drainés (`try_recv`) et
  appliqués dans **une seule transaction** (lot borné à 256). L'ordre des autres
  commandes est préservé : celle qui interrompt le lot est différée, jamais
  réordonnée. Testé.
- [ ] 🟢 **Rafraîchissement intelligent — déjà solide, à préserver.** `update`
  court-circuite tout rendu fenêtre cachée, les caches dérivés ne se
  reconstruisent que sur changement de génération. Point de vigilance :
  `request_repaint` est appelé depuis ~14 endroits — vérifier qu'aucun ne
  maintient un repaint continu à 60 fps hors animation réelle.
- [ ] 🟢 egui recalcule le layout du fil visible à chaque frame de repaint :
  vérifier que `stick_to_bottom` + fenêtrage borne bien le nombre de lignes
  réellement mises en page sur un très long historique déroulé.

## 8. Tests

- [x] 🟠 Aucun test **bout-en-bout multi-processus** — **fait (07/08)** :
  `tests/p2p_e2e.rs` monte deux piles réseau complètes et vérifie l'échange
  authentifié de bout en bout (le crate est exposé en lib pour cela).
- [ ] 🟠 Mesurer la couverture en CI (`cargo llvm-cov`) : 295 tests passent mais
  aucune visibilité sur les zones non couvertes — le rendu UI notamment, où vivent
  la plupart des régressions récentes (survol, pagination, curseur).
- [ ] 🟢 Ajouter des bancs `criterion` pour les chemins chauds (parse markdown,
  `message_hash`, reconstruction du snapshot).
- [ ] 🟢 Tests de propriété (proptest) sur `message_hash` et sur les opérations de
  curseur du composeur (invariants UTF-8).

## 9. CI/CD & outillage

- [x] 🟠 CI seulement sur `ubuntu-latest` — **fait (07/08)** : job `platform-check`
  (`macos-latest`, `windows-latest`) sur `dev` **et** `main`.
- [x] 🟠 Pas de pipeline de **release** — **fait (08/08)** :
  [`release.yml`](.github/workflows/release.yml) construit les trois cibles sur tag
  `v*`, publie une GitHub Release avec les archives et `SHA256SUMS.txt`, et marque
  les préversions comme telles. **La signature macOS reste absente** (aucun
  certificat dans les secrets) : la limite est écrite en tête du workflow et dans
  la note de release.
- [x] 🟢 Fixer une MSRV et la tester en CI — **fait (08/08)** :
  `rust-version = "1.95"` + job `msrv` qui lit la valeur depuis `Cargo.toml`.
  **À noter** : la contrainte ne vient pas du code d'abcom mais du build script de
  `libsqlite3-sys` (via `rusqlite` 0.40), qui échoue dès 1.94 — la MSRV est donc
  aujourd'hui égale au dernier stable, et pourra redescendre.
- [ ] 🟢 Finaliser les githooks partagés (branche `task/shared-githooks` restée
  ouverte) pour aligner le hook local `clippy` sur la CI.
- [x] 🟢 `cargo outdated` périodique — **fait (08/08)** :
  [`dependencies.yml`](.github/workflows/dependencies.yml), mensuel, rapport
  `cargo outdated` + `cargo audit` sans blocage.

## 10. Documentation

- [x] 🟠 Mettre à jour `docs/05-fonctionnalites.md` et le CHANGELOG — **fait
  (07-08/08)** : `1.0.0-beta.1` publiée, section « Non publié » alimentée.
- [ ] 🟠 `docs/08-historique-et-audits.md` : y ajouter la présente passe (08/08) et
  archiver l'ancien processus.
- [ ] 🟢 README : badges CI, capture d'écran à jour, « Quick start » 3 lignes.
- [ ] 🟢 Documenter le format exact du protocole (paquets JSON, framing 64 Ko,
  `MAX_LOGICAL_MESSAGE`) dans `docs/03-reseau-et-securite.md` — la doc décrit
  l'intention et les constantes, pas encore le format binaire complet.
- [ ] 🟢 Ajouter un `CONTRIBUTING.md` court pointant le workflow de branches et la
  convention de tests (désormais documentée dans `07-developpement.md`).
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
- [ ] 🟢 Tester réellement `scripts/install-windows.ps1` et `contrib/` sur les OS
  cibles ; noter la matrice de support dans le README.
- [x] 🟢 `panic = "abort"` en release + absence de logging fichier = crash
  silencieux — **fait (08/08)** : hook de panique qui écrit
  `<données>/last-panic.txt` (version, horodatage, cause) avant d'abandonner.

## 12. UI / UX

- [ ] 🟠 Vérifier le **thème clair** : un sélecteur système/sombre/clair existe
  mais une grande partie des couleurs du fil, de la sidebar et de la barre de
  saisie sont codées en dur pour un fond sombre.
- [ ] 🟢 Navigation clavier au-delà du composeur : passer d'une conversation à
  l'autre, atteindre la recherche d'emoji, fermer les popups à l'Échap de manière
  homogène.
- [ ] 🟢 Accessibilité : vérifier que les boutons peints portent un libellé
  lisible par lecteur d'écran (AccessKit).
- [ ] 🟢 États vides : conversation sans message, pair hors ligne, salon vide —
  harmoniser le ton et proposer une action.

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

**🔴 Ouverts :**

| # | Chantier | Pourquoi il reste ouvert |
|---|----------|--------------------------|
| S3 | Signature de l'annonce de découverte (première rencontre TOFU) | Changement de protocole ; documenté comme limite, parade décrite (empreinte hors-bande, passphrase) |

**🟠 Ouverts, par ordre de valeur :**

1. Découper `chat_panel.rs` (1 589) / `input_bar.rs` (1 133) / `ui/mod.rs`
   (1 007) en sous-modules (§2) — **le vrai frein restant** à la maintenabilité.
2. Sémantique hors-ligne : que faire d'un message envoyé à un pair absent (§4).
3. Extraire la logique de `process_events` hors de l'UI (§3).
4. Décodage paresseux des emojis (§7a) — le dernier gros poste mémoire, laissé
   de côté faute de pouvoir valider le rendu à l'écran dans cette passe.
5. Montée **egui/eframe 0.31 → 0.36** — voir l'encadré ci-dessous.
6. i18n centralisée (§2), couverture de tests en CI (§8), thème clair (§12).

### Montée egui 0.36 : évaluée, volontairement non appliquée

La tentative a été faite et mesurée : **23 erreurs de compilation** sur 8
fichiers. Le compte est trompeur — ce ne sont pas des renommages :

- `eframe::App` change de forme : `update(&mut self, ctx, frame)` disparaît au
  profit de `ui(&mut self, ui, frame)` (+ `logic()` pour les passes sans rendu) ;
- `TopBottomPanel`/`SidePanel` fusionnent dans un `Panel` unique qui s'affiche
  **dans un `Ui`** et non plus depuis le `Context` : toute la racine de l'arbre
  d'affichage change de nature, et `ctx` est passé en profondeur dans une
  douzaine de fonctions de rendu ;
- l'API popups/menus est refondue (`popup_below_widget`, `close_menu`,
  `toggle_popup` supprimés) — or c'est exactement là que vivent les régressions
  récentes du projet (barre de survol, sélecteur de réactions) ;
- divers : `ctx.style()`/`ctx.screen_rect()` retirés, `IMEOutput` gagne des
  champs, `raw_scroll_delta` disparaît.

Ces changements sont **sémantiques, pas syntaxiques** : ils compilent puis se
voient à l'écran. Les valider demande de reprendre une à une la dizaine de
surfaces de l'application (fil, barre latérale, composeur, pickers emoji/GIF,
paramètres, modales, visionneuse). À faire dans une passe dédiée, avec
validation visuelle — pas en même temps qu'une mise à jour de dépendances.

Toutes les **autres** dépendances sont à jour, y compris sept montées majeures
(`dirs` 6, `socket2` 0.6, `ehttp` 0.7, `rfd` 0.17, `rodio` 0.22, `resvg` 0.48,
`objc2` 0.6 / `objc2-*` 0.3).

**✅ Fermés lors des trois passes :** S2, R1, R2, R3, R4, unification des
expéditeurs, handshake hors verrou, éviction du pool, garde-fou de taille
générique, delta des accusés de lecture, ré-appairage TOFU, collage hors `/tmp`,
ACL Windows de la clé, purge des sauvegardes de migration, transactions de lot
SQLite, persistance des accusés, hook de panique, métriques de session,
nettoyage d'arrêt, renderer wgpu, mimalloc, mémoïsation du caret, compteurs de
non-lus dérivés, `cargo audit`/`deny` sur `dev`, MSRV, pipeline de release,
veille de dépendances, versionnage Cargo, licence, documentation (scripts,
tests, découverte, modèle de menace, renderer).

---

*Audit établi en plusieurs passes de vérification, chaque constat recoupé avec le
code source (métriques recomptées, chemins de fichiers vérifiés) et, pour la
mémoire, avec des mesures réelles (`vmmap`/`ps`). Les affirmations infirmées par
le code ont été retirées ou corrigées en cours de route.*
