# Déploiement Rapide d'Abcom

Déploiement simple et rapide sur toutes tes machines du réseau LAN.

---

## 🎯 Cas 1 : Première machine (compilation)

Compile une seule fois sur une machine qui a Rust:

```bash
cd /chemin/vers/abcom
cargo build --release
# Résultat: target/release/abcom (~5 MB)
```

---

## 🎯 Cas 2 : Autres machines (installation rapide)

### Depuis ta machine dev (partage le binaire):

```bash
# Copie le binaire à partager
cp /home/rxdy/dev/abcom/target/release/abcom /tmp/abcom

# Exemple: partage via Samba, USB, SSH, SCP...
# Ou simplement: compress et envoie aux copains
```

### Sur chaque machine cible:

#### Option A : Depuis un USB/fichier reçu
```bash
# 1. Place le fichier abcom quelque part
cp ~/Downloads/abcom /tmp/abcom

# 2. Lance le script d'installation
bash /tmp/abcom-install.sh /tmp/abcom

# C'est fait ! L'app est prête
```

#### Option B : Depuis le dépôt Git
```bash
# Clone depuis ton serveur/fork GitHub
git clone <ton-repo> ~/abcom
cd ~/abcom

# Lance le script d'installation (compile localement)
bash install.sh
```

---

## 🚀 Lancer l'app

### Mode 1 : Ligne de commande
```bash
abcom Alice      # Démarrer avec le pseudo "Alice"
```

### Mode 2 : Raccourci menu (Linux)
- L'installation crée un raccourci **Abcom** dans ton menu Applications
- Clique simplement dessus depuis ton app launcher (Activities/Menu)
- Il te demande ton pseudo au démarrage

### Mode 3 : Service automatique (optionnel)
```bash
# Lancer au démarrage de la session
systemctl --user start abcom.service

# Vérifier le statut
systemctl --user status abcom.service
```

---

## ⚙️ Configuration

### Ports utilisés
- **9000/tcp** : Communication TCP P2P directe
- **9001/udp** : Découverte automatique des pairs (broadcast)

### Données sauvegardées
- **~/.local/share/abcom/messages.json** : Historique des messages
- **~/.config/systemd/user/abcom.service** : Service de démarrage auto

---

## 🔥 Désinstallation

```bash
# Arrête et désactive l'app
systemctl --user disable --now abcom.service

# Supprime les fichiers
rm ~/.local/bin/abcom
rm -rf ~/.local/share/abcom
rm ~/.config/systemd/user/abcom.service

# Remove desktop shortcut
rm ~/.local/share/applications/abcom.desktop
```

---

## ✅ Checkliste de déploiement

- [ ] 1 machine a compilé le binaire (`cargo build --release`)
- [ ] Binaire copié sur les autres machines
- [ ] `abcom-install.sh` lancé sur chaque machine
- [ ] Vérification: toutes les machines pingent la même adresse IP (LAN)
- [ ] Lancé l'app sur chaque machine avec un pseudo unique
- [ ] Vérification: les pairs sont découverts automatiquement (liste à gauche)
- [ ] Envoyé un message → reçu sur l'autre machine ✅

---

## 🆘 Besoin d'aide ?

### Erreur: "Pairs not connecting"
- Vérifier que toutes les machines sont sur le **même réseau LAN** (192.168.x.x)
- Vérifier pas de firewall bloquant **9000/tcp** et **9001/udp**

### Erreur: "Cannot bind port 9000"
- Peut-être une autre instance d'Abcom tourne déjà
- `pkill -f abcom` pour tuer les instances

### Erreur: Cannot execute binary
- Le binaire n'a pas les permissions: `chmod +x ~/.local/bin/abcom`
