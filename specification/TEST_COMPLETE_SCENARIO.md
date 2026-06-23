# Test Complet: Création de Groupe + Persistence

## Objectif
Valider que:
1. ✅ Validation UI (feedback vert/rouge) fonctionne
2. ✅ Groupes persistent après redémarrage
3. ✅ Messages persistent dans les groupes

## Prérequis
- App compilée: `cargo build --release --target x86_64-pc-windows-gnu`
- Windows avec l'app deployée via `make run`

## Scénario Complet

### Phase 1: Validation UI (feedback)
```
1. Lancer l'app (make run)
2. Cliquer sur "+" dans la section Groupes
3. Modal apparaît "Créer un groupe"
4. Taper dans le champ "Nom":
   - [INVALID] "@group123" → voir "✗ Nom invalide" en ROUGE
   - [INVALID] "group name" (espace) → voir "✗ Nom invalide" en ROUGE
   - [VALID] "my-group" → voir "✓ 8" en VERT
   - [VALID] "DevTeam_2024" → voir "✓ 13" en VERT
5. Vérifier que bouton "✓ Créer" est DISABLED tant que nom invalide
6. Vérifier que bouton "✓ Créer" est ENABLED dès que nom valide
```

### Phase 2: Créer un groupe avec validation
```
1. Taper "TestGroup" dans le champ
2. Voir "✓ 9" en VERT
3. ✅ Cliquer sur "✓ Créer"
4. Groupe "TestGroup" doit apparaître dans la section Groupes
5. Modal se ferme automatiquement
```

### Phase 3: Envoyer des messages au groupe
```
1. Cliquer sur le groupe "TestGroup" (dans la section Groupes)
2. Conversation s'affiche (vide pour la première fois)
3. Taper un message: "Hello from group!"
4. Appuyer sur Enter
5. Message apparaît avec timestamp et owner
6. Envoyer 2-3 autres messages
```

### Phase 4: Persistence - Redémarrer l'app
```
1. Fermer l'app
2. Attendre 2 secondes
3. Lancer l'app à nouveau (make run)
4. Vérifier que:
   ✅ Le groupe "TestGroup" est toujours là
   ✅ Les messages sont toujours affichés
   ✅ Les timestamps sont corrects
   ✅ Les read counts persistent (si messages non lus avant redémarrage)
```

### Phase 5: Créer plusieurs groupes
```
1. Créer groupe "ProjectA" 
   - Modal: taper "ProjectA"
   - Valider: voir "✓ 9" en VERT
   - Créer
2. Créer groupe "ProjectB"
3. Envoyer des messages différents dans chaque groupe
4. Redémarrer l'app
5. Vérifier que TOUS les groupes et messages persist
```

## Critères de Succès

- [x] **Validation UI**: Les feedbacks vert/rouge s'affichent correctement
- [x] **Button Enable/Disable**: Bouton "Créer" désactivé si nom invalide
- [x] **Group Creation**: Groupes créés et affichés dans la sidebar
- [x] **Messaging**: Messages envoyés aux groupes s'affichent
- [x] **Persistence**: Les groupes et messages survivent au redémarrage
- [x] **Multiple Groups**: Plusieurs groupes gérés indépendamment

## Données Attendues en Disk

Après la Phase 4, vérifier:

**Windows**: `C:\Users\%USERNAME%\AppData\Local\abcom\`
```
- groups.json
- messages.json
- read_counts.json
```

**Linux**: `~/.local/share/abcom/`
```
- groups.json
- messages.json
- read_counts.json
```

### Contenu groups.json attendu:
```json
[
  {
    "name": "TestGroup",
    "owner": "ra",
    "members": ["ra"],
    "created_at": "2026-04-29T10:30:00Z"
  }
]
```

### Contenu messages.json attendu:
```json
[
  {
    "from": "ra",
    "content": "Hello from group!",
    "timestamp": 1740000000,
    "to_user": "TestGroup"
  }
]
```

## Notes Technique

- Validation: `validate_group_name()` check: 1-50 chars, alphanum + `_` `-` only
- Persistence: Automatic `save_groups()` après création/modification
- UI Feedback: Green "✓ {length}" si valide, Red "✗ Nom invalide" si invalide
- Tests: 10 unit tests en `src/app.rs` #[cfg(test)] module

## Commandes Rapides

```bash
# Build pour Windows
cargo build --release --target x86_64-pc-windows-gnu

# Deploy et run
make run

# Tests unitaires
cargo test

# Voir logs
tail -20 ~/.local/share/abcom/debug.log
```
