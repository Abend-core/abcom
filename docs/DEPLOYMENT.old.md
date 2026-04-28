# Guide de déploiement et test sur 3 machines

## 🎯 Objectif
Tester **Abcom** sur 3 machines différentes connectées au même réseau LAN pour vérifier :
- ✅ Découverte automatique des pairs via UDP broadcast
- ✅ Communication TCP P2P ou broadcast
- ✅ Historique persistant (sauvegarde locale)
- ✅ Indicateurs de frappe
- ✅ Notifications

---

## 📋 Prérequis

- **3 machines** sur le **même réseau LAN** (192.168.x.x ou similaire)
- **Rust 1.95+** ou un **binaire pré-compilé** d'Abcom
- **Ports ouverts** : `9000/tcp` et `9001/udp` (localement, pas de firewall restrictif)

---

## 🚀 Déploiement

### Étape 1 : Construire le binaire

Sur la **machine 1** (ou n'importe laquelle) :

```bash
cd /chemin/vers/abcom
source ~/.cargo/env
cargo build --release
```

Le binaire est généré dans `target/release/abcom` (~5 MB après strip).

### Étape 2 : Déployer sur les 3 machines

Copie le binaire `target/release/abcom` sur chaque machine :

**Machine 1 (Alice)** :
```bash
mkdir -p ~/.local/bin
cp abcom ~/.local/bin/abcom
chmod +x ~/.local/bin/abcom
```

**Machine 2 (Bob)** :
```bash
mkdir -p ~/.local/bin
cp abcom ~/.local/bin/abcom
chmod +x ~/.local/bin/abcom
```

**Machine 3 (Charlie)** :
```bash
mkdir -p ~/.local/bin
cp abcom ~/.local/bin/abcom
chmod +x ~/.local/bin/abcom
```

### Étape 3 : Vérifier la connectivité réseau

Depuis **n'importe quelle machine** :

```bash
# Voir toutes les machines du LAN
ip addr show

# Ping entre machines (ex: 192.168.1.10 à 192.168.1.20)
ping 192.168.1.10
```

---

## 🧪 Lancer le test

### Sur Machine 1 (Alice)

```bash
~/.local/bin/abcom Alice
```

La fenêtre egui s'ouvre avec :
- Panneau gauche : "En attente de pairs..."
- Zone centrale : vide (aucun message)
- Barre basse : prêt pour écrire

### Sur Machine 2 (Bob)

```bash
~/.local/bin/abcom Bob
```

**À ce moment** :
- Alice devrait voir **● Bob** dans son panneau gauche
- Bob devrait voir **● Alice** dans son panneau gauche

### Sur Machine 3 (Charlie)

```bash
~/.local/bin/abcom Charlie
```

**Résultat** :
- Chacun voit les 2 autres dans la liste

---

## 💬 Tester la communication

### Envoyer en broadcast (à tous)

1. Sur **Alice**, ne sélectionne personne à gauche
2. Tape : `Salut à tous ! 😊`
3. Clique **Envoyer** ou presse **Entrée**

**Résultat attendu** :
- ✅ Alice voit son message dans la zone centrale
- ✅ Bob & Charlie reçoivent et voient le message
- ✅ Notification toast apparaît (3 sec haut-droit)
- ✅ Message sauvegardé dans `~/.local/share/abcom/messages.json`

### Envoyer en direct (1 seul)

1. Sur **Bob**, sélectionne **Alice** (clique sur le nom gauche)
2. La barre basse affiche `→ Alice`
3. Tape : `Coucou Alice !`
4. Clique **Envoyer**

**Résultat attendu** :
- ✅ Bob voit son message
- ✅ Alice reçoit le message de Bob uniquement
- ✅ Charlie **ne reçoit pas** le message (envoi direct)

---

## 🧪 Cas de test recommandés

| Cas | Étapes | Résultat attendu |
|-----|--------|-----------------|
| **Découverte** | Lancé 3 instances | Chacun voit les 2 autres dans la liste |
| **Broadcast** | Tape message sans sélection | Tous le reçoivent |
| **Direct** | Sélectionne 1 pair, tape message | Seul ce pair le reçoit |
| **Historique** | Redémarre une app, puis relance | Anciens messages présents |
| **Notification** | Reçoit un message | Toast 3 sec en haut-droit |
| **Emojis** | Clique 😊, sélectionne emoji | Emoji inséré dans le message |

---

## 🔧 Dépannage

### "Address already in use (os error 98)"

**Cause** : Un autre Abcom tourne encore sur ce port.

**Solution** :
```bash
pkill -9 abcom
# Attends 2 secondes
~/.local/bin/abcom Alice
```

### Les pairs ne se découvrent pas

**Cause** : Machines sur réseaux différents ou firewall bloquant UDP `9001`.

**Solution** :
```bash
# Vérifie que tu es sur le même réseau
ip route show
ping 192.168.1.X  # remplace par IP d'une autre machine

# Si bloqué par firewall
sudo ufw allow 9001/udp
sudo ufw allow 9000/tcp
```

### Les messages n'arrivent pas

**Cause** : Firewall TCP `9000` bloqué.

**Solution** :
```bash
sudo ufw allow 9000/tcp
```

---

## 📊 Ce que tu vas observer

### Après 3-5 secondes

```
┌─────────────────────────────────────────┐
│ Abcom                                   │
├────────┬───────────────────────────────┤
│ Pairs  │ [14:22] Alice: Salut tous ! 😊 │
│ LAN    │ [14:23] Bob: Yop !              │
│ ---    │ [14:24] Charlie: Coucou !       │
│ ● Alice│                                 │
│ ● Bob  │ ✍ Bob, Charlie typing...       │
│ ● Char │                                 │
│        ├───────────────────────────────┤
│ Connecté│ → tous                         │
│ rxdy   │ [Écrire un message...] 😊 Env │
└────────┴───────────────────────────────┘
```

---

## 🎓 Pour aller plus loin

- Modifiez les messages avec **emojis**, testez la **grille de sélection**
- Vérifiez que `~/.local/share/abcom/messages.json` se crée et persiste
- Testez les **indicateurs de frappe** : tapez lentement et observez le panneau haut
- Fermez une instance et relancez-la : l'historique revient immédiatement

---

## 📝 Notes techniques

- **Découverte** : UDP broadcast `255.255.255.255:9001` toutes les 3 secondes
- **Messages** : TCP point-à-point `<ip>:9000` ou broadcast si pas de sélection
- **Persistance** : JSON local, pas de serveur central
- **Typage** : Architecture théorique (pas d'envoi pour l'instant)
- **Notifications** : Toast haut-droit, 3 secondes auto-dismiss

---

## ✅ Checklist de test

- [ ] 3 machines découvrent bien les autres
- [ ] Broadcast fonctionne (tous reçoivent)
- [ ] Direct fonctionne (1 seul reçoit)
- [ ] Historique persiste après redémarrage
- [ ] Notifications toast s'affichent
- [ ] Emojis s'insertent correctement
- [ ] Pas de crashes après 10 minutes
