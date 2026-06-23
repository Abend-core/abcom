# Changelog — Abcom

Toutes les modifications notables sont documentées ici.  
Format basé sur [Keep a Changelog](https://keepachangelog.com/fr/1.0.0/), versioning [SemVer](https://semver.org/lang/fr/).

---

## [Non publié] — dev

### Ajouté
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
- 111 tests unitaires couvrant tous les modules

### Corrigé
- Boucle infinie CPU causée par `has_unread` en arrière-plan
- Son de notification bloquant le thread UI (`sleep_until_end` → thread dédié)
- Crash au démarrage lors de la détection réseau sans pairs
- Rendu des emojis/glyphes non supportés dans certains terminaux

### Refactorisé
- Architecture atomique : `app.rs`, `ui.rs`, `message.rs`, `network.rs` découpés en sous-modules

---

## [0.1.0] — 2025-XX-XX

> Première version fonctionnelle — chat P2P local en réseau LAN.

### Ajouté
- Chat 1-à-1 et groupes en réseau local (UDP broadcast + TCP)
- Découverte automatique des pairs par subnet
- Persistance des messages (JSON local)
- Interface TUI avec ratatui/egui
- Détection réseau par SSID (hotspot iPhone inclus)
