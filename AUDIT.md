# Audit qualité — abcom

> Checklist complète des améliorations pour un projet propre au maximum.
> **Révisé le 5 août 2026, branche `dev`** (passe initiale : 7 juillet 2026).
> Seconde passe approfondie incluse : lecture ligne à ligne de `network/` (pool,
> découverte, handshake, streaming média), `identity.rs`, `main.rs` et `storage.rs`.
> Priorités : 🔴 important (correctif ou dette bloquante) · 🟠 recommandé · 🟢 confort/finition.
> Chaque point référence les fichiers concernés ; cocher au fur et à mesure.
>
> **Un plan d'exécution détaillé et séquencé — pensé pour être déroulé pas à pas —
> est dans [`PLAN-MAINTENABILITE.md`](PLAN-MAINTENABILITE.md).**

### Revérification du 5 août 2026 (branche `dev`)

Métriques recomptées sur le code actuel — **inchangées** depuis le 7 juillet sauf mention :

| Signal | 07/07 | 05/08 (avant) | 05/08 (après P1-P6) | État |
|--------|-------|----------------|----------------------|------|
| `unwrap`/`expect`/`panic!` hors tests | 62 | 62 | **~7** (hors `lock_safe`) | ✅ P2 |
| dont `lock().unwrap()` | 55 | 55 | **0** | ✅ P2 |
| `eprintln!`/`println!` de prod | 34 | 34 | **0** (`tracing`) | ✅ P3 |
| `#[allow(dead_code)]` | 16 | 16 | **4** (tous justifiés) | ✅ P4 |
| Tests (`#[test]`/`#[tokio::test]`) | 259 | 272 | **257** (dead code purgé) | — |
| `clippy -D warnings` / `fmt --check` | vert | vert | **vert** | CI bloquante OK |
| Champs `AbcomApp` (god-struct) | 90 | 90 | **46** en 6 sous-structs | ✅ P6 |
| Scan emoji dupliqué | 3 copies | 4 copies | **1** (`match_emoji_at`) | ✅ P5 |

**Exécuté le 5 août 2026 : les 6 phases de [`PLAN-MAINTENABILITE.md`](PLAN-MAINTENABILITE.md)
(P1 hygiène, P2 mutex, P3 logging, P4 dead code, P5 thème/dédup, P6 découpage
AbcomApp) — 6 commits sur `dev`, barrière verte (`fmt`+`clippy -D warnings`+`test`)
à chaque étape. Détail des sous-structs P6 : `NetworkChannels`, `EmojiPickerState`,
`GifPickerState`, `ComposerState`, `ModalsState`, `MediaState`.**

**Sécurité — corrigé depuis (à cocher ci-dessous) :** ✅ la traversée de répertoire à la
réception de média est fermée par `is_safe_media_id` (`media_stream.rs`, PR #28) —
**S1 résolu**. Toujours ouverts : S2 (vérif `from` == pair authentifié), R1 (retry
réel, encore un `eprintln!` stub à `events.rs:418`), R2/R3/R4 et toute la dette §2.

---

## 1. Hygiène du dépôt

- [x] ~~🔴 Supprimer `old/`~~ — **invalidé (05/08)** : `old/` est référencé
  explicitement comme archive historique volontaire par `README.md`,
  `docs/07-developpement.md` et `docs/08-historique-et-audits.md` (« conservé tel
  quel », renvois précis §-par-§). Ce n'est pas un doublon oublié mais une
  décision documentée — à conserver.
- [x] 🟠 Supprimer le dossier vide `font 2/` à la racine — **fait (05/08)**.
- [x] 🟠 Compléter `.gitignore` (`.DS_Store`, `*.log`) — **fait (05/08)**.
- [ ] 🟠 `Cargo.toml` : la version est figée à `0.0.1` alors que le projet a un CHANGELOG —
  adopter un versionnage réel (bump à chaque merge sur `main`, tags git).
- [x] 🟢 Vérifier l'URL `repository` de `Cargo.toml` — **fait (05/08)** : corrigée en
  `https://github.com/Abend-core/abcom` (remote réel confirmé via `git remote -v`).
- [ ] 🟢 `scripts/` : préfixer chaque script d'un en-tête d'usage homogène et lister les
  scripts dans `docs/07-developpement.md` (`integration_test.sh` tourne en CI main,
  `QUICK_START_TEST.sh` est un pense-bête interactif — clarifier le rôle de chacun).

## 2. Qualité du code

- [x] 🔴 **55 `lock().unwrap()`** sur le mutex `AppState` — **fait (05/08, P2)** : trait
  `util::MutexExt::lock_safe()` (`unwrap_or_else(|e| e.into_inner())`), remplacement
  mécanique des 55 sites de production.
- [x] 🔴 **34 `eprintln!`/`println!`** de production — **fait (05/08, P3)** : migration vers
  `tracing`/`tracing-subscriber` (niveaux, horodatage, filtre `RUST_LOG`).
- [x] 🟠 Purger les **16 `#[allow(dead_code)]`** — **fait (05/08, P4)** : 12 supprimés
  (dont les modules orphelins entiers `composer/cursor.rs`, `composer/render.rs`,
  `composer/shortcode.rs`, superseded par `emoji_picker.rs`/`composer/mod.rs`), 4 restants
  justifiés par un commentaire (drop `Tray::icon`, `Identity::ephemeral` utilisé par les
  tests, `cleanup_inactive_peers` filet de sécurité, `rename_group` gap fonctionnel UI).
- [ ] 🟠 Découper les gros fichiers UI : `chat_panel.rs` (1 522 lignes),
  `input_bar.rs` (1 164), `markdown.rs` (996), `composer/mod.rs` (927), `ui/mod.rs` (915).
  Extraire par exemple le rendu d'une ligne de fil, la barre de survol, et le popup
  notifications dans des sous-modules (le modèle `composer/` — cursor/text_ops/shortcode/
  render — est à répliquer).
- [x] 🟠 Dédupliquer le scan d'emojis — **fait (05/08, P5)** : les 4 sites
  (`markdown.rs`, `emoji_picker.rs`, `composer/mod.rs` ×2) consomment désormais
  `emoji_picker::match_emoji_at`, point d'entrée unique.
- [x] 🟠 Centraliser les constantes visuelles dupliquées — **fait (05/08, P5)** :
  `ui/theme.rs` regroupe les 3 valeurs réellement dupliquées (`LINE_HEIGHT`,
  `SEPARATOR`, `TEXT_MUTED`). Les teintes proches mais choisies indépendamment
  (gris 140/160/165/190, bleu du récépissé lu) restent en dur à leur point d'usage
  — les fusionner créerait un couplage visuel qui n'existe pas dans le code.
- [ ] 🟠 i18n : `tr(fr, en)` retourne des `&'static str` éparpillés dans tout le code UI,
  et plusieurs rendus contournent `tr` avec des `match language` locaux
  (`chat_panel.rs::show_receipt_detail_button`, `render_message_body`). Centraliser les
  chaînes (table de clés) pour pouvoir ajouter une langue sans toucher 30 fichiers.
- [ ] 🟢 Homogénéiser la gestion d'erreurs : `anyhow` est présent mais peu exploité en
  dehors de `main.rs`/`klipy.rs` ; les couches `app/`/`network/` mélangent `Option`,
  `std::io::Error` et silences (`let _ = …`). Définir une politique (erreurs typées dans
  `network`, `anyhow` au bord).
- [ ] 🟢 Documenter la convention de tests `src/tests/*.rs` raccordés par
  `#[path = …] mod tests;` dans `docs/07-developpement.md` (inhabituelle mais cohérente —
  un nouveau contributeur la cherchera).

## 3. Architecture

- [ ] 🟠 `ui/events.rs::process_events` mélange trois responsabilités : réception réseau,
  mutation d'état, et politique de notification (sons, tray, focus). Extraire la logique
  métier (ACK/receipts, groupes) dans `app/` pour la tester sans UI.
- [ ] 🟠 Le mutex global `AppState` est verrouillé/déverrouillé plusieurs fois par frame
  et par événement (avec des `drop(s)`/`relock` manuels dans `events.rs`, source classique
  de deadlocks à la modification). Envisager : file de commandes vers un unique
  propriétaire de l'état, ou au minimum documenter l'ordre de verrouillage.
- [ ] 🟢 `klipy.rs` (API externe) vit à la racine de `src/` à côté de `app/`, `network/`,
  `ui/` — le déplacer dans un module `services/` ou `net/klipy.rs` pour clarifier les
  couches.
- [x] 🟠 `AbcomApp` était un god-struct de **90 champs** — **fait (05/08, P6)** : éclaté
  en 6 sous-structs (`NetworkChannels`, `EmojiPickerState`, `GifPickerState`,
  `ComposerState`, `ModalsState`, `MediaState`), 46 champs restants à plat. Un commit
  par sous-struct, migration mécanique des ~280 sites d'accès au total. Préalable posé
  pour le découpage des gros fichiers UI (toujours ouvert ci-dessus).
- [ ] 🟠 `network/sender.rs` : **sept boucles d'émission quasi identiques**
  (`run_sender`, `_group`, `_typing`, `_read_receipts`, `_ack`, `_avatar`, `_reaction`)
  + sept canaux et sept spawns dans `main.rs`. Une seule file générique
  `(SocketAddr, NetworkPacket)` supprimerait ~100 lignes et 6 canaux.
- [ ] 🟠 `ui::run` prend **14 paramètres positionnels** (`main.rs:149-164`) — les
  regrouper dans un struct de contexte (erreur de câblage silencieuse garantie sinon).
- [ ] 🟢 Regrouper l'intégration bureau dans un module `platform/` : aujourd'hui
  `notify.rs` et `autostart.rs` vivent à la racine de `src/`, `tray.rs` dans `ui/`,
  et la bascule Dock macOS dans `ui/mod.rs`.
- [ ] 🟢 `main.rs` parse `.env` à la main (`split_once('=')`) : pas de guillemets, pas
  d'échappement, et `set_var` sera `unsafe` en édition 2024. Utiliser `dotenvy` ou
  restreindre explicitement aux deux clés attendues (`ABCOM_KLIPY_API_KEY`,
  `ABCOM_PASSPHRASE`).

## 4. Protocole & robustesse réseau

- [ ] 🔴 **Le retry des messages est un stub** : `ui/events.rs::periodic_tasks` fait
  seulement `eprintln!("[ui] Retry message delivery …")` — les messages 1-à-1 non ACKés
  ne sont **jamais réémis** malgré `PendingMessage.retry_count` et le backoff calculé
  dans `app/receipts.rs::get_retry_messages`. Brancher la réémission réelle via le pool.
- [ ] 🔴 Pas de **versionnage du protocole** : `NetworkPacket` est un JSON sans champ de
  version ni de capacités. Deux builds différents sur le même LAN peuvent se parler sans
  le savoir. Ajouter `proto_version` au Hello et une politique de compatibilité.
- [ ] 🟠 **17 `try_send`** côté UI : quand un canal (256 slots) est plein, le paquet est
  silencieusement jeté (messages, ACKs, réactions…). Au minimum compter et logger les
  pertes ; idéalement, back-pressure ou canal illimité pour les paquets critiques.
- [ ] 🟠 Accusés livré/lu **non persistés** (`read_receipts`/`delivered_receipts` en
  mémoire seule) : au redémarrage, coches et détail « … » repartent de zéro alors que
  l'historique des messages, lui, est persisté. Persister (table dédiée) comme le faisait
  la branche AR avec `receipts.json`.
- [ ] 🟠 Pas de **file d'attente hors-ligne** : un message envoyé à un pair hors ligne
  (ou un membre de salon absent) est perdu — seul le 1-à-1 a un `pending` (sans retry,
  cf. ci-dessus). Définir la sémantique voulue (stocker et réémettre à la reconnexion ?)
  et l'implémenter ou la documenter.
- [ ] 🟠 Le garde-fou de taille à l'envoi (`input_bar.rs::chat_wire_size`, ajouté le
  07/07) ne couvre que les messages de chat : un avatar volumineux ou un événement de
  groupe énorme peut encore dépasser `MAX_LOGICAL_MESSAGE` et faire couper la connexion
  par le récepteur. Déplacer la vérification dans `pool.send` (générique à tout paquet).
- [ ] 🟠 `ConnectionPool` n'a **aucune éviction ni limite** (`network/pool.rs`) : une
  entrée par pair jamais nettoyée, une tâche d'écriture par connexion qui ne se termine
  qu'en cas d'erreur d'envoi. Sur un LAN mouvant (pairs qui changent d'IP), la map et les
  tâches s'accumulent. Ajouter un TTL/éviction sur `PeerDisconnected`.
- [ ] 🟠 `ConnectionPool::connect` **prend un verrou puis fait un handshake réseau sous
  ce verrou** dans `dial_and_send` (`self.conns.lock().await` autour de `connect`) : deux
  émissions concurrentes vers le même pair sérialisent un handshake complet, et une
  émission vers un pair lent bloque l'insertion des autres. Dialer hors verrou, ne
  verrouiller que pour insérer.
- [ ] 🟢 La découverte lit dans un **buffer fixe de 1 024 octets** (`discovery.rs:75`) :
  un `DiscoveryPacket` (username libre + clé hex 64 + port) tient largement, mais un
  username très long le tronquerait silencieusement (JSON invalide → paquet ignoré).
  Borner la longueur du username à la source.
- [ ] 🟠 Les accusés de lecture différés sont **réémis pour toute la fenêtre de
  messages** à chaque ouverture de conversation (`ui/mod.rs::
  send_read_receipts_for_conversation`, 05-07/07) : jusqu'à 2 000 messages × N membres
  de salon à chaque clic de sidebar. Mémoriser le dernier hash accusé par conversation
  et n'émettre que le delta.
- [ ] 🟢 Documenter les constantes de découverte (`discovery.rs` : multicast
  `239.255.42.98`, broadcast 3 s, timeout 6 s) et leur impact batterie/réseau dans
  `docs/03-reseau-et-securite.md`.

## 5. Sécurité

- [ ] 🔴 **Usurpation d'identité applicative** : après le handshake Noise et le Hello,
  `network/server.rs::dispatch_packet` ne vérifie jamais que le champ `from` des paquets
  (`ChatMessage`, `ReadReceipt`, `MessageAck`, `ReactionEvent`, `TypingIndicator`,
  `AvatarAnnounce`) correspond au username authentifié de la connexion. Tout pair
  authentifié peut se faire passer pour n'importe qui (messages, réactions, accusés).
  Passer le `peer` du Hello à `dispatch_packet` et rejeter les paquets dont
  `from != peer`. Idem côté média : `stream_in` authentifie le pair (TOFU) mais ne
  recoupe pas `header.from` avec ce pair (`network/media_stream.rs`).
- [ ] 🔴 **Le username n'est pas lié à la clé au niveau découverte** : `DiscoveryPacket`
  annonce `username` + `pubkey` en clair, sans preuve de possession. Un pair malveillant
  peut annoncer le username d'un autre avec sa propre clé ; à la **première** rencontre
  (avant tout épinglage TOFU) la victime épingle la mauvaise clé. Le TOFU protège les
  rencontres suivantes, pas la première. Documenter cette limite et envisager une
  signature de l'annonce par la clé privée.
- [x] 🔴 **Traversée de répertoire à la réception de média** — ✅ **Résolu (PR #28).**
  Le récepteur valide désormais l'`id` via `is_safe_media_id` avant tout écriture
  (`network/media_stream.rs:246`) et rejette tout composant de chemin non simple
  (`.`, `..`, séparateurs). *(Constat initial : `media_dir.join(&header.media.id)` était
  écrit sans ré-assainir un `header` venu brut du réseau.)*
- [ ] 🟠 TOFU : le changement de clé déclenche bien alerte + refus (`Trust::Mismatch`),
  mais il n'existe **aucun flux de ré-appairage légitime** (réinstallation d'un pair) —
  l'utilisateur est bloqué sans passer par la suppression manuelle des données. Ajouter
  une action « faire confiance à la nouvelle clé » explicite dans l'UI.
- [ ] 🟠 Historique **en clair au repos** : `abcom.db` (messages, avatars) n'est pas
  chiffré. Documenter ce choix dans le modèle de menace, et évaluer SQLCipher
  (rusqlite feature `sqlcipher`) en option.
- [ ] 🟠 `cargo audit` tourne déjà en CI **main** (réinstallé à chaque run, sans cache),
  mais pas sur **dev** : l'étendre aux PR vers dev, mettre l'outil en cache, et ajouter
  `cargo deny` (licences + doublons de crates).
- [ ] 🟠 Le collage trop long écrit le `.txt` dans `std::env::temp_dir()`
  (`input_bar.rs::stash_overflow_paste`, ajouté le 07/07) : lisible par les autres
  utilisateurs de la machine et jamais nettoyé. L'écrire dans le répertoire de données
  de l'app (0600) et le supprimer après envoi.
- [ ] 🟢 `identity.key` en 0600 ✓ — vérifier l'équivalent Windows (ACL) où
  `from_mode(0o600)` est sans effet.
- [ ] 🟢 Documenter le modèle de menace de la passphrase de salon (PSK `XXpsk3`) :
  qui la connaît, comment elle se distribue, ce qu'elle protège réellement.
- [ ] 🟢 Les messages sont identifiés par un hash **FNV-1a non cryptographique**
  (`app/receipts.rs::message_hash`) : réactions, réponses et accusés d'un pair peuvent
  cibler un hash forgé/deviné. Évaluer un identifiant aléatoire porté par le message
  (le champ `nonce` existe déjà) plutôt qu'un hash dérivé du contenu.

## 6. Persistance & données

- [ ] 🟠 Migration JSON → SQLite : les fichiers `*.json.bak` (et
  `messages.json.bak.<epoch>`) restent indéfiniment dans le répertoire de données.
  Ajouter une politique de nettoyage (suppression après N versions/jours) et une note
  de migration dans la doc.
- [ ] 🟠 `read_counts` compte des **nombres de messages lus** : après
  `clear_conversation_history` ou la purge du ring-buffer, le compte peut désigner un
  ensemble différent de messages (bord de fenêtre). Baser le « lu jusqu'à » sur un
  rowid/hash de dernier message lu, plus robuste.
- [ ] 🟢 Aucune maintenance de la base : ni `VACUUM` périodique, ni contrôle de taille
  (l'historique croît sans limite). Ajouter une commande de compaction et,
  optionnellement, une rétention configurable.
- [ ] 🟢 Pas d'export/sauvegarde de l'historique (portabilité) : une commande
  d'export JSON/texte par conversation serait cohérente avec l'esprit local-first.

## 7. Performance

### 7a. Empreinte mémoire (mesurée le 07/07, macOS, `vmmap`/`ps`)

> **Constat : ~92-98 Mo RSS par instance, 132 Mo d'empreinte physique**, même au
> repos (fenêtre visible, aucune conversation active). Répartition réelle mesurée sur
> une instance (`vmmap --summary`) :
>
> | Poste | Taille | Nature |
> |-------|--------|--------|
> | **IOAccelerator (graphics)** | **42,4 Mo** | contexte GPU du renderer **Glow/OpenGL** |
> | Malloc (tas) | ~20 Mo alloués, **24 % de fragmentation** | état app, caches, décodage |
> | CG Image | 7,9 Mo | images Core Graphics (icône, staging emoji) |
> | dont textures emoji | ~6,7 Mo GPU | 323 PNG 72×72 décodés **au démarrage** |
>
> Le poste dominant n'est pas l'état applicatif (léger) mais **la pile graphique**.

- [ ] 🔴 **Passer le renderer de Glow (OpenGL) à wgpu (Metal natif)** — le plus gros
  levier mémoire *et* pérennité. OpenGL est déprécié sur macOS et émulé au-dessus de
  Metal (le binaire lie encore `OpenGL.framework`) : ~42 Mo d'`IOAccelerator` pour une
  UI 2D triviale. Le seul lien dur à Glow dans le code est **un paramètre inutilisé**
  (`ui/mod.rs:704 on_exit(_gl: Option<&eframe::glow::Context>)`) ; le reste est
  `renderer: Renderer::Glow` (`ui/mod.rs:795`) et la feature eframe. Migration à faible
  risque, gain attendu sur la baseline GPU et le rendu, et aligne l'app sur le backend
  supporté de macOS. À valider par mesure après bascule.
- [ ] 🟠 **Décoder les emojis paresseusement**, par catégorie et à la demande : les 323
  PNG sont décodés et téléversés en GPU **au lancement** (`ui/mod.rs::spawn_emoji_decoder`)
  même si l'utilisateur n'ouvre jamais le sélecteur — ~6,7 Mo GPU + 6,5 Mo de
  `ColorImage` CPU + latence de démarrage. Ne décoder que les emojis réellement présents
  dans le fil visible, et le reste à l'ouverture du picker (par onglet).
- [ ] 🟠 **Rendre la RAM au système sur repli tray** : `hide_to_tray` libère déjà les
  textures (bien), mais le RSS ne baisse pas — l'allocateur système ne rend pas les
  pages à l'OS (24 % de fragmentation constatée). Adopter **mimalloc** (ou jemalloc) en
  allocateur global et déclencher un *purge/decommit* explicite sur `hide_to_tray` :
  c'est la seule façon de faire réellement chuter la mémoire de l'app en arrière-plan,
  qui est l'objectif prioritaire.
- [ ] 🟠 Sur repli tray, envisager de **libérer aussi le contexte graphique** (ou réduire
  la fentere à 1×1 / la détruire) plutôt que de garder ~42 Mo d'`IOAccelerator` alloués
  pendant que l'app ne fait que veiller le réseau — à arbitrer contre le coût de
  reconstruction à la réouverture.
- [ ] 🟢 Runtime tokio en **2 worker threads** (`main.rs:56`) pour une charge purement
  I/O : un runtime `current_thread` (ou 1 worker) suffirait probablement et économise
  des piles de threads — à mesurer.

### 7b. Chemins chauds & rafraîchissement

- [ ] 🟠 `composer/mod.rs::composer_caret_positions` reconstruit à **chaque frame** un
  `Vec<Pos2>` en itérant tous les caractères de la saisie (avec 1-2 lookups HashMap par
  caractère), même sans changement — et deux fois quand la scrollbar apparaît. Memoïser
  par (texte, largeur) comme le fil le fait avec son cache.
- [ ] 🟠 `unread_count`/`mark_conversation_read` re-scannent tous les messages en mémoire
  à chaque rafraîchissement de sidebar ; avec 2 000 messages × N conversations ça reste
  du O(n·m) évitable → compteurs incrémentaux mis à jour dans `add_message`.
- [ ] 🟢 Les seuils du repli des longs messages (`snapshot.rs::COLLAPSE_*`) et le plafond
  du composeur (`composer::MAX_INPUT_CHARS`) sont fondés sur des mesures du 07/07
  (~14 ms de layout pour 100 k caractères) — consigner ces mesures dans la doc et les
  re-vérifier à chaque montée de version egui.
- [ ] 🟢 Le cache de textures médias (`media_textures`, `avatar_textures`) n'a pas de
  borne d'éviction explicite hors GIFs — vérifier le comportement sur un long historique
  d'images.
- [ ] 🟠 Le thread de stockage traite les commandes **une par une, sans transaction de
  lot** (`app/storage.rs::run`) : une rafale (import, réception de salon actif) fait un
  commit WAL par message. Regrouper les `InsertMessage` en attente dans une transaction
  quand la file en contient plusieurs (`try_recv` en boucle courte). *(WAL +
  `synchronous=NORMAL` + `prepare_cached` déjà en place — c'est le lot qui manque.)*
- [ ] 🟢 **Rafraîchissement intelligent — déjà solide, à préserver.** L'`update`
  court-circuite tout rendu quand la fenêtre est cachée/minimisée (`ui/mod.rs:568`), les
  caches dérivés (fil, sidebar) ne se reconstruisent que sur changement de génération
  d'état, et les GIFs sortis du fil libèrent leurs frames. C'est le bon modèle. Point de
  vigilance : `request_repaint` est appelé depuis ~14 endroits, dont certains dans la
  boucle de rendu (survol, flash de highlight, décodage emoji en attente avec
  `request_repaint_after(50ms)`) — vérifier qu'aucun ne maintient un repaint continu à
  60 fps hors animation réelle (coût CPU/batterie sur fenêtre visible mais inactive).
- [ ] 🟢 egui recalcule le layout de tout le fil visible à chaque frame de repaint : le
  cache `ChatCache` évite le re-parse markdown mais pas le re-layout galley d'egui.
  Vérifier que `stick_to_bottom` + fenêtrage (`chat_visible_count`) borne bien le nombre
  de lignes réellement mises en page, y compris sur un très long historique déroulé.

## 8. Tests

- [ ] 🟠 Aucun test **bout-en-bout multi-processus** : `scripts/integration_test.sh`
  (CI main) vérifie compilation, tests unitaires et présence du binaire, pas un échange
  réel entre deux instances (découverte → message → ACK → read receipt) — le cœur du
  produit reste testé à la main via `make run2`. Les briques existent :
  `test_network_server.rs` sait monter un vrai serveur + client chiffrés.
- [ ] 🟠 Mesurer la couverture en CI (`cargo llvm-cov`) : 259 tests unitaires passent
  mais aucune visibilité sur les zones non couvertes (le rendu UI notamment, où vivent
  beaucoup de régressions récentes : survol, pagination, curseur).
- [ ] 🟢 Ajouter des bancs `criterion` pour les chemins chauds mesurés à la main cette
  semaine (parse markdown, `message_hash`, reconstruction du snapshot) afin d'objectiver
  les régressions de perf.
- [ ] 🟢 Tests de propriété (proptest) sur `message_hash` (stabilité inter-versions,
  collisions nonce) et sur les opérations de curseur du composeur (invariants UTF-8) —
  les crashs récents (frontières de sélection) auraient été attrapés.

## 9. CI/CD & outillage

- [ ] 🟠 La CI (fmt, clippy `-D warnings`, build, tests) ne tourne que sur
  `ubuntu-latest` alors que les cibles réelles sont macOS et Windows (code
  spécifique : `objc2`, tray, rodio, ACL). Ajouter une matrice `macos-latest` /
  `windows-latest` au moins sur `main`.
- [ ] 🟠 Pas de pipeline de **release** : aucun binaire publié sur tag, distribution
  uniquement via `scripts/build-and-distribute.sh` manuel. Automatiser (GitHub Release +
  artefacts signés par OS).
- [ ] 🟢 Fixer une MSRV (rust-version dans `Cargo.toml`) et la tester en CI.
- [ ] 🟢 Finaliser les githooks partagés (branche `task/shared-githooks` restée ouverte)
  pour aligner le hook local `clippy` (commit `fix/hook-add-clippy`) sur la CI.
- [ ] 🟢 `cargo outdated` périodique (workflow mensuel) : `dirs 5`, `rodio 0.19`,
  `egui 0.31`… — définir une politique de mise à jour plutôt que de subir les montées
  de version.

## 10. Documentation

- [ ] 🟠 Mettre à jour `docs/05-fonctionnalites.md` **et la section « Non publié » du
  CHANGELOG** avec les changements de la semaine : accusés nominatifs de salon (« … »),
  rendu multiligne, plafond de saisie et compteur, repli des longs messages,
  collage → `.txt`, seed de démo.
- [ ] 🟠 `docs/08-historique-et-audits.md` référence l'audit du 27/06 — y ajouter le
  présent audit et archiver l'ancien processus (`AVANCEMENT.md` a une règle anti-conflit
  spécifique, la rappeler dans le README de `docs/`).
- [ ] 🟢 README : ajouter badges CI, capture d'écran à jour (la barre de saisie a changé),
  et un « Quick start » 3 lignes (`make run2`).
- [ ] 🟢 Documenter le format du protocole (paquets JSON, handshake Noise XX/XXpsk3,
  framing 64 Ko, `MAX_LOGICAL_MESSAGE`) dans `docs/03-reseau-et-securite.md` — la doc
  actuelle décrit l'intention, pas le format binaire exact.
- [ ] 🟢 Ajouter un `CONTRIBUTING.md` court pointant `docs/git.md` (workflow de branches
  déjà rédigé) et la convention de tests.

## 11. Distribution & plateforme

- [ ] 🟠 macOS : binaire ni signé ni notarisé — Gatekeeper le bloquera hors de la machine
  de dev. Documenter la limitation ou intégrer la signature au pipeline de release.
- [ ] 🟢 Tester réellement `scripts/install-windows.ps1` et `contrib/` (desktop entry
  Linux) sur les OS cibles ; noter la matrice de support dans le README.
- [ ] 🟢 `panic = "abort"` en release + absence de logging fichier = crash silencieux
  chez l'utilisateur : au minimum un hook de panique qui écrit la cause dans le
  répertoire de données avant d'abandonner.

## 12. UI / UX

- [ ] 🟠 Vérifier le **thème clair** : un sélecteur système/sombre/clair existe
  (`ui/settings.rs::theme_preference`) mais une grande partie des couleurs du fil, de la
  sidebar et de la barre de saisie sont codées en dur pour un fond sombre (texte blanc
  fixe de l'indicateur de frappe, gris 140-160, liseré 96-96-100…).
- [ ] 🟢 Navigation clavier au-delà du composeur : passer d'une conversation à l'autre
  (Ctrl+Tab / Cmd+K style), atteindre la recherche d'emoji, fermer les popups à
  l'Échap de manière homogène.
- [ ] 🟢 Accessibilité : egui expose AccessKit — vérifier que les boutons peints
  (icônes maison, « + », coches) portent bien un libellé lisible par lecteur d'écran ;
  la barre de survol et les popups n'en ont probablement pas.
- [ ] 🟢 États vides : conversation sans message (« Aucun message »), pair hors ligne,
  salon vide — harmoniser le ton et proposer une action (« Envoyer le premier
  message »).

## 13. Observabilité & robustesse à l'exécution

- [ ] 🔴 **Aucune remontée d'erreur à l'utilisateur pour les échecs réseau** : `pool.rs`
  et les senders avalent tout en `eprintln!` (`Connexion sécurisée impossible`,
  `Handshake échoué`…). Sur un binaire release strippé sans console, l'utilisateur ne
  voit rien — ni « message non parti », ni « pair injoignable ». Relier ces échecs à la
  bannière de notification déjà présente dans l'UI.
- [ ] 🟠 **Pas de nettoyage d'arrêt** : `main.rs` termine par `ui::run(...)?; Ok(())` — à
  la fermeture, `flush_storage` existe mais les tâches tokio et les connexions du pool
  sont abandonnées brutalement. Vérifier qu'aucune écriture SQLite n'est perdue et
  fermer proprement (le WAL aide, mais un flush explicite du pool serait plus sûr).
- [ ] 🟠 Le `TrustStore` utilise `Mutex::lock().unwrap()` (`secure.rs:281`) dans les
  tâches réseau : une panique côté écriture SQLite empoisonnerait ce verrou et bloquerait
  toute nouvelle connexion. Même politique anti-empoisonnement que le point §2.
- [ ] 🟢 Ajouter des métriques de session minimales (messages envoyés/reçus, pairs vus,
  reconnexions, paquets jetés par `try_send`) accessibles depuis Paramètres — aide au
  diagnostic sans logging verbeux.
- [ ] 🟢 Timeouts explicites sur les handshakes sortants (`pool::connect`,
  `media_stream::connect_secure`) : un pair qui accepte la TCP mais ne répond pas au
  handshake bloque la tâche indéfiniment (le streaming média a un `DECISION_TIMEOUT`,
  mais pas le handshake lui-même).

---

## Synthèse des chantiers prioritaires (🔴)

**Sécurité — à traiter en premier :**

| # | Chantier | Fichiers principaux |
|---|----------|--------------------|
| ~~S1~~ | ~~Traversée de répertoire à la réception de média~~ — ✅ **résolu (PR #28)** | `network/media_stream.rs` |
| S2 | Vérifier `from` == pair authentifié, chat **et** média (anti-usurpation) | `network/server.rs`, `network/media_stream.rs` |
| S3 | Première rencontre TOFU : username non lié à la clé en découverte | `discovery.rs`, `message/` |

**Robustesse & fiabilité :**

| # | Chantier | Fichiers principaux |
|---|----------|--------------------|
| R1 | Implémenter le retry réel des messages non ACKés (aujourd'hui un stub) | `ui/events.rs`, `app/receipts.rs`, `network/pool.rs` |
| R2 | Versionner le protocole réseau | `message/`, `network/secure.rs` |
| R3 | Remonter les échecs réseau à l'utilisateur (aujourd'hui `eprintln!` invisible) | `network/pool.rs`, `ui/` |
| R4 | Politique mutex/panic (55 `lock().unwrap()` + `TrustStore`) | tout `ui/`, `app/mod.rs`, `network/secure.rs` |

**Performance mémoire (~92-98 Mo RSS/instance mesuré) :**

| # | Chantier | Gain attendu |
|---|----------|--------------|
| P1 | Renderer Glow (OpenGL émulé) → wgpu (Metal natif) | baseline GPU (~42 Mo `IOAccelerator`) + pérennité macOS |
| P2 | Décodage emoji paresseux par catégorie | ~13 Mo (6,7 GPU + 6,5 CPU) + démarrage plus rapide |
| P3 | Allocateur mimalloc + purge sur repli tray | RAM en arrière-plan **effectivement** rendue à l'OS |

**Dette & hygiène :**

| # | Chantier | État |
|---|----------|------|
| ~~D1~~ | ~~Logging structuré à la place des 34 `eprintln!`~~ | ✅ **fait (05/08, P3)** |
| D2 | Nettoyage dépôt (`font 2/`, gitignore, URL repo) | ✅ **fait (05/08, P1)** — `old/` conservé (archive volontaire) ; versionnage Cargo (`0.0.1`) toujours en suspens |

---

*Audit établi en plusieurs passes de vérification, chaque constat recoupé avec le
code source (métriques recomptées, chemins de fichiers et numéros de ligne
vérifiés) et, pour la mémoire, avec des mesures réelles (`vmmap`/`ps` sur des
instances en fonctionnement). Les affirmations infirmées par le code ont été
retirées ou corrigées en cours de route — p. ex. WAL/`prepare_cached` déjà
présents côté SQLite, `media_id` assaini à l'émission mais pas à la réception,
rafraîchissement déjà court-circuité en arrière-plan.*
