> [🏠 Accueil](../../README.md) > [📦 Composant Abcom](README.md) > [⚡ Performances et optimisations](03-performances-et-optimisations.md)

> 📅 **Généré le** : 2026-04-27  
> 🔖 **Stack analysée** : Rust 2021, tokio 1, serde 1, serde_json 1, eframe 0.31, egui 0.31, chrono 0.4, anyhow 1  
> 🔄 **À régénérer si** : refonte archi, changement majeur de stack, ajout/suppression de composant

# Performances et optimisations

## 🌱 Pour comprendre
Abcom est une application réseau et interface graphique. Les performances dépendent de la gestion du runtime Tokio, de la fréquence de rafraîchissement de l’UI et de la stratégie de buffering des messages.

## 🔧 Pour utiliser
### Comportement de rendu
- L’UI demande un repaint toutes les `100 ms`.
- Les événements réseau sont consommés avec `try_recv()` depuis `event_rx`.

### Tampon de messages
- Historique stocké dans `AppState.messages`.
- L’historique est tronqué lorsque la taille dépasse `500`, en supprimant les `100` premiers messages.

## ⚙️ Pour maîtriser
### Points de contention
- `Arc<Mutex<AppState>>` est un verrou global partagé entre UI et flux réseau.
- `try_recv()` sur `event_rx` est non bloquant, ce qui limite les interférences avec l’UI.
- `tokio::spawn` par envoi de message peut créer un grand nombre de tâches si de nombreux messages sont envoyés simultanément.

### Optimisations possibles
- Remplacer `Mutex` par `tokio::sync::RwLock` si les lectures majorent les écritures.
- Ajouter un framing TCP explicite pour éviter de dépendre de `read_to_end`.
- Limiter le nombre de tâches `tokio::spawn` en utilisant un pool ou un backpressure explicite.

### Mesure de performance
- Aucun outil de profilage n’est intégré dans le dépôt actuel.
- Recommandation : utiliser `cargo flamegraph` ou `tokio-console` pour analyser les goulots.

## 📚 Voir aussi
- [Architecture et structure](01-architecture-et-structure.md)
- [Mécanismes et données](02-mecanismes-et-donnees.md)
- [Fiabilité et tests](04-fiabilite-et-tests.md)
