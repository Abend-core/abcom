# Déploiement et installation

> [🏠 Accueil](../README.md)

## 🌱 Cible du déploiement
Abcom est conçu pour un usage local en réseau privé. Le déploiement vise des postes de bureau Windows et Linux, sans infrastructure serveur centrale.

## 🔧 Installer et déployer
### Linux / macOS
```bash
cd /chemin/vers/abcom
cargo build --release
./target/release/abcom MonPseudo
```

### Windows
```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install-windows.ps1 -Username MonPseudo
```

### Distribution simplifiée
```bash
bash scripts/build-and-distribute.sh
```

Ce script doit créer une archive prête à partager et contenir le binaire `abcom`, les ressources `assets/`, et la documentation essentielle.

## ⚙️ Packaging Windows
Le script `scripts/install-windows.ps1` réalise :
- vérification de la présence de `cargo`,
- compilation `cargo build --release` si le binaire n’existe pas,
- copie de `abcom.exe` dans `%LOCALAPPDATA%\Programs\abcom`,
- copie de `assets/app_icon.png`,
- création de raccourcis bureau et menu Démarrer.

> Le raccourci Windows transmet l’argument `-Username` au binaire.

## 🔧 Service Linux et déploiement
Le dépôt contient un service `contrib/abcom.service`, mais il n’est pas intégré dans le code principal. Pour une installation serveur Linux, il faut :
1. compiler le binaire release,
2. copier `target/release/abcom` dans `/usr/local/bin/abcom`,
3. placer le service systemd dans `/etc/systemd/system/abcom.service`,
4. démarrer avec `systemctl enable --now abcom`.

## ⚙️ Contraintes de distribution
- Abcom doit être lancé sur chaque poste, il n’y a pas de service central unique.
- Le protocole repose sur le LAN IPv4 et le broadcast UDP ; certains réseaux Wi-Fi ou VLAN peuvent bloquer la découverte.
- Les ports fixes sont : `9001` pour UDP discovery et `9000` pour TCP chat.

## 🔧 Vérification post-installation
- Vérifie que `abcom.exe` ou le binaire `abcom` existe dans le répertoire d’installation.
- Lance la fenêtre graphique : un ping UDP doit découvrir les pairs en quelques secondes.
- Si le binaire ne démarre pas sur Windows, vérifie que le raccourci cible pointe bien vers `%LOCALAPPDATA%\Programs\abcom\abcom.exe`.

## 🔧 Points d’amélioration possibles
- ajouter un package MSI ou un installeur ZIP Windows,
- centraliser les paramètres de port dans une configuration unique,
- documenter explicitement le support Linux `systemd` dans un guide séparé.
