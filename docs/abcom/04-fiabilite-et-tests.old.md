> [🏠 Accueil](../../README.md) > [📦 Composant Abcom](README.md) > [🧪 Fiabilité et tests](04-fiabilite-et-tests.md)

> 📅 **Généré le** : 2026-04-27  
> 🔖 **Stack analysée** : Rust 2021, tokio 1, serde 1, serde_json 1, eframe 0.31, egui 0.31, chrono 0.4, anyhow 1  
> 🔄 **À régénérer si** : refonte archi, changement majeur de stack, ajout/suppression de composant

# Fiabilité et tests

## 🌱 Pour comprendre
La fiabilité d’Abcom repose sur la robustesse du code réseau et de la boucle UI. Actuellement, le dépôt ne contient pas de tests automatisés ou de suite de validation.

## 🔧 Pour utiliser
### Gestion des erreurs
- `discovery.rs` : affiche les erreurs de bind et de broadcast sur stderr.
- `network.rs` : ignore les erreurs de connexion et de lecture, en les journalisant sur stderr.
- `ui.rs` : ne déclenche pas d’erreur visible à l’utilisateur en cas de problème réseau.

### Limitations connues
- Pas de tests unitaires ou d’intégration dans le dépôt actuel.
- Pas de validation explicite des données reçues sur le réseau.
- Aucun mécanisme de retry ou de délai exponentiel pour les connexions TCP.

## ⚙️ Pour maîtriser
### Points d’amélioration
- Ajouter des tests unitaires pour `AppState`, `DiscoveryPacket`, et `ChatMessage`.
- Ajouter des tests d’intégration TCP/UDP pour vérifier la découverte et la transmission.
- Enrichir la gestion d’erreurs réseau avec des messages d’alerte pour l’utilisateur.

### Comportement opérationnel
- Si `tokio::net::UdpSocket::bind` échoue, `discovery::run` termine silencieusement.
- Si `TcpListener::bind` échoue, le serveur ne redémarre pas automatiquement.
- `handle_incoming` ne traite que les flux correctement terminés et ignore les paquets JSON malformés.

### Recommandations de tests
- `cargo test` devrait être ajouté avec des cas pour :
  - découverte de pair valide,
  - envoi TCP vers un pair simulé,
  - stockage et purge de l’historique de messages.
- Environnement de test : réseau local simulé ou tests basés sur sockets sur `127.0.0.1`.

## 📚 Voir aussi
- [Architecture et structure](01-architecture-et-structure.md)
- [Mécanismes et données](02-mecanismes-et-donnees.md)
- [Developer Experience](../../docs/02-developer-experience.md)
