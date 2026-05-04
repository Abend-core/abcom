# Spécification: Gestion de l'Historique et Synchronisation des Groupes

## Contexte

Actuellement, l'historique est stocké dans un seul fichier `messages.json` et n'existe que localement. Il n'y a pas de synchronisation entre les pairs quand l'un d'eux se reconnecte.

## Problème

Lorsqu'un utilisateur se déconnecte d'une conversation de groupe:
- Les autres membres continuent à communiquer
- Cet utilisateur n'a pas accès à ces messages
- À la reconnexion, aucun mécanisme ne récupère l'historique manquant

## Solution: Gossip Protocol avec Synchronisation Multi-Membre

### Principes

1. **Historique par groupe**: Chaque groupe a son propre fichier d'historique
   - Format: `messages_groupe_<group_name>.json`
   - Stocké dans `~/.local/share/abcom/`

2. **Chaque membre conserve l'historique complet du groupe**
   - Tous les messages du groupe sont stockés localement
   - Redondance naturelle entre les membres

3. **Synchronisation multi-membre au reconnect**
   - Quand un utilisateur se reconnecte, il synchro avec **TOUS les membres en ligne**
   - Pas juste le premier trouvé
   - Assure une couverture maximale de l'historique

### Architecture

#### Stockage

```
~/.local/share/abcom/
├── messages.json              (conversations privées, historique actuel)
├── messages_groupe_Team.json
├── messages_groupe_DevOps.json
└── messages_groupe_RH.json
```

#### Structure d'un message de groupe

```json
{
  "id": "timestamp_checksum",
  "from": "alice",
  "content": "Salut l'équipe",
  "timestamp": "2026-05-04 14:30:45",
  "group_name": "Team",
  "checksum": "abc123def456"
}
```

L'**identifiant unique** = `timestamp + checksum` pour déduplication

### Flux de Synchronisation

#### 1. Au reconnect d'un utilisateur

```
Utilisateur C reconnecte
   ↓
Pour chaque groupe où C est membre:
   Chercher tous les membres du groupe EN LIGNE
   ↓
   Pour chaque membre en ligne:
     Envoyer: "SyncRequest" (group_name, last_checksum_connu)
     Recevoir: "SyncResponse" (messages manquants)
   ↓
   Fusionner les messages reçus (déduplication par checksum)
   Sauvegarder dans messages_groupe_X.json
```

#### 2. Types de messages pour la sync

**SyncRequest**:
```json
{
  "type": "SyncRequest",
  "group_name": "Team",
  "last_checksum": "abc123def456"
}
```

**SyncResponse**:
```json
{
  "type": "SyncResponse",
  "group_name": "Team",
  "messages": [
    { "id": "...", "from": "...", "content": "...", "timestamp": "...", "checksum": "..." },
    ...
  ]
}
```

### Implémentation - Étapes

1. **Phase 1**: Restructurer le stockage
   - Créer `messages_groupe_*.json` pour chaque groupe existant
   - Migrer les anciens messages

2. **Phase 2**: Implémenter les types de sync
   - Ajouter `SyncRequest` et `SyncResponse` au protocole réseau
   - Implémenter la sérialisation/désérialisation

3. **Phase 3**: Trigger de synchronisation
   - Détecter la reconnexion d'un utilisateur
   - Lancer la sync multi-membre pour tous les groupes

4. **Phase 4**: Fusion d'historique
   - Déduplication par checksum
   - Fusionner les messages reçus

### Considérations

- **Déduplication**: Utiliser `timestamp + checksum` pour éviter les doublons
- **Temps de conservation**: TBD (pour le moment, garder tout)
- **Contexte entreprise**: Les absences > 1 semaine sont rares, le gossip protocol suffit
- **Pas d'archive persistante**: Pas de serveur central ni NAS requis

### Notes

- Le protocole assume au moins 1 membre du groupe en ligne pendant les heures de travail
- En cas d'absence prolongée de tous les membres, des gaps peuvent subsister (acceptable)
- Les syncs successives remplissent progressivement les trous
