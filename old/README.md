# Archives de documentation

**Ce dossier ne contient que de la documentation** — aucun code, rien qui soit
compilé ou exécuté. C'est la mémoire écrite du projet : les documents qui ont
servi à le construire, conservés tels qu'ils étaient au moment où ils ont servi.

**Rien ici n'est tenu à jour.** Certains contenus décrivent des états dépassés
(persistance JSON, protocole en clair, absence de tests) et leurs liens vers le
code pointent vers une arborescence qui a changé depuis. La documentation
vivante est dans [`docs/`](../docs/).

On les garde pour une seule raison : retrouver **pourquoi** une décision a été
prise, quand la question ressurgit des mois plus tard. Les fichiers sont
préfixés de leur date, donc classés par ordre chronologique.

## Avril 2026 — documentation d'origine

Écrite avant la refonte du 5 juillet 2026. Décrit une version antérieure du
projet : monolithe, persistance JSON, sans chiffrement.

| Fichier | Contenu | Remplacé par |
|---|---|---|
| `2026-04-readme-racine.md` | Premier README du dépôt | [README.md](../README.md) |
| `2026-04-architecture-globale.md` | Architecture initiale | [docs/02](../docs/02-architecture.md) |
| `2026-04-developer-experience.md` | Build, Makefile, service systemd | [docs/07](../docs/07-developpement.md) |
| `2026-04-cicd-et-deploiement.md` | Première chaîne CI/CD | [docs/07](../docs/07-developpement.md) |
| `2026-04-securite-globale.md` | Sécurité avant le chiffrement Noise | [docs/03](../docs/03-reseau-et-securite.md) |
| `2026-04-glossaire.md` | Vocabulaire du projet | [docs/01](../docs/01-presentation.md) |
| `2026-04-installation-windows.md` | Guide d'installation Windows | [docs/06](../docs/06-installation.md) |
| `2026-04-doc-generee/` | Documentation générée (architecture, mécanismes, perfs, tests) | [docs/01](../docs/01-presentation.md) à [04](../docs/04-stockage.md) |
| `2026-04-adr/` | Deux décisions d'architecture : choix de Rust, choix du P2P LAN | [docs/01](../docs/01-presentation.md), section « Décisions fondatrices » |

## Juin 2026 — premier audit et solidification

| Fichier | Contenu | Remplacé par |
|---|---|---|
| `2026-06-audit-technique.md` | Premier audit formel et ses quatre sprints | [docs/08](../docs/08-historique-et-audits.md) |
| `2026-06-dependances-et-licences.md` | Inventaire des dépendances et de leurs licences | [docs/07](../docs/07-developpement.md) |
| `2026-06-workflow-git.md` | Règles Git et conventions de commit | [docs/07](../docs/07-developpement.md), [CONTRIBUTING.md](../CONTRIBUTING.md) |

## Juillet 2026 — performance, sécurisation, spécifications

| Fichier | Contenu | Remplacé par |
|---|---|---|
| `2026-07-audit-performance.md` | Audit performance détaillé (constats `fichier:ligne`, **protocole de mesure** encore utilisé pour refaire les relevés) | [docs/08](../docs/08-historique-et-audits.md) |
| `2026-07-plan-optimisation.md` | Plan d'exécution des phases A/B/C et résultats | [docs/08](../docs/08-historique-et-audits.md) |
| `2026-07-seconde-passe-et-securisation.md` | Seconde passe d'audit et plan de passage à Noise | [docs/03](../docs/03-reseau-et-securite.md), [docs/08](../docs/08-historique-et-audits.md) |
| `2026-07-spec-runner-resident.md` | Spécification du mode résident (tray) | [docs/05](../docs/05-fonctionnalites.md) |
| `2026-07-spec-groupes.md` | Spécification des groupes — décrit l'époque où **le nom d'un salon était son identifiant**, ce qui a depuis été corrigé | [docs/05](../docs/05-fonctionnalites.md) |

## Août 2026 — audit de dette et son exécution

| Fichier | Contenu | Remplacé par |
|---|---|---|
| `2026-08-audit-dette-de-code.md` | Checklist de dette de code, 69 des 71 points appliqués | [docs/08](../docs/08-historique-et-audits.md), [docs/09](../docs/09-limites-et-pistes.md) |
| `2026-08-plan-maintenabilite.md` | Plan séquencé (phases P1 à P6) de cet audit, déroulé intégralement | [docs/08](../docs/08-historique-et-audits.md) |
| `2026-08-audit-dependances.md` | Écart entre ce que les dépendances offrent et ce qu'on en tirait — 25 de ses 30 constats appliqués | [docs/08](../docs/08-historique-et-audits.md) ; les 5 restants dans [docs/09](../docs/09-limites-et-pistes.md) |
