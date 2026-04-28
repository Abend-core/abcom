> [🏠 Accueil](../README.md) > [📖 Glossaire](05-glossaire.md)

> 📅 **Généré le** : 2026-04-27  
> 🔖 **Stack analysée** : Rust 2021, tokio 1, serde 1, serde_json 1, eframe 0.31, egui 0.31, chrono 0.4, anyhow 1  
> 🔄 **À régénérer si** : refonte archi, changement majeur de stack, ajout/suppression de composant

# Glossaire

## 🌱 Pour comprendre
Ce glossaire définit les termes techniques clés utilisés dans la documentation Abcom.

### LAN
Local Area Network. Un réseau local composé de machines sur une même plage d’adresses, généralement de confiance.

### UDP broadcast
Type de paquet UDP envoyé à l’adresse de diffusion du réseau local (`255.255.255.255`) pour annoncer la présence d’un hôte.

### TCP
Transmission Control Protocol. Protocole orienté connexion et fiable, utilisé ici pour envoyer les messages texte.

### Tokio
Runtime asynchrone Rust utilisé pour exécuter les tâches réseau en parallèle avec l’interface graphique.

### egui / eframe
Bibliothèque Rust pour interface graphique native. `eframe` fournit le conteneur d’application autour de `egui`.

### JSON
JavaScript Object Notation. Format de sérialisation utilisé pour les structures `DiscoveryPacket` et `ChatMessage`.

## 🔧 Pour utiliser
- `ChatMessage` : structure contenant `from`, `content`, `timestamp`.
- `DiscoveryPacket` : structure de découverte UDP contenant `username`.
- `AppEvent` : enum de communication interne entre le réseau et l’UI.

## ⚙️ Pour maîtriser
- Le protocole interne est simple :
  - UDP broadcast pour annoncer son pseudo,
  - TCP pour envoyer un message JSON à un pair.
- L’état local est géré par `AppState` dans `src/app.rs`.
- L’UI consomme `AppEvent` pour mettre à jour la liste des pairs et des messages.

## 📚 Voir aussi
- [Architecture globale](01-architecture-globale.md)
- [Mécanismes et données](abcom/02-mecanismes-et-donnees.md)
