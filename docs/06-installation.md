# 06 — Installation et déploiement

## Prérequis

- Rust stable (`rustup`), édition 2021.
- Linux : paquets de développement audio et clavier (`libasound2-dev`, `libxkbcommon-dev` sur Debian/Ubuntu — les mêmes que la CI).
- Réseau : les machines doivent être sur le même LAN, ports 9000-9001/tcp et 9001/udp ouverts entre elles.

## Lancer depuis les sources (toutes plateformes)

```bash
cargo run --release -- <pseudo>
```

Sans argument, le pseudo reprend la variable `USER`/`USERNAME`. Le binaire compilé se trouve dans `target/release/abcom`.

### Variables d'environnement

Lues depuis l'environnement ou un fichier `.env` à la racine du répertoire courant :

| Variable | Rôle |
|---|---|
| `ABCOM_PASSPHRASE` | Passphrase de salon : seules les machines partageant la même valeur peuvent se connecter entre elles (handshake `XXpsk3`) |
| `ABCOM_KLIPY_API_KEY` | Clé API Klipy — nécessaire au sélecteur GIF/mèmes/stickers |
| `ABCOM_INSTANCE` | Numéro d'instance pour lancer plusieurs Abcom sur la même machine (ports et données séparés) |

### Tester le P2P en local

```bash
bash scripts/run-multi.sh          # lance plusieurs instances prêtes à se découvrir
# ou manuellement :
ABCOM_INSTANCE=1 cargo run --release -- alice
ABCOM_INSTANCE=2 cargo run --release -- bob
```

## Linux

```bash
make install
```

compile en release puis :

- copie le binaire dans `~/.local/bin/abcom` ;
- installe le service utilisateur [contrib/abcom.service](../contrib/abcom.service) dans `~/.config/systemd/user/` ;
- crée un lanceur desktop dans `~/.local/share/applications/`.

Activation du service (session graphique requise, aucun droit root) :

```bash
systemctl --user daemon-reload
systemctl --user enable --now abcom.service
```

Pour installer un binaire déjà compilé sur une autre machine : `bash scripts/abcom-install.sh ./target/release/abcom`. Désinstallation : `make uninstall` ou `scripts/uninstall.sh`.

Pare-feu (ufw) :

```bash
sudo ufw allow 9000:9001/tcp
sudo ufw allow 9001/udp
```

## macOS

```bash
cargo run --release -- <pseudo>
```

L'application gère elle-même le mode résident (barre de menus, retrait du Dock quand la fenêtre est cachée) et propose l'autostart via un Launch Agent. Il n'y a pas encore de bundle `.app` : conséquence connue, les notifications système sont attribuées au terminal plutôt qu'à Abcom. Le packaging `.app` (identifiant `com.abend.abcom`, via cargo-bundle ou cargo-packager) est prévu pour la première release.

## Windows

Compiler et installer depuis PowerShell (pas depuis WSL, sinon l'application ne s'affichera pas sur le bureau Windows) :

```powershell
cd C:\chemin\vers\abcom
powershell -ExecutionPolicy Bypass -File .\scripts\install-windows.ps1
```

Le script vérifie `cargo`, compile en release si nécessaire, installe `abcom.exe` dans `%LOCALAPPDATA%\Programs\abcom`, et crée des raccourcis sur le bureau et dans le menu Démarrer. Pour épingler à la barre des tâches : lancer l'application puis clic droit sur son icône → « Épingler ». Un installateur MSI reste à faire.

## Docker (tests multi-pairs)

```bash
cd scripts/docker
docker compose up --build
```

Le [Dockerfile](../scripts/docker/Dockerfile) construit une image Rust avec les dépendances graphiques d'egui ; le compose lance trois services (`alice`, `bob`, `charlie`) en `network_mode: host`. Utile pour des tests isolés ; nécessite un accès au serveur X11 de l'hôte pour l'affichage.

## Cibles Makefile utiles

| Cible | Effet |
|---|---|
| `make build` / `make release` | Compilation debug / release |
| `make run` | Lance l'application |
| `make run-multi` | Plusieurs instances locales |
| `make install` / `make uninstall` | Installation Linux complète / retrait |
| `make check` | Formatage, Clippy toutes cibles/features et tests |
| `make test` / `make test-verbose` | Tests (normal / sortie complète) |
| `make test-module M=app::groups` | Tests d'un module |
