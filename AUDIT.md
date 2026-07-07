# Audit qualité — abcom

> Checklist complète des améliorations pour un projet propre au maximum.
> État au 7 juillet 2026, branche `refactor/input-bar-layout`.
> Priorités : 🔴 important (correctif ou dette bloquante) · 🟠 recommandé · 🟢 confort/finition.
> Chaque point référence les fichiers concernés ; cocher au fur et à mesure.

---

## 1. Hygiène du dépôt

- [ ] 🔴 Supprimer `old/` (24 fichiers de documentation legacy suivis par git) après avoir
  vérifié que tout le contenu utile a bien été migré vers `docs/` — le doublon crée de la
  confusion sur la doc de référence.
- [ ] 🟠 Supprimer le dossier vide `font 2/` à la racine (non suivi, nom avec espace —
  résidu de manipulation macOS).
- [ ] 🟠 Compléter `.gitignore` : `.DS_Store`, `nohup.out` y est déjà, ajouter `*.log`
  éventuels des scripts de test.
- [ ] 🟠 `Cargo.toml` : la version est figée à `0.0.1` alors que le projet a un CHANGELOG —
  adopter un versionnage réel (bump à chaque merge sur `main`, tags git).
- [ ] 🟢 Vérifier que l'URL `repository` de `Cargo.toml` (`github.com/rxdy/abcom`)
  correspond bien au remote effectif (org `Abend-core` vue dans les PR).
- [ ] 🟢 `scripts/` : préfixer chaque script d'un en-tête d'usage homogène et lister les
  scripts dans `docs/07-developpement.md` (`integration_test.sh` tourne en CI main,
  `QUICK_START_TEST.sh` est un pense-bête interactif — clarifier le rôle de chacun).

## 2. Qualité du code

- [ ] 🔴 **62 `unwrap()`/`expect()`/`panic!` hors tests**, dont **55 `lock().unwrap()`**
  sur le mutex `AppState` : un thread qui panique en tenant le verrou empoisonne le mutex
  et fait tomber toute l'app en cascade. Introduire un helper
  (`fn state(&self) -> MutexGuard<…>` avec `unwrap_or_else(|e| e.into_inner())`) ou
  passer à `parking_lot::Mutex` (pas d'empoisonnement).
- [ ] 🔴 Remplacer les **34 `eprintln!`/`println!`** de production par un vrai logging
  (`log` + `env_logger`, ou `tracing`) : niveaux, horodatage, et possibilité d'écrire
  dans un fichier pour diagnostiquer chez un utilisateur.
- [ ] 🟠 Purger les **16 `#[allow(dead_code)]`** : soit le code sert (le brancher), soit
  il meurt (`app/receipts.rs::is_message_pending`, `app/messages.rs::get_conversations`,
  `composer/cursor.rs` entier, etc.).
- [ ] 🟠 Découper les gros fichiers UI : `chat_panel.rs` (1 506 lignes),
  `input_bar.rs` (1 164), `ui/mod.rs` (898). Extraire par exemple le rendu d'une ligne
  de fil, la barre de survol, et le popup notifications dans des sous-modules.
- [ ] 🟠 Dédupliquer le scan d'emojis : `markdown.rs::is_text_emoji_only`,
  `emoji_picker.rs::render_inline` et `composer/mod.rs::composer_caret_positions`
  réimplémentent tous trois la même itération « séquence de 2 puis 1 caractères dans
  `emoji_map` » — extraire un itérateur commun.
- [ ] 🟠 Centraliser les constantes visuelles dupliquées : `line_height = 22.0` apparaît
  en dur dans plusieurs fonctions du composeur ; couleurs (gris 140/150/160, bleu
  80-180-255…) dispersées dans `chat_panel.rs`, `input_bar.rs`, `sidebar.rs` → un module
  `ui/theme.rs`.
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
- [ ] 🟢 Regrouper l'intégration bureau dans un module `platform/` : aujourd'hui
  `notify.rs` et `autostart.rs` vivent à la racine de `src/`, `tray.rs` dans `ui/`,
  et la bascule Dock macOS dans `ui/mod.rs`.

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
  `from != peer`.
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

---

## Synthèse des 6 chantiers prioritaires (🔴)

| # | Chantier | Fichiers principaux |
|---|----------|--------------------|
| 1 | Vérifier `from` == pair authentifié (anti-usurpation) | `network/server.rs` |
| 2 | Implémenter le retry réel des messages non ACKés | `ui/events.rs`, `app/receipts.rs`, `network/pool.rs` |
| 3 | Politique mutex/panic (55 `lock().unwrap()`) | tout `ui/`, `app/mod.rs` |
| 4 | Versionner le protocole réseau | `message/`, `network/secure.rs` |
| 5 | Logging structuré à la place des `eprintln!` | transversal |
| 6 | Nettoyage dépôt (`old/`, versionnage Cargo) | racine |
