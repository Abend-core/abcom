> [🏠 Accueil](../README.md) > [🛠️ Developer Experience](02-developer-experience.md)

> 📅 **Généré le** : 2026-04-28
> 🔖 **Stack analysée** : Rust 2021, tokio 1, serde 1, serde_json 1, eframe 0.31, egui 0.31, chrono 0.4, anyhow 1
> 🔄 **À régénérer si** : ajout d’un workflow CI, nouvelle cible de packaging, changement de runtime graphique

# Developer Experience

## 🌱 Comprendre la structure du dépôt
Le projet est centré sur un binaire unique `abcom` décrit dans `Cargo.toml`. Le code est organisé en modules Rust, et l’interface graphique est native via `eframe`.

### Fichiers clefs
- `Cargo.toml` : configuration du package et dépendances.
- `src/` : code source applicatif.
- `Makefile` : commandes de build, run, install, uninstall.
- `contrib/abcom.service` : service `systemd` utilisateur.
- `scripts/abcom-install.sh` : installation locale pour utilisateur Linux.
- `scripts/install-windows.ps1` : installation Windows avec raccourcis.
- `scripts/docker/Dockerfile` et `scripts/docker/docker-compose.yml` : image et exécution Docker.

## 🔧 Build et lancement
### Compilation rapide
```bash
cargo build
```

### Compilation optimisée
```bash
cargo build --release
```

### Lancement local
```bash
cargo run --release -- <username>
```

### Installation utilisateur
```bash
make install
```

Cette commande :
- compile le binaire en release,
- copie `target/release/abcom` dans `~/.local/bin`,
- installe le service `contrib/abcom.service` dans `~/.config/systemd/user`,
- crée un lanceur desktop dans `~/.local/share/applications`.

## ⚙️ Environnements supportés
### Service systemd utilisateur
Le service `abcom.service` est prévu pour une session graphique et utilise :
- `DISPLAY`
- `WAYLAND_DISPLAY`
- `XDG_RUNTIME_DIR`
- `DBUS_SESSION_BUS_ADDRESS`

Il s’exécute en tant qu’utilisateur, sans besoin de `sudo`, via :
```bash
systemctl --user enable --now abcom.service
```

### Distribution par Docker
Les scripts Docker construisent une image à partir de `scripts/docker/Dockerfile` et exposent l’application dans le réseau hôte :
```bash
cd scripts/docker
docker compose up --build
```

### Installation sur Windows
Un guide dédié existe dans `docs/INSTALL_WINDOWS.md` et le script d’installation est `scripts/install-windows.ps1`.

### Conseils pratiques
- Vérifie que le dossier `~/.local/share/abcom` existe et est accessible en écriture.
- Si l’application ne démarre pas en service, lance-la d’abord en local pour détecter les erreurs d’affichage.
- Les noms d’utilisateur sont extraits de l’argument `username` ou de la variable d’environnement `USER`.
