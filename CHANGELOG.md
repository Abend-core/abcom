# Changelog — Abcom

Toutes les modifications notables sont documentées ici.  
Format basé sur [Keep a Changelog](https://keepachangelog.com/fr/1.0.0/), versioning [SemVer](https://semver.org/lang/fr/).

---

## [Non publié] — dev

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

## Prototype initial — avril 2026 (non publié)

> Première version fonctionnelle mais rudimentaire — chat P2P local sur LAN.
> Jamais taguée : aucune release n'a encore été publiée (voir le README).

### Ajouté
- Chat en réseau local : découverte UDP broadcast, une connexion TCP par paquet
- Découverte automatique des pairs par subnet
- Persistance des messages en JSON local (réécriture complète du fichier)
- Interface graphique native egui
