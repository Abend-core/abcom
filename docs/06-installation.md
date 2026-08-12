# 06 — Installation et déploiement

## Prérequis

- Rust stable (`rustup`), édition 2021.
- Réseau : les machines doivent être sur le même LAN, ports 9000-9001/tcp et 9001/udp ouverts entre elles.

### Dépendances système sous Linux

Elles sont **requises à la compilation**, pas seulement à l'exécution : sans
elles, `cargo build` échoue avant même d'atteindre notre code.

| Besoin | Debian / Ubuntu | Fedora | Arch | Alpine |
|---|---|---|---|---|
| Clavier, fenêtrage | `libxkbcommon-dev` | `libxkbcommon-devel` | `libxkbcommon` | `libxkbcommon-dev` |
| Son des notifications | `libasound2-dev` | `alsa-lib-devel` | `alsa-lib` | `alsa-lib-dev` |

**La dernière ligne est optionnelle** : voir les features ci-dessous.

L'icône résidente ne figure plus dans ce tableau : elle parle directement le
protocole D-Bus StatusNotifierItem (crate `ksni`, Rust pur), là où
`libappindicator` imposait GTK3 et libxdo. Rien à installer, ni pour compiler
ni pour exécuter.

### Compiler léger : `sound`

La seule dépendance C restante derrière une feature est ALSA. La désactiver
permet de compiler sur un système minimal — conteneur, image Alpine, poste sans
environnement de bureau.

```bash
# Sans son ni icône résidente
cargo build --release --no-default-features

# Sans le son uniquement : ALSA n'est plus requis
cargo build --release --no-default-features --features tray
```

Sans `tray`, fermer la fenêtre quitte réellement l'application au lieu de la
replier. Sans `sound`, le bip de notification disparaît ; les notifications
système, elles, restent (elles passent par D-Bus en Rust pur, sans dépendance C)
— tout comme le tray désormais.

### musl et Alpine

**Compilation dynamique contre musl** (Alpine, `x86_64-unknown-linux-musl` sans
lien statique) : fonctionne, avec les paquets du tableau ci-dessus. Aucun code
d'abcom ne dépend de la glibc.

**Binaire entièrement statique** : ce n'est **pas** possible, et ce n'est pas une
limite qu'on peut lever par configuration. Le rendu passe par `wgpu`, qui charge
le pilote Vulkan à l'exécution via `dlopen` — or `dlopen` ne fonctionne pas dans
un binaire musl statique. Le son (ALSA) ajoute la même contrainte. Un binaire
statique supposerait d'abandonner l'interface graphique.

En résumé, sur Alpine : installer les paquets, compiler normalement, obtenir un
binaire lié dynamiquement à musl. Réduire la surface avec
`--no-default-features` si ALSA n'est pas souhaité.

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
| `ABCOM_INSTANCE` | Numéro d'instance pour lancer plusieurs Abcom sur la même machine (ports et données séparés). Plafonné à ce que la plage de ports permet : une valeur trop grande est ramenée au maximum au lieu de déborder sur des ports déjà attribués |

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
- crée un lanceur desktop dans `~/.local/share/applications/`, pointant sur le chemin absolu du binaire (`%h` est un spécificateur systemd : dans un fichier `.desktop` il n'est pas développé et le raccourci ne lance rien) ;
- supprime un éventuel service systemd installé par une version antérieure.

### Lancement à l'ouverture de session

**Un seul mécanisme, géré par l'application** : une entrée XDG dans
`~/.config/autostart/`, activable et désactivable depuis Paramètres → Général.

Aucun service systemd n'est installé. Les versions antérieures en posaient un
*en plus* de l'entrée XDG : deux instances démarraient et se disputaient les
mêmes ports. Un service systemd serait de toute façon inadapté ici — il
démarrerait l'application hors de toute session graphique.

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
