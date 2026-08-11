# Abcom

> 📅 **Généré le** : 2026-04-28
> 🔖 **Stack analysée** : Rust 2021, tokio 1, serde 1, serde_json 1, eframe 0.31, egui 0.31, chrono 0.4, anyhow 1
> 🔄 **À régénérer si** : refonte de l’architecture, ajout d’un service ou d’un composant, migration vers un backend central

## 🎯 Pitch projet
Abcom est une application de messagerie instantanée conçue pour un réseau local (LAN). Le client fonctionne en mode peer-to-peer, découvre automatiquement les pairs via UDP broadcast et échange les messages au format JSON par TCP.

> Ancienne documentation archivée dans les fichiers `.old.md` pour assurer traçabilité.

## 🏗️ Architecture globale
Le projet est un monolithe Rust à exécution locale. L’application combine un runtime Tokio, un serveur TCP, un émetteur UDP de découverte, et une interface graphique native `egui`.

```mermaid
C4Context
    title Abcom — Vue système
    Person(user, "Utilisateur LAN", "Utilisateur d’une machine sur le LAN")
    System(abcom, "Abcom", "Application de chat LAN en Rust")
    System_Ext(network, "Réseau local", "Méthode de transport et de découverte")
    Rel(user, abcom, "utilise")
    Rel(abcom, network, "découvre et échange des messages via")
```

## 🚀 Quick start

### Développement
```bash
cargo run --release -- <username>
```

### Installation locale
```bash
make install
```

### Setup (une fois après le clone)
```bash
git config core.hooksPath .githooks
```
Active le hook pre-commit qui bloque les commits non formatés (`cargo fmt`).

### Déploiement utilisateur
```bash
bash scripts/abcom-install.sh ./target/release/abcom
systemctl --user enable --now abcom.service
```

### Mode distribution Docker
```bash
cd scripts/docker
docker compose up --build
```

## 📚 Sommaire exhaustif

- **Documentation globale**
  - [Architecture globale](2026-04-architecture-globale.md)
  - [Developer Experience](2026-04-developer-experience.md)
  - [CICD et déploiement](2026-04-cicd-et-deploiement.md)
  - [Sécurité globale](2026-04-securite-globale.md)
  - [Glossaire](2026-04-glossaire.md)
  - [Groupes — Phase 10](2026-07-spec-groupes.md)
  - [Installation Windows](2026-04-installation-windows.md)
  - Notes de migration (supprimé)
- **Décisions (ADR)**
  - [Choix du langage Rust et de la stack](2026-04-adr/ADR-001-langage-et-stack-rust.md)
  - [Architecture peer-to-peer sur LAN](2026-04-adr/ADR-002-architecture-lan-peer-to-peer.md)
- **Composant Abcom**
  - [Présentation du composant](2026-04-doc-generee/README.md)
  - [Architecture et structure](2026-04-doc-generee/01-architecture-et-structure.md)
  - [Mécanismes et données](2026-04-doc-generee/02-mecanismes-et-donnees.md)
  - [Performances et optimisations](2026-04-doc-generee/03-performances-et-optimisations.md)
  - [Fiabilité et tests](2026-04-doc-generee/04-fiabilite-et-tests.md)

## 🧭 Glossaire express

- [LAN](2026-04-glossaire.md#lan)
- [UDP broadcast](2026-04-glossaire.md#udp-broadcast)
- [TCP](2026-04-glossaire.md#tcp)
- [Tokio](2026-04-glossaire.md#tokio)
- [egui / eframe](2026-04-glossaire.md#egui--eframe)
- [systemd user](2026-04-glossaire.md#systemd-user)
