# 🚀 Déploiement Abcom - Résumé pour Toi

Tu as demandé : **Comment déployer rapidement sans compiler sur chaque machine ?**

Réponse : **Compile une fois, distribue partout en 30 sec !**

---

## 📋 TL;DR (Le plus rapide)

### Sur ta machine (compilation + préparation):
```bash
cd ~/dev/abcom
bash build-and-distribute.sh
```

Et c'est fait ! Un ZIP prêt à envoyer à tes copains.

---

## 🎯 Processus complet

### **Étape 1: Préparer la distribution** (ta machine)

```bash
bash build-and-distribute.sh
```

Crée un dossier `dist/` avec:
- Binaire compilé (`abcom`)
- Script d'installation (`abcom-install.sh`)
- Guides (`README.md`, `DEPLOY.md`)

**Résultat:** `dist/abcom_DATE.zip` (~5 MB)

---

### **Étape 2: Partager le ZIP** 

Envoie le ZIP aux copains:
- **Via USB** 💾
- **Via email/Discord** 📧
- **Via cloud (Drive, Nextcloud, etc.)** ☁️
- **Via SCP/SSH** 🔐

```bash
# Exemple: envoyer par SCP
scp dist/abcom_20260427_203548.zip alice@192.168.1.100:~/
```

---

### **Étape 3: Les copains installent** (sur leur machine)

```bash
# 1. Décompresse
unzip abcom_20260427_203548.zip
cd abcom_20260427_203548

# 2. Installation (une seule commande!)
bash abcom-install.sh ./abcom

# 3. Redémarre le terminal
source ~/.bashrc
```

Et c'est tout ! Aucune compilation, aucune dépendance Rust.

---

## 🎮 Lancer l'app

### Option 1: Terminal
```bash
abcom MonPseudo
```

### Option 2: Menu graphique ⭐ (le plus cool !)
- Ouvre ton **App Launcher** (Activities / Applications)
- Cherche **Abcom**
- Clique
- Rentre un pseudo
- C'est parti !

---

## 📋 Récapitulatif des commandes Makefile

```bash
# Compilation full (dev + service + raccourci)
make install

# Préparation pour distribution (création du ZIP)
make deploy-bin

# Installation depuis binaire (sans compiler)
make install-bin

# Désinstallation
make uninstall

# Nettoyage
make clean
```

---

## 🎁 Fichiers de déploiement créés

| Fichier | Description |
|---------|-------------|
| `QUICK_DEPLOY.md` | Guide déploiement minimal |
| `abcom-install.sh` | Script d'installation (ne compile pas) |
| `build-and-distribute.sh` | Prépare le ZIP de distribution |
| `INSTALL_FRIEND.md` | Guide pour les copains |
| `contrib/abcom.desktop` | Raccourci menu + icône |

---

## ✨ Ce que tu gagnes

✅ **Compile une seule fois** → binaire réutilisable  
✅ **ZIP auto-généré** → facile à partager  
✅ **Zéro dépendance Rust** sur les autres machines  
✅ **Installation en 30 sec** → `bash abcom-install.sh ./abcom`  
✅ **Raccourci menu** → pas besoin terminal  
✅ **Démarrage auto** (optionnel) → service systemd  

---

## 🚨 Points importants

- **Ports** : 9000/tcp (P2P) et 9001/udp (découverte)
- **Réseau** : Toutes les machines doivent être sur le **même LAN**
- **Pseudo** : Unique par machine (Alice, Bob, Charlie, etc.)
- **Données** : Sauvegardées dans `~/.local/share/abcom/messages.json`

---

## 🔥 Prochaines étapes

1. Lance `bash build-and-distribute.sh`
2. Envoie le ZIP à tes copains (USB/email/etc.)
3. Ils lancent le script d'installation
4. C'est fait ! Les machines se découvrent auto sur le LAN

---

Besoin d'aide ? Consulte:
- `QUICK_DEPLOY.md` - Guide détaillé du déploiement
- `INSTALL_FRIEND.md` - Pour partager avec les copains
- `DEPLOYMENT.md` - Doc technique complète (tests 3 machines, firewall, etc.)

**C'est parti ! 🚀**
