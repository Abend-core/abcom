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

## Comment mettre à jour ce fichier

**En début de feature** : ajouter une ligne dans "En cours" avec le statut `🚧 En cours`.

**En fin de feature** (avant PR → dev) : déplacer la ligne dans "Terminées" avec la date du merge.

**Les agents IA** doivent obligatoirement mettre à jour ce fichier sur `dev` avant de merger une feature.
