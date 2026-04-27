> [🏠 Accueil](../README.md) > [🚚 CICD et déploiement](03-cicd-et-deploiement.md)

> 📅 **Généré le** : 2026-04-27  
> 🔖 **Stack analysée** : Rust 2021, tokio 1, serde 1, serde_json 1, eframe 0.31, egui 0.31, chrono 0.4, anyhow 1  
> 🔄 **À régénérer si** : refonte archi, changement majeur de stack, ajout/suppression de composant

# CICD et déploiement

## 🌱 Pour comprendre
Le dépôt ne contient pas de configuration CI/CD formelle (`.github/workflows`, `gitlab-ci.yml`, etc.). La stratégie de déploiement actuelle repose sur un `Makefile` local et un service systemd utilisateur dans `contrib/abcom.service`.

## 🔧 Pour utiliser
### Installation locale
```bash
make install
```

### Lancement manuel
```bash
~/.local/bin/abcom
```

### Démarrage systemd utilisateur
Le service est installé dans `~/.config/systemd/user/abcom.service` et activé par :
```bash
systemctl --user daemon-reload
systemctl --user enable --now abcom.service
```

### Arrêt et désinstallation
```bash
make uninstall
```

## ⚙️ Pour maîtriser
### Contenu du service systemd
- `ExecStart=%h/.local/bin/abcom`
- `Restart=on-failure`
- `PassEnvironment=DISPLAY WAYLAND_DISPLAY XDG_RUNTIME_DIR DBUS_SESSION_BUS_ADDRESS`
- `WantedBy=graphical-session.target`

### Limites du déploiement actuel
- Pas de gestion de versions ou de paquets dans un dépôt de packages.
- Pas de pipeline CI automatique dans le dépôt.
- Installation dépend d’un environnement Linux avec systemd et d’une session graphique.

### Améliorations possibles
- ajouter un workflow GitHub Actions ou GitLab CI pour construire et publier le binaire.
- packager l’application en `cargo deb`, `flatpak`, ou `AppImage`.
- introduire une vérification des versions du service au démarrage.

## 📚 Voir aussi
- [Developer Experience](02-developer-experience.md)
- [Architecture globale](01-architecture-globale.md)
- [Sécurité globale](04-securite-globale.md)
