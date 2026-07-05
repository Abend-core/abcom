> [🏠 Accueil](../../README.md) > [📦 Composant Abcom](README.md) > Fiabilité et tests

> 📅 **Généré le** : 2026-04-28
> 🔖 **Stack analysée** : Rust 2021, tokio 1, serde 1, serde_json 1, eframe 0.31, egui 0.31, chrono 0.4, anyhow 1
> 🔄 **À régénérer si** : couverture de tests ajoutée, robustesse réseau renforcée, persistance modifiée

# Fiabilité et tests

## 🌱 Robustesse existante
La fiabilité d’Abcom repose sur des mécanismes simples : gestion d’erreurs locale, `try_recv` non bloquant dans l’UI, et persistance JSON qui tolère l’absence du fichier initial.

### Traitement des erreurs
- `discovery.rs` gère les échecs de `bind` et d’activation du broadcast.
- `network.rs` ignore les échecs d’acceptation de connexion et logue les erreurs de connexion sortante.
- `app.rs` tente de créer les dossiers de stockage sans planter l’application.

## 🔧 Scénarios de tolérance
- perte d’un pair : l’adresse est mise à jour à chaque découverte.
- duplication de nom : le pair est ignoré si le nom correspond au poste local.
- message mal formé : la conversion JSON échoue silencieusement et ne bloque pas le serveur.

## ⚙️ Tests et couverture
### Situation actuelle
Aucun test automatisé n’a été détecté dans le dépôt.

### Priorités de test recommandées
- tests unitaires de `ChatMessage` et `DiscoveryPacket` pour la sérialisation JSON,
- tests d’intégration pour la découverte UDP et la transmission TCP,
- tests de persistance sur `AppState::save_messages` et `load_messages`,
- tests de filtrage de conversation avec `get_conversation_messages`.

### Actions immédiates
- ajouter un module `tests/` ou des tests intégrés `#[cfg(test)]` dans les modules existants,
- exécuter `cargo test` sur chaque merge request,
- documenter le périmètre de non-régression.

> [À COMPLÉTER PAR L'ÉQUIPE] : inventaire des cas d’erreur de l’UI, priorisation des scénarios de charge.
