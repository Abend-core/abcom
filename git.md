# Règles Git — Abcom

## Structure des branches

```
main
 └── dev
      └── feature/<nom>
           └── task/<nom>
```

| Branche | Rôle |
|---|---|
| `main` | Production stable — merge uniquement depuis `dev` via PR validée |
| `dev` | Intégration — reçoit les features terminées |
| `feature/<nom>` | Une fonctionnalité complète — part de `dev`, merge dans `dev` |
| `task/<nom>` | Une sous-tâche — part de `feature/<nom>`, merge dans `feature/<nom>` |

## Nommage des branches

```
feature/nom-court-en-kebab-case
task/nom-court-en-kebab-case
fix/nom-du-bug
```

## Commits

- Format : `type(scope): description courte en français`
- Types autorisés : `feat`, `fix`, `refactor`, `docs`, `test`, `chore`
- Pas de co-auteur automatique (pas de `Co-Authored-By`)
- Un commit = une intention claire, pas de commits "WIP" sur `dev` ou `main`

```
feat(ui): ajouter le panneau de paramètres
fix(network): corriger la reconnexion après timeout
refactor(app): découper app.rs en sous-modules
```

## Workflow type

```bash
# Partir de dev à jour
git checkout dev && git pull

# Créer la branche feature
git checkout -b feature/ma-feature

# Créer une branche task pour chaque sous-tâche
git checkout -b task/ma-tache

# ... travail, commits ...

# Merger la task dans la feature
git checkout feature/ma-feature
git merge task/ma-tache

# Merger la feature dans dev
git checkout dev
git merge feature/ma-feature
git push origin dev

# Supprimer les branches terminées
git branch -d task/ma-tache feature/ma-feature
```

## Règles pour les agents IA

- Toujours partir de `dev`, jamais de `main` directement
- Créer une branche `feature/` ou `task/` selon la portée du changement
- Ne jamais push sur `main` directement
- Pas de `Co-Authored-By` dans les messages de commit
- Vérifier `cargo check` avant tout commit sur une branche Rust
- Ne jamais utiliser `--no-verify` ou `--force` sans accord explicite
- Les merges vers `main` se font uniquement via PR revue par un humain

## Merge vers main

Uniquement via Pull Request sur GitHub, après :
1. Merge de toutes les tasks dans la feature
2. Merge de la feature dans `dev`
3. Vérification que `dev` compile et passe les tests
4. PR `dev` → `main` approuvée par un membre de l'équipe
