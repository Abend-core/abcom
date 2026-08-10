# Abcom

[![CI — dev](https://github.com/Abend-core/abcom/actions/workflows/ci-dev.yml/badge.svg?branch=dev)](https://github.com/Abend-core/abcom/actions/workflows/ci-dev.yml)
[![CI — main](https://github.com/Abend-core/abcom/actions/workflows/ci-main.yml/badge.svg?branch=main)](https://github.com/Abend-core/abcom/actions/workflows/ci-main.yml)
[![Licence AGPL-3.0](https://img.shields.io/badge/licence-AGPL--3.0-blue.svg)](LICENSE)

Messagerie instantanée pour réseau local, écrite en Rust. Les machines d'un même LAN se découvrent automatiquement et échangent messages, fichiers et médias en pair-à-pair, sans serveur, sans compte, sans connexion Internet. Tout le trafic est chiffré de bout en bout (Noise XX) et l'historique est stocké localement en SQLite.

Interface graphique native (egui), pensée pour tourner en permanence : la fenêtre se replie dans la barre de menus / zone de notification, l'application reste joignable et consomme quasiment rien au repos.

## Fonctionnalités

- **Conversations** : fil public « Tous », messages privés, salons de groupe avec gestion des membres
- **Messages riches** : Markdown, emojis (picker + `:shortcodes:`), réactions, réponses citées, indicateur de frappe
- **Accusés** : livraison (✓✓ gris) et lecture (✓✓ bleu) en privé, avec retransmission automatique
- **Fichiers et médias** : envoi de fichiers et dossiers (> 1 Go), acceptation par le destinataire, vignettes et visionneuse
- **GIF, mèmes, stickers** : sélecteur Klipy intégré
- **Sécurité** : identité X25519 par machine, chiffrement Noise XX, épinglage des clés (TOFU), passphrase de salon optionnelle
- **Résident** : fermeture = repli dans le tray, notifications système, badge non-lus, lancement automatique à l'ouverture de session
- **Bilingue** : interface FR/EN, thème clair/sombre
- **Hors ligne** : un message écrit à un pair absent part automatiquement à sa reconnexion
- **Local-first** : export texte d'une conversation et compaction de la base depuis Paramètres

## Démarrage rapide

```bash
# Une fois après le clone : active le hook pre-commit (cargo fmt)
git config core.hooksPath .githooks

# Lancer
cargo run --release -- <pseudo>

# Tester le P2P en local : deux instances sur la même machine
ABCOM_INSTANCE=1 cargo run --release -- alice   # terminal 1
ABCOM_INSTANCE=2 cargo run --release -- bob     # terminal 2
# ou : make run2  (raccourci de scripts/run-multi.sh)
```

Installation par plateforme (Linux/systemd, Windows, Docker) : voir [docs/06-installation.md](docs/06-installation.md).

## Documentation

| Document | Contenu |
|---|---|
| [01 — Présentation](docs/01-presentation.md) | Ce qu'est Abcom, comment ça marche, décisions fondatrices, vocabulaire |
| [02 — Architecture](docs/02-architecture.md) | Modules, threads, flux d'événements, rendu et caches UI |
| [03 — Réseau et sécurité](docs/03-reseau-et-securite.md) | Découverte, protocole, chiffrement, modèle de menace |
| [04 — Stockage](docs/04-stockage.md) | Base SQLite, schéma, médias, fichiers de données |
| [05 — Fonctionnalités](docs/05-fonctionnalites.md) | Comportement détaillé : conversations, groupes, accusés, médias, tray |
| [06 — Installation](docs/06-installation.md) | Linux, macOS, Windows, Docker, variables d'environnement |
| [07 — Développement](docs/07-developpement.md) | Build, tests, CI, workflow Git, dépendances et licences |
| [08 — Historique et audits](docs/08-historique-et-audits.md) | Phases du projet, audits menés, résultats mesurés |
| [09 — Limites et pistes](docs/09-limites-et-pistes.md) | Limites connues et travaux envisagés |
| [10 — Cahier de tests](docs/10-cahier-de-tests.md) | Tests manuels : fonctionnalités, régressions, spécificités par OS |

Pour contribuer : [CONTRIBUTING.md](CONTRIBUTING.md) (barrière verte, conventions, workflow Git). Le suivi au fil de l'eau est dans [CHANGELOG.md](CHANGELOG.md) et [AVANCEMENT.md](AVANCEMENT.md). Les documents historiques (audits et plans d'origine, ADR, anciennes versions) sont conservés tels quels dans [old/](old/).

## État du projet

Version **1.0.0-beta.1**, sous licence **AGPL-3.0**. 308 tests automatisés — dont un scénario P2P headless externe et un rendu headless de toute l'interface — et CI GitHub Actions sur Linux, macOS et Windows, avec MSRV, `cargo audit`, `cargo deny` et mesure de couverture.

Ce qui reste ouvert est listé dans [AUDIT.md](AUDIT.md) (dette de code) et [AUDIT-DEPENDANCES.md](AUDIT-DEPENDANCES.md) (ce que nos dépendances offrent et qu'on n'exploite pas).
