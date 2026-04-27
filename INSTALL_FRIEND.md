# 📦 Abcom - Guide du Copain

Reçu le dossier d'Abcom ? Voici comment l'installer en 30 secondes.

---

## 🚀 Installation (Option 1: avec binaire pré-compilé)

### Étape 1: Lance le script
```bash
bash abcom-install.sh  abcom
```
ou
```bash
bash abcom-install.sh ./abcom
```

### Étape 2: C'est fait !
L'app est installée et prête.

---

## 🚀 Installation (Option 2: depuis le dépôt Git)

### Étape 1: Clone
```bash
git clone <lien-du-repo> ~/abcom
cd ~/abcom
```

### Étape 2: Installe
```bash
bash abcom-install.sh
```

### Étape 3: Redémarre le terminal
pour que le PATH soit mis à jour

---

## 🎯 Lancer Abcom

### Depuis le Terminal
```bash
abcom TonNom
# Exemple:
abcom Alice
abcom Bob
```

### Depuis le Menu Applications
- Ouvre ton **App Launcher** (Activities sur GNOME, Applications sur KDE)
- Cherche **Abcom**
- Clique, rentre un pseudo, et c'est parti !

### Au démarrage automatique (optionnel)
```bash
systemctl --user start abcom.service
```

---

## 💬 Comment ça marche

**Sur ta machine:**
1. Lance Abcom avec un pseudo unique (Alice, Bob, etc.)
2. Sur la **gauche**: tu vois tous les autres qui tournent sur le réseau (découverte auto)
3. En **haut**: les conversations ouvertes (Global + chaque personne)
4. **En bas**: écris tes messages
5. Click un nom sur la gauche → ouvert une conversation directe

**Entre machines:**
- Tous connectés au **même WiFi/LAN**
- Les messages arrivent tout seul entre les machines
- L'historique se sauvegarde localement

---

## ⚙️ Configuration

**Ports utilisés:**
- 9000/tcp (messages P2P direct)
- 9001/udp (découverte des pairs)

**Dossier de config:**
- `~/.local/share/abcom/messages.json` = historique

---

## 🆘 Problèmes

**"Je ne vois pas les autres pairs"**
- Vérifie que vous êtes sur le **même réseau Wi-Fi**
- Essaye: `ping 192.168.x.x` d'une machine à l'autre

**"Port 9000 déjà utilisé"**
- Une autre instance tourne: `pkill -f abcom`

**"Permission denied"**
- Réinstalle: `bash abcom-install.sh ./abcom`

---

## 📝 Remarques

- Les données sont **sauvegardées localement**, pas sur un serveur
- **Pas de compte**, **pas d'identifiant** (juste un pseudo pour ce session)
- Parfait pour un réseau local LAN
- Code ouvert sur GitHub

---

Besoin d'aide ? Demande au créateur ! 😊
