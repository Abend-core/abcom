> [🏠 Accueil](../../README.md) > [📘 ADR](ADR-002-architecture-lan-peer-to-peer.md)

# ADR-002 — Architecture peer-to-peer sur LAN

## Statut
Accepté (rétro-actif)

## Contexte
Abcom doit fonctionner sans serveur central sur un réseau local. La communication doit être simple et permettre la découverte d’autres machines sur le LAN.

## Décision
Le projet adopte un modèle peer-to-peer léger :
- découverte de pairs par UDP broadcast sur le port `9001`,
- échange de messages directs en TCP sur le port `9000`.

## Alternatives écartées
- Serveur central de découverte : inutile pour un LAN simple.
- Multicast IP : moins portable que l’UDP broadcast dans certains environnements.
- Utilisation de WebSocket ou HTTP : complexité supplémentaire.

## Conséquences
- Positives : simplicité d’implémentation et fonctionnement direct sur le LAN.
- Négatives : dépendance à un réseau non segmenté, absence de sécurité native.
- Neutres : le modèle est facile à étendre mais nécessite une future couche d’authentification si le projet sort du LAN de confiance.
