> [🏠 Accueil](../README.md) > [🔒 Sécurité globale](04-securite-globale.md)

> 📅 **Généré le** : 2026-04-27  
> 🔖 **Stack analysée** : Rust 2021, tokio 1, serde 1, serde_json 1, eframe 0.31, egui 0.31, chrono 0.4, anyhow 1  
> 🔄 **À régénérer si** : refonte archi, changement majeur de stack, ajout/suppression de composant

# Sécurité globale

## 🌱 Pour comprendre
Abcom est conçu pour fonctionner sur un réseau local sécurisé. Le modèle de menace se concentre sur la découverte de pairs et l’échange de messages en clair, sans chiffrement ni authentification des utilisateurs.

## 🔧 Pour utiliser
### Ports nécessaires
- `9001/udp` : découverte de pairs.
- `9000/tcp` : messages.

Autoriser avec un pare-feu local :
```bash
sudo ufw allow 9000/tcp
sudo ufw allow 9001/udp
```

### Environnement systemd
Le service utilisateur passe les variables graphiques essentielles :
- `DISPLAY`
- `WAYLAND_DISPLAY`
- `XDG_RUNTIME_DIR`
- `DBUS_SESSION_BUS_ADDRESS`

## ⚙️ Pour maîtriser
### Principaux risques
- Absence d’authentification : toute machine sur le LAN peut potentiellement envoyer des messages.
- Pas de chiffrement : les messages JSON transitent en clair sur TCP.
- Broadcast UDP ouvert : le protocole découvre tous les pairs sur le segment de réseau.

### Recommandations
- Utiliser Abcom uniquement sur des LAN de confiance.
- Limiter l’accès réseau aux ports `9000` et `9001`.
- Envisager un mécanisme d’authentification ou de signature des messages si le projet évolue.

### À compléter
- Tests de sécurité réseau et traitement des paquets malformés.  
- Politique de renouvellement des dépendances de sécurité.  
- Audit de la bibliothèque `egui` et de `eframe` vis-à-vis des surfaces d’attaque GUI.

## 📚 Voir aussi
- [CICD et déploiement](03-cicd-et-deploiement.md)
- [Architecture globale](01-architecture-globale.md)
- [Composant Abcom](../docs/abcom/README.md)
