# Avancement des features — branche dev

Ce fichier est mis à jour à chaque début et fin de feature.  
Il donne une vue d'ensemble de l'état du développement en cours.

---

## En cours

| Feature | Branche | Description | Responsable | Statut |
|---|---|---|---|---|
| — | — | — | — | — |

---

## Terminées (mergées dans dev)

| Feature | Branche | Description | Mergé le |
|---|---|---|---|
| Règles Git & docs projet | `feature/project-docs` | git.md, CHANGELOG.md, AVANCEMENT.md | 2026-06-23 |
| Correctif surcharge CPU | `fix/cpu-overload-loop` | Boucle infinie has_unread + son bloquant UI | 2026-06-23 |
| Transfert de fichiers | `feature/transfert` | Envoi/réception avec acceptation, progress bar | — |
| Accusés de réception | `feature/receipts` | ACK réseau + indicateurs ✓ et ✓✓ | — |
| Picker emoji + shortcodes | `feature/emoji` | Recherche par :shortcode:, picker par catégorie | — |
| Rendu Markdown | `feature/markdown` | Gras, italique, code inline/bloc, liens | — |
| Architecture modulaire | `refactor/atomic-arch` | Découpage app.rs / ui.rs / message.rs | — |
| Multilingue FR/EN | `feature/i18n` | Bascule langue dans les paramètres | — |

---

## Planifiées

| Feature | Description | Priorité |
|---|---|---|
| — | — | — |

---

## Comment mettre à jour ce fichier — règle anti-conflit

> **Ce fichier n'existe que sur `dev`. Il ne doit JAMAIS être modifié depuis une branche `feature/` ou `task/`.**  
> Les mises à jour se font uniquement en deux moments précis, directement sur `dev` :

**1. En début de feature** — juste après avoir créé la branche feature depuis dev :
```bash
git checkout dev
# éditer AVANCEMENT.md : ajouter la feature en "En cours"
git add AVANCEMENT.md && git commit -m "chore(avancement): démarrer feature/ma-feature"
git checkout feature/ma-feature
```

**2. En fin de feature** — juste après le merge de la PR feature → dev :
```bash
# la PR vient d'être mergée sur dev
git checkout dev && git pull
# éditer AVANCEMENT.md : déplacer la feature en "Terminées"
git add AVANCEMENT.md && git commit -m "chore(avancement): clore feature/ma-feature"
git push origin dev
```

Pourquoi ça n'entraîne pas de conflits : les features ne touchent jamais ce fichier,  
donc il ne peut pas y avoir de divergence. Les seules modifications viennent de `dev` lui-même,  
de façon séquentielle.
