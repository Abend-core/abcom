# Changelog - Abcom

Toutes les versions et changements notables d'Abcom.

---

## [v0.0.1] - 2026-04-27

### 🎉 First Stable Release

#### ✨ Features
- **P2P LAN Chat**: Communication directe et broadcast sur réseau local
- **Auto Peer Discovery**: Découverte automatique via UDP broadcast (9001/udp)
- **Direct Messaging**: Conversations directes + mode broadcast global
- **Native GUI**: Interface native avec egui (Linux/Mac/Windows compatible)
- **Message Persistence**: Historique sauvegardé en JSON (~/.local/share/abcom/messages.json)
- **Typing Indicators**: Affichage "✍ X en train d'écrire..."
- **Toast Notifications**: Notifications pop-up 3 secondes haut-droit
- **Conversation Tabs**: Interface avec onglets (Global + conversation par pair)
- **Emoji Picker**: 128 emojis dans une grille 8x10
- **Desktop Integration**: 
  - Raccourci menu pour lancer l'app (Applications → Abcom)
  - Service systemd pour démarrage auto
  - Installation via `abcom-install.sh` (zéro dépendance Rust)

#### 📦 Deployment
- **Three deployment methods**:
  1. Git clone: `git clone https://github.com/rxdy/abcom.git && cd abcom && bash abcom-install.sh`
  2. Binary ZIP: Compile once, distribute as ZIP
  3. Direct install: `make install` for local setup
- **Scripts**:
  - `abcom-install.sh`: Installation from binary (zero compilation needed)
  - `build-and-distribute.sh`: Prepare distribution package (ZIP)
- **Documentation**:
  - DEPLOY_SIMPLE.md: Quick start guide
  - QUICK_DEPLOY.md: Detailed deployment options
  - INSTALL_FRIEND.md: Guide for sharing with friends
  - DEPLOYMENT.md: Complete guide with 3-machine test
  - TEST_DISTRIBUTION.md: Distribution process walkthrough

#### 🛠 Technical
- **Language**: Rust 2021 edition
- **Runtime**: tokio (async multi-threaded)
- **GUI**: egui 0.31 + eframe
- **Networking**: UDP broadcast + TCP P2P
- **Serialization**: serde + JSON
- **Time**: chrono for timestamps
- **Paths**: dirs crate for XDG-compliant data storage

#### 🐛 Known Limitations
- Broadcast assumes unicast for network with bridges (multi-subnet LAN)
- No user authentication or encryption
- Single file format for persistence
- UI message ordering is receive-based (no timestamp sorting)
- Typing indicators timeout manually (no heartbeat)

#### 📝 Code Quality
- 7 atomic commits with clear separation of concerns
- Well-documented deployment process
- Multi-machine tested (2 real machines successful)
- Clean error handling with anyhow
- Proper resource cleanup (Drop, mutex guards)

#### 📊 Release Stats
- **Binary size**: ~16 MB (uncompressed), ~5 MB (ZIP compressed)
- **Compile time**: ~1.5 seconds (release mode)
- **Install time**: ~30 seconds (no compilation on target machines)
- **Dependencies**: 15 total crates

---

## Versions Plannifiées

### [v0.1.0] - TODO
- [ ] End-to-end encryption
- [ ] User authentication/profiles
- [ ] Message search
- [ ] Multi-LAN bridge support
- [ ] Web UI (optional)
- [ ] Mobile app (optional)

### [v0.2.0] - TODO
- [ ] File transfer
- [ ] Voice/video (WebRTC)
- [ ] Groups/channels
- [ ] User presence/status
- [ ] Message reactions/edits

---

## Contributing

Pour contribuer:
1. Fork le dépôt: https://github.com/rxdy/abcom
2. Crée une branche: `git checkout -b feature/ma-feature`
3. Commit tes changements
4. Push et ouvre une Pull Request

---

## License

MIT License - voir LICENSE file
