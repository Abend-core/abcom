# Abcom

Messagerie instantanée pour réseau local (LAN), développée en Rust.

## Description du projet

Abcom est une application de chat pour machines sur le même réseau local. Chaque machine exécute le même programme, qui :

- découvre les autres pairs via **UDP broadcast** sur le réseau local,
- envoie et reçoit des messages via **TCP**,
- affiche une interface graphique native avec **egui**.

Le but est d’avoir un client simple à déployer sur plusieurs machines d’un même LAN.

## Installation avec Makefile

La manière recommandée est d’utiliser le `Makefile` fourni :

```bash
git clone https://github.com/Abend-core/abcom.git
cd abcom
make install
```

La commande `make install` fait :

1. compilation en `release`,
2. installation du binaire dans `~/.local/bin/abcom`,
3. installation du service systemd utilisateur,
4. activation du démarrage automatique à la connexion graphique.

## Lancer l'application

Après installation, la commande est :

```bash
~/.local/bin/abcom
```

Tu peux aussi lancer directement pendant le développement :

```bash
make run
```

ou

```bash
cargo run --release -- MonPrenom
```

## Désinstallation

```bash
make uninstall
```

## Services et démarrage automatique

Le projet contient un service systemd utilisateur `contrib/abcom.service`.

Après `make install`, le service est activé pour démarrer automatiquement avec la session graphique.

## Ports utilisés

| Port | Protocole | Usage |
|------|-----------|-------|
| 9000 | TCP | Échange de messages |
| 9001 | UDP | Découverte des pairs |

Si tu as un pare-feu actif :

```bash
sudo ufw allow 9000/tcp
sudo ufw allow 9001/udp
```

## Stack technique

- **Rust** + **Tokio** (runtime asynchrone)
- **egui / eframe** (interface graphique native)
- **serde_json** (sérialisation des messages)
- **chrono** (horodatage)
