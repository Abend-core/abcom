# ADR-001 — Choix du langage Rust et de la stack

## Statut
Accepté (rétro-actif)

## Contexte
Le projet Abcom est une application cliente de messagerie locale avec des exigences de performance, de concurrence et de stabilité. Il doit fonctionner sur des postes utilisateurs avec un affichage natif.

## Décision
Le projet utilise Rust comme langage principal, avec les dépendances suivantes :
- `tokio` pour le runtime asynchrone,
- `serde` et `serde_json` pour la sérialisation,
- `eframe` / `egui` pour l’interface graphique native,
- `chrono` pour les horodatages,
- `anyhow` pour la gestion d’erreurs.

## Alternatives écartées
- Electron ou frameworks web : trop lourds pour une application LAN native.
- Python ou Node.js : moins adaptés pour la compilation statique et les exigences de performance.
- GUI multiplateforme lourde (Qt, GTK) : dépassement de complexité pour un outil simple.

## Conséquences
- Positives : performance, sécurité mémoire, application native légère.
- Négatives : courbe d’adoption pour des contributeurs Rust, compilation plus longue.
- Neutres : besoin de maîtriser le runtime asynchrone et l’écosystème Rust.
