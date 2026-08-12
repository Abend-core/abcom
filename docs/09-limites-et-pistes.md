# 09 — Limites connues et pistes

Inventaire honnête de ce que l'application ne fait pas (ou fait imparfaitement), avec les pistes envisagées. Chaque limite listée est **assumée** : elle résulte d'un arbitrage documenté, pas d'un oubli.

## Sécurité

| Limite | Détail | Piste |
|---|---|---|
| Pas de chiffrement au repos | `abcom.db`, `media/` et les avatars sont en clair sur le disque local ; seul le transit est chiffré | SQLCipher, ou chiffrement fichier par clé dérivée de la session utilisateur |
| Découverte visible | L'annonce UDP (pseudo + empreinte de clé) est lisible par tout le LAN — c'est la fonction même de la découverte | La passphrase de salon empêche déjà un inconnu d'établir une session |
| Liste blanche du CDN de GIF codée en dur | Le chargement d'un média distant est restreint à `https://*.klipy.com` — le CDN observé est `static.klipy.com`, donc couvert. Un changement de domaine côté Klipy ferait retomber les GIF sur une carte fichier | Le domaine se corrige dans `ALLOWED_MEDIA_URL_HOSTS` ; test 7.3 du [cahier](10-cahier-de-tests.md) |
| Autorisation locale, sans signature transférable | L'auteur Noise est vérifié et les actions sont autorisées selon le propriétaire connu ; un événement n'est toutefois pas vérifiable hors de sa session d'origine | Signer les événements uniquement si un relais tiers apparaît |

## Groupes et messagerie

| Limite | Détail | Piste |
|---|---|---|
| Pas de réémission hors ligne | Un membre absent au moment d'un envoi ne recevra jamais ce message (les mutations de membres, elles, sont rattrapées par le propriétaire) | Journal par salon avec offset par membre |
| Renommage de salon sans UI | Le protocole existe (`Rename`) et le renommage ne coûte plus rien depuis le passage aux identifiants immuables, mais le bouton n'est pas câblé | Ajouter l'entrée dans le modal de gestion |
| Salon sans membre en ligne | Le message part dans le vide (historique local seulement) ; la barre de saisie reste active, contrairement au privé qui signale « hors ligne » | Avertissement visuel |
| Chiffrement de groupe par lien | Pas de clé de salon partagée : l'émetteur chiffre vers chaque membre sur sa session pair-à-pair — suffisant tant que l'émetteur relaie lui-même | Clé de groupe si le relais par un tiers apparaît |

## Protocole et transport

| Limite | Détail | Piste |
|---|---|---|
| Deux ports TCP | Les médias ont leur propre listener (`chat + 1`) avec son en-tête dédié | Multiplexer les médias sur la connexion chat unique (un octet de type de trame suffit) — différé volontairement : refonte de protocole juste après la précédente, gain purement architectural, à faire une fois le transport validé par l'usage |
| Identité liée à la machine | Une clé par machine, pas par utilisateur ; changer de machine = nouvelle identité (bandeau « clé changée » chez les pairs) | Export/import de l'identité |
| Compatibilité stricte | Le `Hello` porte une version et rejette explicitement une version incompatible ; aucune négociation N/N-1 n'est encore prévue | Ajouter des capacités négociées lorsqu'une première évolution compatible devient nécessaire |

## Plateformes et packaging

| Limite | Détail | Piste |
|---|---|---|
| Pas de binaire musl statique | `wgpu` charge le pilote Vulkan par `dlopen`, impossible dans un binaire musl statique ; ALSA ajoute la même contrainte. La compilation **dynamique** contre musl (Alpine) fonctionne | Aucune : lever la limite supposerait d'abandonner l'interface graphique |
| Pas de bundle macOS | Binaire nu : les notifications sont attribuées au terminal | Bundle `.app` (`com.abend.abcom`) via cargo-bundle/cargo-packager, à la première release |
| Pas d'installateur Windows complet | Le script PowerShell installe et crée les raccourcis, sans MSI | Package MSI ou ZIP signé |
| Tray Linux dépendant du shell | Nécessite StatusNotifier/AppIndicator (KDE, XFCE ok ; GNOME avec extension) ; sans tray, la croix quitte normalement (repli sûr). C'est le shell qui décide d'afficher le protocole : aucune implémentation côté application ne lève cette limite | — |
| Restauration depuis le tray en Wayland natif | Wayland interdit à un client de se mettre au premier plan seul (pas de jeton `xdg-activation` sur un clic D-Bus) : la fenêtre peut revenir sans passer devant. Non observé aujourd'hui, le backend X11 étant forcé (cf. `prefer_x11_backend`) | Aucune côté application — à mesurer le jour où le contournement winit sera retiré |
| Tray Windows/Linux non testés en réel | Implémentés mais l'environnement de dev est macOS | Passe manuelle §13 du [cahier de tests](10-cahier-de-tests.md) sur machines cibles |

## Dette identifiée, non résorbée

Constats de l'audit d'exploitation des dépendances (archivé dans
[`old/2026-08-audit-dependances.md`](../old/2026-08-audit-dependances.md), 25 de
ses 30 points appliqués) délibérément laissés en l'état, avec leur raison.

| Constat | Détail | Pourquoi pas appliqué |
|---|---|---|
| Sélecteur de fichiers reporté d'une frame | `rfd::AsyncFileDialog` gérerait lui-même le dispatch sur le thread principal exigé par macOS ; notre code reporte l'ouverture d'une frame à la place | Le `Future` demande un exécuteur dont l'UI n'a pas de handle. Faire remonter le runtime tokio jusqu'à l'interface pour gagner **une frame**, avec pour mode d'échec un sélecteur de fichiers cassé et aucun test automatisé possible : le rapport risque/gain ne le justifie pas |
| Pas d'actions dans les notifications | `notify-rust` sait poser des boutons (« Répondre ») | Sur macOS, `show()` bloque en attendant la réponse, et les boutons n'apparaissent de façon fiable que depuis un bundle `.app` signé — que nous n'avons pas encore. À reprendre avec la signature macOS |
| Fenêtrage du fil fait à la main | `ScrollArea::show_rows` suppose des lignes de hauteur uniforme, ce que nos messages n'ont pas (markdown, médias, citations, réactions) ; `show_viewport` exige des offsets connus d'avance | Reviendrait à reconstruire notre fenêtrage sous une autre forme, sans gain |
| Schéma SQLite non `STRICT` | Les colonnes n'ont pas de typage strict | Différé à une prochaine migration de schéma, pour ne pas cumuler deux réécritures de tables |

## Rendu et mesures restantes

- Mesures à refaire selon le protocole de l'audit (conservé dans `old/2026-07-audit-performance.md` §6) : GPU au repos, RSS après navigation GIF intensive picker fermé, débit d'un transfert > 1 Go.
- QA manuelle du fenêtrage : compensation d'offset au chargement de 100 messages pendant qu'un message arrive.

## Outillage

- Le scénario P2P headless couvre handshake, identité et message sur une vraie socket. La découverte UDP et le cycle complet de deux processus GUI restent validés manuellement.
