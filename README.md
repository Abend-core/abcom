# Abcom

Messagerie instantanée pour réseau local (LAN), développée en Rust.

## Fonctionnement

Chaque machine du réseau installe et lance le même binaire. Les machines se découvrent automatiquement par broadcast UDP, les messages sont acheminés en TCP.

```
Machine A ──── réseau local ──── Machine B
  (UDP broadcast découverte)
  (TCP messages)
```

## Installation rapide

```bash
git clone https://github.com/Abend-core/abcom.git
cd abcom
bash install.sh
```

Le script :
1. Installe Rust si nécessaire
2. Compile en mode release
3. Installe le binaire dans `~/.local/bin/`
4. Configure un service systemd utilisateur (démarrage automatique)

## Lancement manuel

```bash
# Avec le nom d'utilisateur en argument (optionnel, $USER par défaut)
~/.local/bin/abcom
# ou
cargo run --release -- MonPrenom
```

## Désinstallation

```bash
bash uninstall.sh
```

## Ports utilisés

| Port | Protocole | Usage |
|------|-----------|-------|
| 9000 | TCP | Échange de messages |
| 9001 | UDP | Découverte des pairs |

Ouvrir ces ports dans le pare-feu si nécessaire :
```bash
sudo ufw allow 9000/tcp
sudo ufw allow 9001/udp
```

## Stack

- **Rust** + **Tokio** (async runtime)
- **egui / eframe** (interface graphique native)
- **serde_json** (sérialisation des messages)
- **chrono** (horodatage)
