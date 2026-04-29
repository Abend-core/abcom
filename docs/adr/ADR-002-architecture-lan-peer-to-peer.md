# ADR-002 — Architecture LAN peer-to-peer

**Statut** : Accepté (rétro-actif)

## 🌱 Contexte
Abcom est conçu pour fonctionner dans un réseau local sans service central. La question était de savoir s’il fallait une architecture client-serveur ou peer-to-peer.

## 🔧 Décision retenue
- Architecture : peer-to-peer sur LAN.
- Découverte : UDP broadcast sur le port `9001`.
- Transport des messages : TCP direct sur le port `9000`.
- Aucune communication avec Internet n’est nécessaire.

## ⚙️ Conséquences techniques
- Chaque instance se comporte comme récepteur TCP et émetteur UDP.
- Le système dépend du broadcast réseau ; certains segments ou VLAN peuvent ne pas être compatibles.
- Aucun service central n’est requis, ce qui simplifie le déploiement mais rend le routage inter-sous-réseaux impossible.

### Contraintes futures
- Ne pas ajouter de mécanisme qui force un point central sans documentation d’architecture.
- Toute nouvelle fonctionnalité de découverte doit rester compatible avec l’existant UDP broadcast.
- Si des gateways ou des sessions multi-réseaux sont nécessaires, elles doivent être introduites comme une évolution explicite et documentée dans une ADR séparée.
