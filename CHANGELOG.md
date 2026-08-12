# Changelog — Abcom

Toutes les modifications notables sont documentées ici.  
Format basé sur [Keep a Changelog](https://keepachangelog.com/fr/1.0.0/), versioning [SemVer](https://semver.org/lang/fr/).

---

## [Non publié]

### Modifié
- **Le tray Linux ne dépend plus de GTK** : `libappindicator` n'était qu'une enveloppe GTK autour du protocole D-Bus StatusNotifierItem — l'application le parle désormais directement (`ksni`, Rust pur, sur le `zbus` déjà tiré par les notifications). `libgtk-3`, `libayatana-appindicator3` et `libxdo` disparaissent de l'arbre de dépendances Linux, à la compilation comme à l'exécution : plus aucune bibliothèque à installer pour compiler, et un binaire distribué qui ne peut plus refuser de démarrer sur une machine où elles manquent. La boucle GTK dédiée et le drapeau de badge qu'elle imposait disparaissent avec elle ; macOS et Windows gardent `tray-icon`, inchangés

### Ajouté
- **Identifiant immuable pour les salons** : le nom d'un groupe devient un simple libellé, renommable sans rien casser. Auparavant le nom entrait dans le hash des messages, si bien qu'un renommage orphelinait d'un coup réactions, accusés et repère de lecture. Migration de base automatique ; **le protocole passe en version 3**, les pairs restés en version 2 ne se connectent plus
- **Cahier de tests manuels** ([docs/10](docs/10-cahier-de-tests.md)) : fonctionnalités, régressions et spécificités par OS
- **Recherche dans l'historique** (Cmd/Ctrl+F) : index FTS5 sans seconde copie des messages sur le disque, recherche au fil de la frappe, clic sur un résultat pour sauter au message
- **Raccourcis clavier globaux** : Cmd/Ctrl+F recherche, Cmd/Ctrl+, paramètres, Ctrl+Tab et Ctrl+Maj+Tab pour changer de conversation, Échap ferme la surcouche la plus haute
- **Glisser-déposer** de fichiers directement dans la fenêtre
- **Journal fichier** tournant dans `<données>/logs/`, en plus de la console
- **Messages hors ligne** : un message écrit à un pair absent reste dans le fil et part automatiquement à sa reconnexion, au lieu d'être refusé — la file est persistée, un redémarrage ne perd rien
- **Annonces de découverte signées** (Ed25519 dérivé de l'identité Noise) avec horodatage : plus personne ne peut injecter de pairs fantômes sur le LAN ni rejouer une annonce capturée
- **Export d'une conversation** en texte et **compaction de la base** (Paramètres → Général → Données)
- Ré-appairage explicite après un changement de clé d'identité : l'alerte propose « Faire confiance à la nouvelle clé » (la connexion reste refusée tant que l'utilisateur n'a pas tranché) — jusqu'ici, un pair réinstallé était bloqué sans recours
- Compteurs de session dans Paramètres → Général → Diagnostic : paquets envoyés, reçus, **jetés**, pairs vus — les pertes réseau silencieuses deviennent visibles
- Bannière « X : injoignable, message non envoyé » quand aucune connexion sécurisée ne peut être établie (auparavant, l'échec n'existait que dans les logs, invisibles sur un binaire release)
- Rapport de crash : la cause d'une panique est écrite dans `last-panic.txt` du répertoire de données avant l'arrêt
- Pipeline de release (tag `v*`) : binaires Linux/macOS/Windows et `SHA256SUMS` attachés à une GitHub Release ; veille mensuelle `cargo outdated` + `cargo audit`
- MSRV déclarée (`rust-version = "1.95"`) et vérifiée en CI ; `cargo audit` et `cargo deny` désormais exécutés sur `dev` comme sur `main`

### Corrigé
- **Arrêt brutal à l'ouverture d'un sélecteur de fichiers** (joindre un fichier, exporter, changer d'image de profil) : les boîtes de dialogue natives bloquantes démarrent une boucle d'événements imbriquée, dans laquelle winit se retrouve à traiter un événement alors qu'il en traite déjà un — et panique. L'application se fermait sans un mot, de façon intermittente selon les événements en vol. Les sélecteurs sont désormais asynchrones, le thread de rendu ne bloque plus
- **Carrés vides à la place des caractères non latins** : les polices par défaut ne couvraient ni la coche `✓`, ni les flèches, ni surtout le moindre alphabet non latin — un message collé en chinois, en japonais ou en coréen était illisible d'un bout à l'autre. Une chaîne de polices remplace le choix unique : **Noto Sans** (texte), **Noto Sans Symbols 2** (symboles), **Inter** (flèches, déjà embarquée pour les noms d'auteur) puis **GNU Unifont** en dernier recours, qui couvre tout le plan multilingue de base. Chaque police n'est consultée que pour ce que les précédentes ne savent pas dessiner : le rendu tramé d'Unifont ne sert donc que là où le choix est entre « moche » et « rien ». Couverture du plan de base : 4 340 → 57 496 caractères
- **Images BMP en vignette cassée** : l'extension était traitée comme une image sans que le décodeur correspondant soit compilé
- **Messages perdus en silence entre deux pairs** : chaque annonce de découverte part deux fois (multicast et broadcast) et revient sous deux adresses source, si bien que l'adresse retenue pour un pair basculait toutes les trois secondes. Le pool rouvrait une connexion à chaque bascule, le pair d'en face refusait cette session en double *après* le handshake — l'émetteur croyait donc son canal établi — et les messages écrits entre-temps disparaissaient sans la moindre erreur, jusqu'à six minutes durant. Mesuré à 69 % de pertes entre deux instances locales, aussi bien en privé qu'en salon ou en diffusion. L'adresse d'un pair est désormais stable tant qu'il donne signe de vie, et une session entrante est remplacée par la plus récente au lieu d'être refusée
- **Envoi de salon vers une adresse fantôme** : la branche « groupe » était la seule à ne pas écarter les pairs restaurés hors ligne (`0.0.0.0:0`)
- **Accusé de lecture au retour du focus** : un message reçu pendant que la fenêtre était en arrière-plan n'était acquitté qu'après avoir quitté la conversation et y être revenu
- **Accusé de réception avant persistance** : le destinataire acquittait dès la mise en file d'écriture ; un arrêt avant commit laissait l'expéditeur croire son message livré alors qu'il avait disparu. L'accusé suit désormais le commit
- **File d'envoi vidée trop tôt** : un message quittait la file durable dès son admission dans le canal réseau — ni écriture socket, ni réception garanties. Un arrêt au mauvais moment le privait de toute réémission
- **Erreurs SQLite invisibles** : disque plein ou base en lecture seule ne produisaient qu'une ligne de log, et la fermeture annonçait un succès. L'échec est maintenant remonté
- **Pagination définitivement bloquée** après débordement de la fenêtre mémoire : le curseur perdu se confondait avec « tout l'historique est chargé », au moment précis où il restait le plus à charger
- **Seuil d'acceptation des médias abaissé de 1 Gio à 50 Mo** : le pire cas écrit sans accord explicite passe de plusieurs Gio à environ 200 Mo
- **Traçage par média distant** : l'URL d'un GIF venue d'un pair déclenchait une requête HTTP à l'affichage, sans clic — révélant l'adresse IP du destinataire et l'instant de sa lecture. Le chargement est restreint au CDN Klipy
- **Schémas d'URL non filtrés** dans les liens Markdown : `file://` et `smb://` (fuite d'empreinte NTLM sous Windows) restent désormais du texte brut
- **Débordement d'entier** sur la péremption des pairs : une horloge corrigée en arrière déclarait tous les pairs perdus d'un coup (et tuait la tâche de découverte en build debug)
- **Fenêtre de lisibilité sur `identity.key`** : le fichier était créé selon l'umask puis resserré, et une clé existante n'était jamais revérifiée
- **Passphrase de salon** dérivée par un hachage unique sans sel : la dérivation est désormais itérée, ce qui coûte une dérivation complète par tentative à un attaquant
- **Effacement d'une conversation** laissant réactions et accusés orphelins jusqu'au redémarrage
- **Un thread OS par pièce jointe** : remplacé par un pool borné, la préparation étant limitée par le disque
- **Plafond du cache média** appliqué au seul démarrage : réappliqué toutes les 15 minutes
- **Erreurs de parcours ignorées** à l'archivage d'un dossier, produisant un ZIP silencieusement incomplet
- **Export annoncé avant écriture** : le succès s'affichait avant que le fichier ne soit écrit
- **Accusé de lecture marqué envoyé avant émission** : une file d'envoi pleine le condamnait à ne jamais partir
- **Découverte non bornée** : plafond du nombre de pairs suivis
- **Ports d'instance** : un `ABCOM_INSTANCE` élevé faisait déborder le calcul
- **Lanceur Linux cassé** : `%h` n'est pas développé dans un fichier `.desktop`, le raccourci ne lançait rien
- **Bouton de mise en forme** jamais implémenté, retiré de la barre de saisie
- Un avatar ou un événement de groupe volumineux pouvait dépasser la taille logique maximale et faire couper la connexion par le destinataire : la vérification est maintenant générique à tout paquet, à la source
- Les connexions vers des pairs disparus n'étaient jamais libérées : balayage périodique et libération immédiate quand la découverte déclare un pair expiré
- Chaque ouverture de conversation réémettait un accusé de lecture pour toute la fenêtre de messages (jusqu'à 2 000 × N membres) : seul le delta est envoyé, et le mémo d'un pair est réinitialisé à sa déconnexion
- Le texte collé trop long était écrit dans `/tmp` (lisible par les autres comptes de la machine) et jamais nettoyé : il va dans `<données>/scratch/` en 0600, purgé après 24 h
- `identity.key` n'était protégé que sous Unix : l'ACL Windows est désormais restreinte au propriétaire
- Les sauvegardes `*.json.bak` de la migration JSON → SQLite restaient indéfiniment : purgées après 30 jours
- Le fichier `.env` injectait n'importe quelle variable dans l'environnement : seules les trois clés attendues sont lues, guillemets gérés, et l'environnement existant a la priorité

### Performance
- **TCP_NODELAY** sur les sockets de chat : notre trafic est fait de petits paquets (messages, accusés, frappe) que l'algorithme de Nagle retenait jusqu'à 40 ms chacun, sur un réseau local où la latence réelle est inférieure à la milliseconde
- **Index SQLite `(to_user, id)`** : toutes les requêtes de conversation parcouraient la table entière ; ajout aussi des pragmas `busy_timeout`, `cache_size`, `mmap_size`, `temp_store` et d'un `PRAGMA optimize` à la fermeture
- **`PowerPreference::LowPower`** : le défaut d'egui réveillait le GPU dédié d'un portable pour peindre une interface 2D
- **`reduce_texture_memory`** : la copie CPU des images est libérée après téléversement en GPU
- **egui/eframe 0.36** : la logique (événements réseau, tray, tâches périodiques) est séparée du rendu, et tourne désormais aussi quand la fenêtre est repliée
- **Emojis décodés à la demande** au lieu du démarrage : empreinte physique au repos 91,7 → 57,6 Mo, et lancement plus rapide
- **Renderer wgpu (Metal natif sur macOS) à la place d'OpenGL** : contexte GPU ramené de 29,6 à 6,2 Mo, RSS de 155,8 à 146,0 Mo, pic d'empreinte de 132,4 à 110,8 Mo (mesures A/B au repos). `glow` n'est plus lié du tout
- **mimalloc en allocateur global**, avec restitution explicite des pages au système lors du repli dans la barre de menus : le RSS descend enfin quand l'application ne fait plus que veiller le réseau (138,9 Mo contre 146,0 Mo)
- Les positions du curseur du composeur étaient recalculées à chaque frame — deux fois quand la barre de défilement apparaît — en itérant toute la saisie : elles sont désormais mémoïsées par (texte, largeur, densité de pixels)
- Le nombre de non-lus re-parcourait tout l'historique en mémoire pour chaque conversation à chaque rafraîchissement de la barre latérale : un seul parcours par changement de contenu suffit maintenant
- Une rafale de messages (import, salon actif, retour en ligne) provoquait un commit SQLite par message : les insertions en attente sont regroupées dans une seule transaction

### Sécurité
- Les images reçues sont **refusées sur leurs dimensions avant d'être décodées** : un pair pouvait faire allouer jusqu'à 512 Mo avec un fichier de quelques kilo-octets
- Le handshake Noise porte un **prologue** liant la session à la version de protocole : une version incompatible échoue avant l'établissement de la session
- Les boutons peints (croix, icônes d'action, coches d'accusé) annoncent un **libellé accessible** : ils étaient muets pour un lecteur d'écran

### Modifié
- `chat_panel.rs` (1 589 lignes) et `input_bar.rs` (1 130) découpés en sous-modules ; `klipy` passe dans `services/`, `notify`/`autostart`/`tray` dans `platform/`
- Trois tests peignent désormais toute l'interface sans fenêtre (panneaux, modales, pickers) : les régressions de structure sont attrapées en CI
- **Licence clarifiée** : `Cargo.toml` déclarait `MIT` alors que `LICENSE` et l'application affichent la GNU AGPL v3. Ce n'était pas une double licence mais une incohérence — le projet est sous **AGPL-3.0**
- Dépendances mises à jour : `dirs` 6, `socket2` 0.6, `ehttp` 0.7, `rfd` 0.17, `rodio` 0.22, `resvg` 0.48, `objc2` 0.6 / `objc2-*` 0.3, plus toutes les montées compatibles semver
- Les accusés de livraison et de lecture sont **persistés** : les coches et le détail « … » survivent au redémarrage au lieu de repartir de zéro
- Fermeture plus propre : après le flush SQLite, les tâches réseau disposent d'un délai borné de 2 s pour terminer leurs écritures en cours au lieu d'être abandonnées

---

## [1.0.0-beta.1] — 2026-08-07

> Première bêta publiée du projet. Tout le développement listé ci-dessous (et
> dans la phase alpha plus bas) faisait partie du travail interne sur `dev`,
> jamais publié ni tagué individuellement — voir README.

### Ajouté
- Raccourcis clavier usuels dans la zone de saisie : Entrée/Maj+Entrée insèrent une nouvelle ligne, Cmd/Ctrl+Entrée envoie le message, Option/Ctrl+⌫ et Option/Ctrl+Suppr suppriment un mot, Cmd+⌫ efface jusqu'au début de ligne, Option/Ctrl+←/→ et Cmd+←/→ déplacent le curseur par mot ou en bout de ligne, Cmd/Ctrl+C/X copient et coupent la sélection — documentés dans `docs/05-fonctionnalites.md`
- Sélecteur de contenu Klipy : GIF animés, mèmes statiques et stickers en 3 onglets indépendants (GIF par défaut)
- Recherche Klipy avec debounce 300 ms, scroll infini et pagination par onglet
- Affichage des GIF animés directement dans le fil de conversation (360×300 px max, ratio préservé)
- Transport GIF par URL uniquement — chaque pair charge le contenu depuis le CDN Klipy
- Attribution « Powered by KLIPY » intégrée dans le pied du sélecteur (dark/light)
- Crédits restructurés : sections Abcom, Klipy, OpenEmoji et Inter avec détails complets
- Règles de workflow Git (`git.md`) pour l'équipe et les agents IA
- Fichier `AVANCEMENT.md` pour le suivi des features sur `dev`
- Script `scripts/run-multi.sh` pour tester la connexion P2P en local
- Transfert de fichiers avec demande d'acceptation du destinataire
- Accusés de réception (ACK) et indicateurs de lecture (✓✓)
- Picker d'emojis avec recherche par shortcode
- Rendu Markdown dans les messages (gras, italique, code, liens)
- Indicateur de frappe en temps réel dans la barre de saisie
- Sélection de texte par clic-glisser dans le compositeur
- Support pièces jointes (fichiers et dossiers)
- Modale de paramètres (thème, langue, notifications)
- Support multilingue FR/EN
- Suite de tests unitaires couvrant tous les modules
- Groupes (Phase 10) : messagerie de salon réservée aux membres, gestion des membres (ajout, exclusion, départ avec succession du propriétaire, suppression), compteurs non-lus et sourdine par salon, modal de gestion — voir `docs/05-fonctionnalites.md`

### Corrigé
- Crash de la zone de saisie quand une frappe et une sélection tombaient dans la même frame (positions de caractères périmées lues par le rendu de la sélection)
- Deux messages pouvaient afficher leur surbrillance de survol en même temps dans la bande de chevauchement de leurs rectangles
- Tremblement du fil pendant le chargement de l'historique vers le haut : l'offset est maintenant compensé dans la même frame (`request_discard`)
- Icône générique dans le Dock à la réouverture de la fenêtre depuis la barre de menus (le retour en politique `Regular` réinitialise l'icône — elle est ré-appliquée)
- Curseur de saisie figé entre deux repaints : clignotement régulier quand le champ a le focus et la fenêtre est au premier plan
- Crash de l'application quand le curseur de saisie se trouvait juste avant un `:` (slice inversée dans la détection de shortcode, déclenchée notamment par Maj+Entrée devant un shortcode)
- Gel de l'application à la création d'un groupe (deadlock sur le verrou d'état dans le modal)
- Messages de groupe diffusés à tous les pairs du réseau au lieu des seuls membres
- Fil de salon vide : les messages des autres membres n'apparaissaient jamais
- Boucle infinie CPU causée par `has_unread` en arrière-plan
- Son de notification bloquant le thread UI (`sleep_until_end` → thread dédié)
- Crash au démarrage lors de la détection réseau sans pairs
- Rendu des emojis/glyphes non supportés dans certains terminaux

### Refactorisé
- Architecture atomique : `app.rs`, `ui.rs`, `message.rs`, `network.rs` découpés en sous-modules

---

## Phase alpha — avril 2026 (non publiée)

> Prototype initial, première version fonctionnelle mais rudimentaire — chat
> P2P local sur LAN. Jamais taguée individuellement : c'est le point de départ
> de la phase alpha qui a mené à `1.0.0-beta.1`.

### Ajouté
- Chat en réseau local : découverte UDP broadcast, une connexion TCP par paquet
- Découverte automatique des pairs par subnet
- Persistance des messages en JSON local (réécriture complète du fichier)
- Interface graphique native egui
