> [🏠 Accueil](../../README.md) > [📘 ADR](ADR-001-langage-et-stack-rust.md)

# ADR-001 — Choix du langage Rust et de la stack

## Statut
Accepté (rétro-actif)

## 🌱 Contexte
Abcom est une application cliente de messagerie LAN avec des besoins de performance, d’interface native et de concurrence réseau. Le projet doit rester léger et exécutable localement sans serveur central.

## 🔧 Décision retenue
Le projet utilise Rust comme langage principal, avec la stack suivante :
- `tokio` pour le runtime asynchrone,
- `serde` / `serde_json` pour la sérialisation JSON,
- `eframe` et `egui` pour l’interface native,
- `chrono` pour les horodatages,
- `anyhow` pour la gestion d’erreurs.

## ⚙️ Conséquences techniques
- Positives : performance, sécurité mémoire, binaire natif léger.
- Négatives : courbe d’adoption pour des contributeurs non-Rust, compilation plus longue.
- Neutres : dépendance à l’écosystème Rust et aux versions de `Cargo`.

## Alternatives écartées
- Electron / web : trop lourds pour une application LAN simple.
- Python / Node.js : moins adaptés aux binaires statiques et à la concurrence native.
- Qt / GTK : complexité excessive pour un outil de chat léger.
