# 🚀 Déploiement Abcom - Résumé pour Toi

Tu as demandé : **Comment déployer rapidement sans compiler sur chaque machine ?**

Réponse : **3 options au choix !**

---

## 📋 TL;DR (Le plus rapide)

### Option A: Git Clone (LA PLUS SIMPLE!) ⭐

Sur chaque machine:
```bash
git clone https://github.com/rxdy/abcom.git
cd abcom
bash abcom-install.sh
```

C'est tout ! Zéro binaire à partager, zéro fichier ZIP.

### Option B: Distribution binaire 

Sur ta machine (compilation une fois):
```bash
bash build-and-distribute.sh
```

Puis envoie `dist/abcom_DATE.zip` aux copains (USB/email/cloud/SSH) et ils font:
```bash
unzip abcom_DATE.zip
cd abcom_DATE
bash abcom-install.sh ./abcom
```

### Option C: Installation directe (ta machine seulement)

```bash
make install   # Compile + install + service
```

---

## 🎯 Comparaison des 3 options:

| Critère | Git Clone | Binaire ZIP | Direct (make) |
|---------|-----------|-----------|--------------|
| **Simplicité** | ⭐⭐⭐⭐⭐ Le plus simple | ⭐⭐⭐⭐ | ⭐⭐⭐ |
| **Partage** | URL GitHub (pas de fichier) | ZIP à partager | Pas de partage |
| **Compilation requise?** | Une fois (localement) | Zéro | Oui |
| **Installation copains** | `bash abcom-install.sh` | `bash abcom-install.sh ./abcom` | N/A |
| **Fichier temporaire** | Zéro | ZIP (~5 MB) | Zéro |
| **Internet requis?** | OUI (clone) | NON (USB/email ok) | NON |
| **Meilleur pour** | Copains tech | Distribution physique | Développeur local |

---

## ✨ Option A: Git Clone (RECOMMANDÉE!)

### Sur ta machine (une fois):
```bash
# Pré-requis: Git installé (normalement oui)
git clone https://github.com/rxdy/abcom.git ~/abcom
cd ~/abcom
bash abcom-install.sh
```

### Sur les machines des copains:
```bash
# Ultra simple
git clone https://github.com/rxdy/abcom.git ~/abcom
cd ~/abcom
bash abcom-install.sh
```

**Avantages:**
✅ Partage via URL (pas de fichier énorme)
✅ Zéro configuration
✅ Les copains ont le code source (bonus!)
✅ Mise à jour facile: `git pull`

---

## 📦 Option B: Distribution binaire

### Sur ta machine (compilation + préparation):
```bash
bash build-and-distribute.sh
```

Crée un dossier `dist/` avec:
- Binaire compilé (`abcom`)
- Script d'installation (`abcom-install.sh`)
- Guides (`README.md`, `DEPLOY.md`)

**Résultat:** `dist/abcom_DATE.zip` (~5 MB)

### Envoie le ZIP aux copains:
- **Via USB** 💾
- **Via email/Discord** 📧
- **Via cloud (Drive, Nextcloud, etc.)** ☁️
- **Via SCP/SSH** 🔐

```bash
# Exemple: envoyer par SCP
scp dist/abcom_20260427_203548.zip alice@192.168.1.100:~/
```

---

### Sur les copains (quelques secondes):
```bash
unzip abcom_DATE.zip
cd abcom_DATE
bash abcom-install.sh ./abcom
source ~/.bashrc  # Met à jour le PATH
```

**Avantages:**
✅ Pas besoin de Git
✅ Distribution physique possible (USB)
✅ Zéro dépendance Rust sur les machines cibles
✅ Plus rapide si pas de connexion internet

---

## 🎮 Lancer l'app (une fois installée)

### Méthode 1: Terminal
```bash
abcom MonPseudo
```

### Méthode 2: Menu graphique ⭐ (le plus facile!)
- Ouvre ton **App Launcher** (Activities / Applications)
- Cherche **Abcom**
- Clique
- Rentre un pseudo
- C'est parti !

### Méthode 3: Service auto au démarrage (optionnel)
```bash
systemctl --user start abcom.service
```

---

## 📋 Récapitulatif des commandes Makefile

```bash
# Pour développer localement
make install           # Compile + install + service
make install-bin       # Install depuis binaire
make run               # Lance en dev

# Pour distribuer via binary ZIP
make deploy-bin        # Crée dist/abcom_DATE.zip

# Maintenance
make uninstall         # Supprime tout
make clean             # Nettoie target/
```

---

## 💾 Scripts d'installation

| Script | Usage | Pour qui |
|--------|-------|----------|
| `abcom-install.sh` | Installe depuis binaire (zéro compilation) | Copains + zip |
| `build-and-distribute.sh` | Prépare le ZIP de distribution | Toi (une fois) |

---

## 🎯 Choisir la meilleure option pour toi

### 👨‍💻 Si tes copains sont tech (Git déjà installé)
➜ **Option A: Git Clone** ⭐⭐⭐⭐⭐
```bash
git clone https://github.com/rxdy/abcom.git
cd abcom
bash abcom-install.sh
```
Le plus simple, pas de fichier à partager, mise à jour facile.

### 🎁 Si tu dois distribuer via USB/email/cloud
➜ **Option B: Binaire ZIP**
```bash
bash build-and-distribute.sh
# Envoie dist/abcom_DATE.zip
```
Pas besoin de Git, portable sur USB, zéro dépendance Rust.

### 💻 Si tu développes juste sur ta machine
➜ **Option C: Installation directe**
```bash
make install
```
Compile une fois, configuration complète, service auto.

---

## 🔗 Ressources

| Doc | Pour qui | Contenu |
|-----|----------|---------|
| [QUICK_DEPLOY.md](QUICK_DEPLOY.md) | Déploiement avancé | Options complètes, pare-feu, diagnostics |
| [INSTALL_FRIEND.md](INSTALL_FRIEND.md) | À envoyer aux copains | Guide simplifié pour les non-techs |
| [DEPLOYMENT.md](DEPLOYMENT.md) | Test multi-machines | Procédure complète 3 machines + troubleshooting |
| [TEST_DISTRIBUTION.md](TEST_DISTRIBUTION.md) | Validation finale | Checklist de test et essai |

---

## 🚀 Résumé ultra-rapide

**Toi:**
```bash
git clone https://github.com/rxdy/abcom.git ~/abcom
cd ~/abcom
bash abcom-install.sh
```

**Tes copains:**
```bash
git clone https://github.com/rxdy/abcom.git ~/abcom
cd ~/abcom
bash abcom-install.sh
```

**Tout le monde:**
```bash
abcom MonPseudo
# ou Applications → Abcom
```

**Et c'est tout !** ✨

---

## ℹ️ Infos importantes

- **Ports** : 9000/tcp (P2P) et 9001/udp (découverte)
- **Réseau** : Toutes les machines doivent être sur le **même LAN**
- **Pseudo** : Unique par machine (Alice, Bob, Charlie, etc.)
- **Données** : Sauvegardées dans `~/.local/share/abcom/messages.json`
- **Version** : v0.0.1 (première release stable)

---

**C'est parti ! 🚀**
