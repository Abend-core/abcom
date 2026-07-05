> [🏠 Accueil](../README.md) > [🔒 Sécurité globale](04-securite-globale.md)

> 📅 **Généré le** : 2026-04-28
> 🔖 **Stack analysée** : Rust 2021, tokio 1, serde 1, serde_json 1, eframe 0.31, egui 0.31, chrono 0.4, anyhow 1
> 🔄 **À régénérer si** : chiffrement réseau ajouté, authentification, architecture client-serveur

# Sécurité globale

## 🌱 Modèle de menace
Abcom est conçu pour un réseau local de confiance. Il n’offre pas de chiffrement natif, pas d’authentification des utilisateurs et repose sur un environnement LAN non segmenté.

### Risques principaux
- divulgation en clair des messages sur le LAN,
- usurpation d’identité d’un pair si deux machines utilisent le même nom,
- dépendance à l’ouverture des ports `9000/tcp` et `9001/udp`.

## 🔧 Bonnes pratiques de déploiement
### Ports à ouvrir / vérifier
- `9001/udp` : découverte de pairs.
- `9000/tcp` : échanges de messages.

Sur Linux avec `ufw` :
```bash
sudo ufw allow 9000/tcp
sudo ufw allow 9001/udp
```

### Service `systemd` utilisateur
Le service est installé dans `~/.config/systemd/user/abcom.service` et est prévu pour une session graphique. Il n’exécute pas de processus root.

### Recommandations
- utiliser Abcom uniquement sur un LAN de confiance,
- éviter les réseaux publics sans VPN,
- assigner des noms d’utilisateur uniques pour limiter les collisions.

## ⚙️ Limites actuelles
- pas de chiffrement de bout en bout,
- pas de signature ou de validation d’identité,
- pas de gestion de session ni de liste de pairs bloqués.

> [À COMPLÉTER PAR L'ÉQUIPE] : modèle de menace formel, acceptation des risques et stratégie de durcissement.
