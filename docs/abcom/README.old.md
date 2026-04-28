> [🏠 Accueil](../../README.md) > [📦 Composant Abcom](README.md)

> 📅 **Généré le** : 2026-04-27  
> 🔖 **Stack analysée** : Rust 2021, tokio 1, serde 1, serde_json 1, eframe 0.31, egui 0.31, chrono 0.4, anyhow 1  
> 🔄 **À régénérer si** : refonte archi, changement majeur de stack, ajout/suppression de composant

# Composant Abcom

## 🌱 Pour comprendre
Le composant `abcom` centralise tout le code de l’application : discovery réseau, transmission de messages, interface GUI et état local. Il n’existe pas de sous-service additionnel dans le dépôt, ce composant est le cœur fonctionnel du projet.

### Fonctions supportées
- découverte automatique de pairs LAN,
- liste des pairs disponibles,
- envoi de messages directs ou broadcast,
- affichage graphique des messages.

## 🔧 Pour utiliser
### Lancement du composant
```bash
cargo run --release -- <username>
```

### Structuration des docs composant
- [Architecture et structure](01-architecture-et-structure.md)
- [Mécanismes et données](02-mecanismes-et-donnees.md)
- [Performances et optimisations](03-performances-et-optimisations.md)
- [Fiabilité et tests](04-fiabilite-et-tests.md)

## ⚙️ Pour maîtriser
### Principaux fichiers
- `src/main.rs` : orchestre runtime et tâches.
- `src/app.rs` : état global et localisation des peers.
- `src/discovery.rs` : UDP broadcast et réception.
- `src/network.rs` : serveur TCP et envoi.
- `src/ui.rs` : vue et interactions `egui`.
- `src/message.rs` : schéma JSON des messages.

### Comportement de l’UI
- le panneau gauche liste les pairs découverts,
- la zone centrale affiche l’historique des messages,
- le champ de saisie permet l’envoi à un pair sélectionné ou à tous.

## 📚 Voir aussi
- [Architecture globale](../../docs/01-architecture-globale.md)
- [Developer Experience](../../docs/02-developer-experience.md)
- [Sécurité globale](../../docs/04-securite-globale.md)
