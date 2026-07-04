> [🏠 Accueil](../README.md) > [📖 Glossaire](05-glossaire.md)

> 📅 **Généré le** : 2026-04-28
> 🔖 **Stack analysée** : Rust 2021, tokio 1, serde 1, serde_json 1, eframe 0.31, egui 0.31, chrono 0.4, anyhow 1
> 🔄 **À régénérer si** : évolution du vocabulaire technique, ajout d’un nouveau composant

# Glossaire

## LAN
Local Area Network. Réseau local où toutes les machines peuvent communiquer sans traverser Internet. Abcom est conçu pour fonctionner exclusivement sur un LAN de confiance.

## UDP broadcast
Méthode d’envoi de paquets UDP à l’adresse de diffusion du réseau local (`255.255.255.255`). Utilisée ici pour annoncer la présence d’un pair sur le port `9001`.

## TCP
Transmission Control Protocol. Protocole orienté connexion et fiable. Abcom l’utilise pour transmettre les messages JSON d’un pair à un autre sur le port `9000`.

## Tokio
Runtime asynchrone Rust. Il gère les tâches concurrentes de découverte, d’écoute TCP et d’envoi TCP dans Abcom.

## eframe / egui
Bibliothèques Rust pour interface graphique native. `eframe` lance la fenêtre native et `egui` gère les widgets, les panneaux et les contrôles.

## systemd user
Mode de service systemd exécuté au niveau utilisateur, sans privilèges root. Abcom peut être démarré et activé via `systemctl --user`.

## JSON
JavaScript Object Notation. Format texte utilisé pour sérialiser les paquets de découverte et les messages échangés.

## Peer
Pair. Une autre instance Abcom détectée sur le LAN. Chaque pair est identifiée par un nom d’utilisateur et une adresse TCP calculée à partir de son adresse IP.
