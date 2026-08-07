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
cargo test --all-features --locked    # 289 tests
cargo test app::groups                # un module
cargo fmt && cargo clippy -- -D warnings   # ce que la CI exigera
```

Le profil release est optimisé pour un binaire compact (`lto = "thin"`, `codegen-units = 1`, `strip`, `panic = "abort"`) : la compilation release est sensiblement plus longue que le debug, c'est normal.

## Tests

289 tests automatisés : 288 tests unitaires regroupés dans [src/tests/](../src/tests/) et un test d'intégration externe dans [tests/p2p_e2e.rs](../tests/p2p_e2e.rs). Points notables :

- les tests réseau utilisent de **vraies sockets** (`TcpListener::bind("127.0.0.1:0")`, UDP réel) — pas de mocks ;
- le chiffrement est testé par des handshakes Noise complets en mémoire et entre endpoints réels (y compris le rejet d'un client en clair et le refus sur clé changée) ;
- la migration JSON → SQLite, les règles de groupes (succession, purge d'historique), les accusés et le composer ont chacun leur suite.

`scripts/integration_test.sh` exécute exclusivement le scénario P2P headless : handshake Noise, identité applicative et échange d'un message sur une vraie socket. La découverte UDP entre deux processus complets reste un test manuel.

## Intégration continue

Deux workflows GitHub Actions :

| Workflow | Déclencheur | Étapes |
|---|---|---|
| [ci-dev.yml](../.github/workflows/ci-dev.yml) | PR/push vers `dev` | format · Clippy · build · tests sur Linux, checks macOS/Windows |
| [ci-main.yml](../.github/workflows/ci-main.yml) | PR/push vers `main` | idem + scénario P2P headless + `cargo audit` |

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

Les dépendances directes sont déclarées dans [Cargo.toml](../Cargo.toml) et verrouillées par `Cargo.lock`. Les principales :

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

**Licence du projet à décider avant release** : `Cargo.toml` déclare MIT tandis que `LICENSE` et l'interface indiquent AGPL-3.0. Ces sources doivent être alignées dans un même changement après décision du propriétaire. Ressources embarquées : police Inter (OFL-1.1, licence dans `assets/fonts/`), jeu d'emojis type Twemoji.

Outils d'entretien : `cargo audit` (vulnérabilités — exécuté par la CI de `main`), `cargo license` (inventaire), `cargo update` (mises à jour semver). `Cargo.lock` est versionné : builds reproductibles.

## Fichiers de suivi

- [CHANGELOG.md](../CHANGELOG.md) — modifications notables, alimenté en continu, consolidé à chaque release.
- [AVANCEMENT.md](../AVANCEMENT.md) — tableau des features en cours/terminées, tenu sur `dev` uniquement.
- [old/](../old/) — documentation historique (audits, plans, ADR, anciennes versions), conservée en l'état.
