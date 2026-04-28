> [🏠 Accueil](../README.md) > [🛠️ Developer Experience](02-developer-experience.md)

> 📅 **Généré le** : 2026-04-27  
> 🔖 **Stack analysée** : Rust 2021, tokio 1, serde 1, serde_json 1, eframe 0.31, egui 0.31, chrono 0.4, anyhow 1  
> 🔄 **À régénérer si** : refonte archi, changement majeur de stack, ajout/suppression de composant

# Developer Experience

## 🌱 Pour comprendre
Le dépôt est organisé autour d’un binaire unique `abcom`. La compilation utilise Cargo et la distribution profite d’un `Makefile` pour simplifier l’installation locale et l’activation du service systemd.

### Structure de base
- `Cargo.toml` : dépendances et binaire `abcom`.
- `src/` : code source Rust.
- `contrib/abcom.service` : service systemd utilisateur.
- `Makefile` : commandes `build`, `run`, `install`, `uninstall`.

## 🔧 Pour utiliser
### Compilation
```bash
cargo build
cargo build --release
```

### Exécution locale
```bash
make run
# ou
cargo run --release -- <username>
```

### Installation utilisateur
```bash
make install
```

Cette commande :
- compile en release,
- copie `target/release/abcom` vers `~/.local/bin/abcom`,
- installe `contrib/abcom.service` dans `~/.config/systemd/user/`,
- active et démarre le service user.

### Désinstallation
```bash
make uninstall
```

## ⚙️ Pour maîtriser
### Arborescence des modules
- `main.rs` orchestre les canaux et le runtime.
- `app.rs` contient `AppState` et la logique de gestion des pairs/messages.
- `discovery.rs` émet et reçoit des paquets UDP.
- `network.rs` gère le serveur TCP et l’envoi des messages.
- `ui.rs` gère l’interface `eframe`/`egui` et la boucle de rendu.

### Variables d’environnement et configuration
- `USER` : utilisé par `cargo run` et `Makefile`.
- `HOME` : pour l’installation dans `~/.local/bin`.
- Systemd passe `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, `DBUS_SESSION_BUS_ADDRESS`.

### Développement et contribution
- Les tests ne sont pas présents dans le dépôt actuel. Voir [Fiabilité et tests](../docs/abcom/04-fiabilite-et-tests.md).
- Les dépendances sont déclarées dans `Cargo.toml` et peuvent être mises à jour avec `cargo update`.

## 📚 Voir aussi
- [Architecture globale](01-architecture-globale.md)
- [Composant Abcom](../docs/abcom/README.md)
- [CICD et déploiement](03-cicd-et-deploiement.md)
