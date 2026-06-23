# Règles Git — Abcom

## Structure des branches

```
main          ← production stable (PR uniquement depuis dev)
 └── dev      ← intégration (PR uniquement depuis feature/)
      └── feature/<nom>   ← une fonctionnalité complète
           └── task/<nom> ← une sous-tâche de la feature
```

| Branche | Rôle | Merge vers |
|---|---|---|
| `main` | Production stable | — |
| `dev` | Intégration continue | `main` via PR |
| `feature/<nom>` | Fonctionnalité complète | `dev` via PR |
| `task/<nom>` | Sous-tâche isolée | `feature/<nom>` direct |

> **Règle absolue** : on ne push jamais directement sur `main` ni sur `dev`.  
> Tout passe par une PR.

---

## Nommage des branches

```
feature/nom-court-en-kebab-case
task/nom-court-en-kebab-case
fix/nom-du-bug
```

Exemples tirés du projet :
```
feature/transfert-fichiers
feature/markdown-renderer
task/add-acceptance-modal
task/fix-cursor-click
fix/cpu-overload-loop
```

---

## Convention de commits (atomiques)

### Format

```
type(scope): description courte en français
```

### Règle atomique

**Un commit = une intention.** Ne jamais mélanger un `feat` et un `fix` dans le même commit. Si tu fais deux choses, tu fais deux commits.

Mauvais :
```
feat(ui): ajouter le panneau + fix crash au démarrage
```
Bon :
```
feat(ui): ajouter le panneau de paramètres
fix(app): corriger le crash au démarrage si config absente
```

### Types autorisés

| Type | Usage |
|---|---|
| `feat` | Nouvelle fonctionnalité |
| `fix` | Correction de bug |
| `refactor` | Restructuration sans changement de comportement |
| `docs` | Documentation uniquement |
| `test` | Ajout ou modification de tests |
| `chore` | Maintenance (dépendances, CI, config) |

### Scopes autorisés (issus du projet)

`ui`, `app`, `network`, `transfer`, `input`, `markdown`, `i18n`, `receipts`, `discovery`, `config`

### Règles

- Description en **français**, impératif court (`ajouter`, `corriger`, `supprimer`)
- Pas de majuscule après le `:`
- Pas de point final
- Pas de `Co-Authored-By` automatique
- Pas de commits `WIP` ou `fix typo` sur `dev` ou `main` (squash avant merge)

Exemples valides :
```
feat(transfer): demander l'acceptation du destinataire avant réception
fix(ui): corriger la boucle infinie has_unread en arrière-plan
refactor(app): découper app.rs en sous-modules indépendants
docs(git): ajouter les règles de workflow pour l'équipe
test(app): étendre la couverture à 111 tests unitaires
```

---

## Workflow type

```bash
# 1. Toujours partir de dev à jour
git checkout dev && git pull

# 2. Créer la branche feature
git checkout -b feature/ma-feature

# 3. Pour chaque sous-tâche : créer une branche task
git checkout -b task/ma-tache
# ... travail, commits atomiques ...
git checkout feature/ma-feature
git merge task/ma-tache
git branch -d task/ma-tache

# 4. Mettre à jour AVANCEMENT.md sur dev avant de merger
git checkout dev
# éditer AVANCEMENT.md pour marquer la feature comme terminée
git checkout feature/ma-feature

# 5. PR feature → dev sur GitHub
git push origin feature/ma-feature
# Ouvrir la PR sur GitHub

# 6. Après merge de la PR, nettoyer
git branch -d feature/ma-feature
```

---

## Règles pour les agents IA

- Toujours partir de `dev`, jamais de `main` directement
- Créer une branche `feature/` ou `task/` selon la portée
- Ne jamais push sur `main` ou `dev` directement
- Vérifier `cargo check` avant tout commit
- Ne jamais utiliser `--no-verify` ou `--force` sans accord explicite
- Pas de `Co-Authored-By` dans les messages de commit
- Mettre à jour `AVANCEMENT.md` sur `dev` à chaque début et fin de feature

---

## Releases et CHANGELOG

- Chaque merge de `dev` → `main` correspond à une version
- La version suit [semver](https://semver.org) : `MAJOR.MINOR.PATCH`
- Mettre à jour `CHANGELOG.md` avant tout merge vers `main`
- Tagger `main` après chaque release : `git tag v0.x.x`

---

## Protection des branches (GitHub)

| Branche | Push direct | PR requise | Approbation |
|---|---|---|---|
| `main` | Interdit | Oui | 1 reviewer |
| `dev` | Interdit | Oui | — |
| `feature/*` | Autorisé | Non | — |
| `task/*` | Autorisé | Non | — |
