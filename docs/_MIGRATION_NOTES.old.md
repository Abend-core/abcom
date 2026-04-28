# Notes de migration

> 📅 **Généré le** : 2026-04-27  
> 🔖 **Stack analysée** : Rust 2021, tokio 1, serde 1, serde_json 1, eframe 0.31, egui 0.31, chrono 0.4, anyhow 1  
> 🔄 **À régénérer si** : refonte archi, changement majeur de stack, ajout/suppression de composant

| Ancien fichier | Contenu extrait | Réinjecté dans | Statut |
|----------------|-----------------|----------------|--------|
| `README.md` | Description du projet, installation, commandes Makefile, ports TCP/UDP, stack technique | `/README.md`, `/docs/02-developer-experience.md`, `/docs/03-cicd-et-deploiement.md` | ✅ |
| `regle-doc.md` | Règles de documentation et heuristiques de génération | `/docs/_MIGRATION_NOTES.md` | ✅ |

## Détails

- Les fichiers originaux ont été préservés sous `README.md.old` et `regle-doc.md.old`.
- Aucune information n’a été supprimée ; tout le contenu utile a été réinjecté ou référencé.
