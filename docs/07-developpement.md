# 07 — Développement

## Mise en place

```bash
git clone <repo> && cd abcom
git config core.hooksPath .githooks   # hook pre-commit : cargo fmt + clippy bloquants
cp .env.example .env 2>/dev/null || true  # ou créer .env (clé Klipy, passphrase…)
cargo build
```

## Commandes courantes

```bash
cargo run --release -- <pseudo>       # lancer
cargo test                            # 264 tests
cargo test app::groups                # un module
cargo fmt && cargo clippy -- -D warnings   # ce que la CI exigera
```

Le profil release est optimisé pour un binaire compact (`lto = "thin"`, `codegen-units = 1`, `strip`, `panic = "abort"`) : la compilation release est sensiblement plus longue que le debug, c'est normal.

## Tests

264 tests automatisés, regroupés dans [src/tests/](../src/tests/) (un fichier par module testé). Points notables :

- les tests réseau utilisent de **vraies sockets** (`TcpListener::bind("127.0.0.1:0")`, UDP réel) — pas de mocks ;
- le chiffrement est testé par des handshakes Noise complets en mémoire et entre endpoints réels (y compris le rejet d'un client en clair et le refus sur clé changée) ;
- la migration JSON → SQLite, les règles de groupes (succession, purge d'historique), les accusés et le composer ont chacun leur suite.

`scripts/integration_test.sh` est exécuté par la CI de `main` mais reste sommaire ; un vrai test d'intégration « deux instances se découvrent et échangent » est dans le backlog.

## Intégration continue

Deux workflows GitHub Actions :

| Workflow | Déclencheur | Étapes |
|---|---|---|
| [ci-dev.yml](../.github/workflows/ci-dev.yml) | PR vers `dev` | `cargo fmt --check` · `clippy -D warnings` · `build --release` · `test` (~6 min 30) |
| [ci-main.yml](../.github/workflows/ci-main.yml) | PR vers `main` | idem + `integration_test.sh` + `cargo audit` |

Le hook local `.githooks/pre-commit` bloque tout commit mal formaté avant même la CI.

## Workflow Git

### Branches

```
main          ← production stable (PR uniquement depuis dev)
 └── dev      ← intégration (PR uniquement depuis feature/)
      └── feature/<nom>   ← une fonctionnalité complète
           └── task/<nom> ← une sous-tâche de la feature
```

Push direct interdit sur `main` et `dev` : tout passe par une PR (protection GitHub active ; `main` exige une approbation). Nommage en kebab-case : `feature/transfert-fichiers`, `task/fix-cursor-click`, `fix/cpu-overload-loop`.

### Commits

Format : `type(scope): description courte en français` — impératif, pas de majuscule après le `:`, pas de point final.

**Un commit = une intention.** Ne jamais mélanger un `feat` et un `fix` ; deux choses = deux commits. Pas de commits `WIP` sur `dev` ou `main` (squash avant merge).

Types : `feat`, `fix`, `refactor`, `docs`, `test`, `chore`. Scopes usuels : `ui`, `app`, `network`, `transfer`, `input`, `markdown`, `i18n`, `receipts`, `discovery`, `config`, `runner`, `groupes`.

```
feat(transfer): demander l'acceptation du destinataire avant réception
fix(ui): corriger la boucle infinie has_unread en arrière-plan
docs(git): ajouter les règles de workflow pour l'équipe
```

### Règles pour les agents IA

- Toujours partir de `dev` à jour ; créer une branche `feature/` ou `task/` selon la portée.
- `cargo test` avant tout commit ; jamais de `--no-verify` ni de `--force` sans accord explicite.
- **Jamais de `git push` sans accord explicite de l'utilisateur.**
- **Pas de trailer `Co-Authored-By` dans les messages de commit.**
- PR via `gh pr create` puis `gh pr merge` — ne pas renvoyer l'utilisateur vers GitHub.
- Après le merge d'une feature : mettre à jour `AVANCEMENT.md` **sur `dev` uniquement** (ce fichier ne doit jamais être modifié depuis une branche feature, c'est ce qui le garde sans conflits).

### Releases

Chaque merge `dev` → `main` correspond à une version SemVer, taguée `v0.x.x`, avec `CHANGELOG.md` mis à jour au préalable (format Keep a Changelog).

## Dépendances

Une quinzaine de dépendances directes ([Cargo.toml](../Cargo.toml)), ~550 paquets dans le graphe résolu. Les principales :

| Crate | Rôle |
|---|---|
| `tokio` (features explicites, pas `full`) | Runtime asynchrone du réseau |
| `eframe` / `egui` / `egui_extras` 0.31 | Fenêtre native, interface, chargement d'images animées |
| `serde` / `serde_json` | Sérialisation des paquets réseau et des données |
| `rusqlite` (`bundled`) | Stockage SQLite embarqué |
| `snow` + `blake2` | Handshake Noise, empreintes et dérivation de PSK |
| `socket2` | Options bas niveau des sockets (broadcast, réuse de port) |
| `image`, `resvg` (optionnel), `rfd`, `walkdir`, `zip` | Décodage d'images, import SVG (feature `avatar-svg`), sélecteurs de fichiers natifs, parcours et archivage de dossiers |
| `tray-icon`, `notify-rust`, `auto-launch` | Icône résidente, notifications système, autostart |
| `rodio` | Sons de notification |
| `ehttp`, `chrono`, `anyhow`, `dirs` | Requêtes Klipy, horodatage, erreurs, chemins par plateforme |

**Licences** : le projet est MIT. Le graphe est à ~96 % MIT/Apache-2.0 et assimilés, plus quelques paquets MPL-2.0 (famille resvg, symphonia). La MPL-2.0 est un copyleft au niveau du fichier : comme ces crates sont consommés sans modification, aucune obligation n'en découle. Aucune dépendance GPL/AGPL. Ressources embarquées : police Inter (OFL-1.1, licence dans `assets/fonts/`), jeu d'emojis type Twemoji.

Outils d'entretien : `cargo audit` (vulnérabilités — exécuté par la CI de `main`), `cargo license` (inventaire), `cargo update` (mises à jour semver). `Cargo.lock` est versionné : builds reproductibles.

## Fichiers de suivi

- [CHANGELOG.md](../CHANGELOG.md) — modifications notables, alimenté en continu, consolidé à chaque release.
- [AVANCEMENT.md](../AVANCEMENT.md) — tableau des features en cours/terminées, tenu sur `dev` uniquement.
- [old/](../old/) — documentation historique (audits, plans, ADR, anciennes versions), conservée en l'état.
