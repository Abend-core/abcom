> [🏠 Accueil](../../README.md) > [📦 Composant Abcom](README.md)

> 📅 **Généré le** : 2026-04-28
> 🔖 **Stack analysée** : Rust 2021, tokio 1, serde 1, serde_json 1, eframe 0.31, egui 0.31, chrono 0.4, anyhow 1
> 🔄 **À régénérer si** : refonte du cœur applicatif, découpage en plusieurs binaires, modification des modules

# Composant Abcom

## 🌱 Vue du composant
Le composant `abcom` rassemble l’intégralité de l’application : découverte LAN, communication TCP, interface utilisateur et stockage local.

### Types de responsabilités
- découverte de pairs -> `src/discovery.rs`
- réseau et transmission de messages -> `src/network.rs`
- état applicatif et historique -> `src/app.rs`
- interface graphique -> `src/ui.rs`
- format de message et événements -> `src/message.rs`

## 🔧 Points d’entrée
### Lancement
```bash
cargo run --release -- <username>
```

### Installation
```bash
make install
```

## ⚙️ Maîtriser le composant
### Documentation interne
- [Architecture et structure](01-architecture-et-structure.md)
- [Mécanismes et données](02-mecanismes-et-donnees.md)
- [Performances et optimisations](03-performances-et-optimisations.md)
- [Fiabilité et tests](04-fiabilite-et-tests.md)

### Principaux fichiers
- `src/main.rs` : orchestration du runtime et des canaux Tokio.
- `src/app.rs` : état partagé, filtres de conversation, persistance JSON.
- `src/discovery.rs` : découverte des pairs par broadcast UDP.
- `src/network.rs` : réception TCP et envoi TCP.
- `src/ui.rs` : vue, saisie, notifications et emoji picker.
