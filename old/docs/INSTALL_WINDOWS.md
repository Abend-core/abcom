> [🏠 Accueil](../README.md) > [📦 Installation Windows](INSTALL_WINDOWS.md)

> 📅 **Généré le** : 2026-04-28
> 🔖 **Stack analysée** : Rust 2021, tokio 1, serde 1, serde_json 1, eframe 0.31, egui 0.31, chrono 0.4, anyhow 1
> 🔄 **À régénérer si** : ajout de packaging Windows, script d’installation, changement de cible de compilation

# Installation sur Windows

## 🌱 Contexte
Abcom est une application native Rust prévue pour s’exécuter sur un bureau Windows. Le déploiement Windows nécessite un binaire `.exe` et un raccourci natif, contrairement au service `systemd` Linux.

## 🔧 Installer et lancer sur Windows

### Prérequis
- Rust installé sur Windows (`rustup`, `cargo`).
- Une console PowerShell ou CMD ouverte dans le dossier du dépôt.
- Pas d’installation depuis WSL si tu veux voir l’appli dans le bureau Windows.

### Construction
```powershell
cd C:\chemin\vers\abcom
cargo build --release
```

### Exécuter l’application
```powershell
.	arget\release\abcom.exe MonPseudo
```

### Installation recommandée
Le dépôt contient un script PowerShell d’aide :
```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install-windows.ps1
```

Ce script :
- compile le binaire si nécessaire,
- installe `abcom.exe` dans `%LOCALAPPDATA%\Programs\abcom`,
- crée un raccourci sur le bureau Windows,
- crée un raccourci dans le menu Démarrer.

## ⚙️ Faire apparaître Abcom dans la barre des tâches
1. Lance Abcom depuis le raccourci créé sur le bureau ou dans le menu Démarrer.
2. Clique droit sur l’icône dans la barre des tâches.
3. Choisis `Épingler à la barre des tâches`.

## 🔧 Script d’installation Windows
Le script se trouve dans `scripts/install-windows.ps1`.

### Ce qu’il fait
- vérifie si `cargo` est disponible,
- compile le binaire `release` si nécessaire,
- crée l’arborescence `%LOCALAPPDATA%\Programs\abcom`,
- copie `abcom.exe` et le fichier `README.md` dans le répertoire d’installation,
- crée un raccourci bureau et un raccourci menu Démarrer.

## 📌 Remarques importantes
- Si tu utilises WSL, le binaire doit être construit et installé depuis Windows pour fonctionner correctement.
- Le service Linux `contrib/abcom.service` n’est pas applicable sur Windows.
- Si tu veux un installateur Windows complet, il faudra ajouter un package MSI ou un script d’archivage ZIP.
