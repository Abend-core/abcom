# ADR-001 — Langage et stack Rust

**Statut** : Accepté (rétro-actif)

## 🌱 Pourquoi ce choix de langue et de runtime
Le projet Abcom est un client de messagerie LAN qui doit être léger, natif et capable de gérer de la concurrence réseau de manière efficace. Rust offre une bonne sécurité mémoire, un runtime asynchrone mature avec `tokio`, et une cible desktop simple à empaqueter.

## 🔧 Décision retenue
- Langage : Rust édition 2021.
- Runtime async : `tokio` 1.
- UI native : `eframe` / `egui`.
- Sérialisation : `serde` + `serde_json`.

## ⚙️ Conséquences techniques
- L’application est compilée en binaire natif unique, sans dépendances d’exécution externes.
- Le code réseau bénéficie de `async`/`await` et de tâches Tokio séparées pour discovery, serveur TCP et envois.
- La GUI reste embarquée dans le même binaire que la logique réseau, ce qui simplifie le packaging mais impose une monolithe applicatif.

### Alternatives écartées
- Utiliser un framework web pour l’interface : cela aurait ajouté une dépendance navigateur/serveur local et complexifié l’architecture.
- Adopter un autre runtime async, comme `async-std` : `tokio` a été retenu pour sa maturité et son large écosystème.
