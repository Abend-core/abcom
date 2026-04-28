# 3 Options pour tester sur 3 machines

## Option A : 3 Vraies machines (recommandé)

Si tu as **3 PCs/laptops** sur le même WiFi/LAN :

1. **Machine 1 (Alice)** : `~/.local/bin/abcom Alice`
2. **Machine 2 (Bob)** : `~/.local/bin/abcom Bob`
3. **Machine 3 (Charlie)** : `~/.local/bin/abcom Charlie`

Elles découvriront automatiquement par broadcast UDP 9001.

---

## Option B : VirtualBox/KVM VMs (facile)

Crée 3 **VMs Linux** (Ubuntu Minimal suffit) :

### Étapes

1. **Installer Rust sur chaque VM** :
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
   ```

2. **Copier le repo Abcom** sur chaque VM

3. **Compiler** :
   ```bash
   cd abcom
   cargo build --release
   ```

4. **Lancer** sur chaque VM :
   ```bash
   ./target/release/abcom Alice
   ```

**Configuration réseau** : Mode **Bridge** sur toutes les VMs (pour partager le réseau host).

---

## Option C : Docker Compose (très facile)

Crée 3 containers sur la **même network** Docker :

### 1. Créer le Dockerfile

```dockerfile
FROM rust:latest

WORKDIR /app
COPY . .

RUN cargo build --release

ENTRYPOINT ["./target/release/abcom"]
```

### 2. Créer docker-compose.yml

```yaml
version: '3'
services:
  alice:
    build: .
    environment:
      - DISPLAY  # si tu veux GUI
    network_mode: host
    command: Alice

  bob:
    build: .
    environment:
      - DISPLAY
    network_mode: host
    command: Bob

  charlie:
    build: .
    environment:
      - DISPLAY
    network_mode: host
    command: Charlie
```

### 3. Lancer

```bash
docker-compose up
```

⚠️ **Limitation GUI** : Docker a du mal avec X11. Mieux avec des VMs.

---

## Option D : SSH sur VMs/Machines distantes

Si les machines sont sur le réseau mais distantes :

### Machine 1 (Local)
```bash
~/.local/bin/abcom Alice
```

### Machine 2 (SSH)
```bash
ssh user@192.168.1.20
~/.local/bin/abcom Bob
```

### Machine 3 (SSH)
```bash
ssh user@192.168.1.30
~/.local/bin/abcom Charlie
```

---

## 🎯 Recommandation pour TOI

Vu que tu as **une seule machine** (Merlin), voici les options rapides :

### ✅ **La plus facile** : Utiliser des amis

Envoie-leur le binaire (`~/.local/bin/abcom`) et dis-leur :
```
~/.local/bin/abcom Alice
```
Chacun lance sur sa machine, voilà !

### ✅ **Sans amis** : 3 VMs VirtualBox

1. Crée 3 VMs léger (Ubuntu Server, pas de GUI)
2. SSH dedans et lance Abcom
3. Elles sont sur le même réseau virtuel → découverte auto

### ✅ **Express** : Tester avec Docker

```bash
docker-compose up
```

(Limite : GUI difficile, mais les logs réseau visible)

---

## 📊 Tableau comparatif

| Option | Facilité | Réalisme | GUI | Temps |
|--------|----------|----------|-----|-------|
| Vraies machines | ⭐⭐ | ⭐⭐⭐ | ✅ | 5 min |
| VMs VirtualBox | ⭐⭐⭐ | ⭐⭐ | ✅ | 20 min |
| Docker Compose | ⭐⭐⭐ | ⭐ | ❌ | 5 min |
| SSH distants | ⭐⭐ | ⭐⭐⭐ | ✅ | 10 min |

---

## 🧪 Pour valider le test

Une fois les 3 instances lancées :

✅ **Découverte** : Chacun voit les 2 autres dans la liste gauche
✅ **Broadcast** : Message reçu par tous les 3
✅ **Direct** : Message reçu par 1 seul pair sélectionné
✅ **Historique** : Restart l'app, anciens messages persistent
✅ **Notifications** : Toast en haut-droit quand reçoit un message
✅ **Emojis** : Clique 😊, sélectionne un emoji dans le picker

---

**Quelle option préfères-tu ?** 🚀
