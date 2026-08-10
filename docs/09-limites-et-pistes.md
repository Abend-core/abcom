# 09 — Limites connues et pistes

Inventaire honnête de ce que l'application ne fait pas (ou fait imparfaitement), avec les pistes envisagées. Chaque limite listée est **assumée** : elle résulte d'un arbitrage documenté, pas d'un oubli.

## Sécurité

| Limite | Détail | Piste |
|---|---|---|
| Pas de chiffrement au repos | `abcom.db`, `media/` et les avatars sont en clair sur le disque local ; seul le transit est chiffré | SQLCipher, ou chiffrement fichier par clé dérivée de la session utilisateur |
| Découverte visible | L'annonce UDP (pseudo + empreinte de clé) est lisible par tout le LAN — c'est la fonction même de la découverte | La passphrase de salon empêche déjà un inconnu d'établir une session |
| Autorisation locale, sans signature transférable | L'auteur Noise est vérifié et les actions sont autorisées selon le propriétaire connu ; un événement n'est toutefois pas vérifiable hors de sa session d'origine | Signer les événements uniquement si un relais tiers apparaît |

## Groupes et messagerie

| Limite | Détail | Piste |
|---|---|---|
| Le nom du groupe est son identifiant | Deux groupes créés indépendamment sous le même nom se percutent (l'upsert du dernier propriétaire vu fait foi) | UUID + nom d'affichage libre — changement de protocole, à faire d'un bloc |
| Pas de réémission hors ligne | Un membre absent au moment d'un envoi ne recevra jamais ce message (les mutations de membres, elles, sont rattrapées par le propriétaire) | Journal par salon avec offset par membre |
| Renommage de salon sans UI | Le protocole et la migration d'historique existent (`Rename`), pas le bouton | Ajouter l'entrée dans le modal de gestion |
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
| Pas de bundle macOS | Binaire nu : les notifications sont attribuées au terminal | Bundle `.app` (`com.abend.abcom`) via cargo-bundle/cargo-packager, à la première release |
| Pas d'installateur Windows complet | Le script PowerShell installe et crée les raccourcis, sans MSI | Package MSI ou ZIP signé |
| Tray Linux dépendant du shell | Nécessite StatusNotifier/AppIndicator (KDE, XFCE ok ; GNOME avec extension) ; sans tray, la croix quitte normalement (repli sûr) | — |
| Tray Windows/Linux non testés en réel | Implémentés mais l'environnement de dev est macOS | Test manuel sur machines cibles |

## Rendu et mesures restantes

- **Renderer** : Glow (OpenGL) est déprécié par Apple ; `wgpu` (Metal natif) pourrait consommer moins de GPU. À trancher **à la mesure** (`powermetrics`), maintenant que le repaint permanent a disparu.
- Mesures à refaire selon le protocole de l'audit (conservé dans `old/docs/06-audit-performance.md` §6) : GPU au repos, RSS après navigation GIF intensive picker fermé, débit d'un transfert > 1 Go.
- QA manuelle du fenêtrage : compensation d'offset au chargement de 100 messages pendant qu'un message arrive.

## Outillage

- Le scénario P2P headless couvre handshake, identité et message sur une vraie socket. La découverte UDP et le cycle complet de deux processus GUI restent validés manuellement.
