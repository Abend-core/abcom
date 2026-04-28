> [🏠 Accueil](../../README.md) > [📘 ADR](ADR-002-architecture-lan-peer-to-peer.md)

# ADR-002 — Architecture peer-to-peer sur LAN

## Statut
Accepté (rétro-actif)

## 🌱 Contexte
Abcom doit fonctionner sans serveur central sur un réseau local. La communication doit rester simple et permettre la découverte automatique des machines.

## 🔧 Décision retenue
Le projet adopte un modèle peer-to-peer léger :
- découverte de pairs par UDP broadcast sur `9001/udp`,
- échange de messages directs en TCP sur `9000/tcp`.

## ⚙️ Conséquences techniques
- Positives : simplicité d’implémentation, fonctionnement direct sur le LAN.
- Négatives : dépendance à un réseau non segmenté et absence de sécurité native.
- Neutres : extensibilité possible vers une couche d’authentification ultérieure.

## Alternatives écartées
- serveur central de découverte : inutile pour un LAN simple,
- multicast IP : moins robuste que l’UDP broadcast sur certaines configurations,
- WebSocket / HTTP : complexité inutile pour un client LAN natif.
