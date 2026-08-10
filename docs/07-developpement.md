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
cargo test --all-features --locked    # 329 tests
cargo test app::groups                # un module
cargo fmt && cargo clippy -- -D warnings   # ce que la CI exigera
```

Le profil release est optimisé pour un binaire compact (`lto = "thin"`, `codegen-units = 1`, `strip`, `panic = "abort"`) : la compilation release est sensiblement plus longue que le debug, c'est normal.

## Tests

329 tests automatisés : 328 tests unitaires regroupés dans [src/tests/](../src/tests/) et un test d'intégration externe dans [tests/p2p_e2e.rs](../tests/p2p_e2e.rs). Les tests manuels — réseau réel, interface, différences entre OS — sont dans le [cahier de tests](10-cahier-de-tests.md). Points notables :

- les tests réseau utilisent de **vraies sockets** (`TcpListener::bind("127.0.0.1:0")`, UDP réel) — pas de mocks ;
- le chiffrement est testé par des handshakes Noise complets en mémoire et entre endpoints réels (y compris le rejet d'un client en clair et le refus sur clé changée) ;
- la migration JSON → SQLite, les règles de groupes (succession, purge d'historique), les accusés et le composer ont chacun leur suite.

`scripts/integration_test.sh` exécute exclusivement le scénario P2P headless : handshake Noise, identité applicative et échange d'un message sur une vraie socket. La découverte UDP entre deux processus complets reste un test manuel.

### Convention : les tests vivent dans `src/tests/`

Les tests unitaires ne sont **pas** dans un `mod tests` en fin de fichier source, mais dans un fichier miroir de [src/tests/](../src/tests/) raccordé depuis le module testé :

```rust
// en bas de src/app/groups.rs
#[cfg(test)]
#[path = "../tests/test_app_groups.rs"]
mod tests;
```

Le nom du fichier suit le chemin du module (`src/ui/composer/mod.rs` → `src/tests/test_ui_composer_mod.rs`). Les tests gardent l'accès aux éléments privés (c'est un module fils), mais les fichiers sources restent courts et lisibles. **En ajoutant un module, ajouter son fichier de tests au même endroit.** Deux exceptions assumées : les modules dont les tests tiennent en quelques lignes (`protocol.rs`, `metrics.rs`, `ui/outbound.rs`) gardent un `mod tests` inline, et [tests/p2p_e2e.rs](../tests/p2p_e2e.rs) est un test d'intégration externe, qui ne voit donc que l'API publique du crate.

## Scripts

Tous les scripts portent un en-tête `# <nom> — <rôle>` en première ligne utile.

| Script | Rôle | Quand |
|---|---|---|
| [run-multi.sh](../scripts/run-multi.sh) | Lance N instances locales (`ABCOM_INSTANCE`) pour tester le P2P | Développement quotidien (`make run2`) |
| [seed-demo.py](../scripts/seed-demo.py) | Remplit trois instances d'un jeu de données de démonstration | Captures, démos, tests manuels |
| [integration_test.sh](../scripts/integration_test.sh) | Scénario P2P headless (handshake + message authentifié) | **Exécuté par la CI `main`** |
| [build-and-distribute.sh](../scripts/build-and-distribute.sh) | Build release + archive de distribution dans `dist/` | Préparation d'une livraison manuelle |
| [deploy.sh](../scripts/deploy.sh) | Prépare un binaire pour un test multi-machines | Test sur un vrai LAN |
| [install.sh](../scripts/install.sh) / [abcom-install.sh](../scripts/abcom-install.sh) | Installation Linux (avec / sans service systemd) | Poste utilisateur |
| [uninstall.sh](../scripts/uninstall.sh) | Désinstallation Linux + service | Poste utilisateur |
| [install-windows.ps1](../scripts/install-windows.ps1) | Installation et raccourcis Windows | Poste utilisateur Windows |

## Intégration continue

Quatre workflows GitHub Actions :

| Workflow | Déclencheur | Étapes |
|---|---|---|
| [ci-dev.yml](../.github/workflows/ci-dev.yml) | PR/push vers `dev` | format · Clippy · build · tests sur Linux · `cargo audit` + `cargo deny` · MSRV · checks macOS/Windows |
| [ci-main.yml](../.github/workflows/ci-main.yml) | PR/push vers `main` | idem + scénario P2P headless |
| [release.yml](../.github/workflows/release.yml) | tag `v*` | Binaires Linux/macOS/Windows + `SHA256SUMS` attachés à une GitHub Release |
| [dependencies.yml](../.github/workflows/dependencies.yml) | 1er du mois | Rapport `cargo outdated` + `cargo audit` (informatif) |

Le hook local `.githooks/pre-commit` bloque tout commit mal formaté avant même la CI.

### MSRV

`rust-version` dans [Cargo.toml](../Cargo.toml) déclare la version minimale de Rust, vérifiée par le job `msrv` de la CI `dev`. Elle est actuellement **1.95, c'est-à-dire le dernier stable** : la contrainte ne vient pas du code d'abcom mais du build script de `libsqlite3-sys` (tiré par `rusqlite` 0.40), qui échoue dès 1.94. Elle pourra redescendre à la prochaine montée de `rusqlite` — le job CI le signalera.

### Chaîne d'approvisionnement

`cargo audit` (vulnérabilités RUSTSEC) et `cargo deny` (licences, sources, doublons de crates, configuré dans [deny.toml](../deny.toml)) tournent désormais **sur `dev` comme sur `main`**, à partir de binaires pré-compilés (`taiki-e/install-action`) au lieu d'un `cargo install` recompilé à chaque exécution.

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

### Releases

Chaque merge `dev` → `main` correspond à une version SemVer, taguée `v0.x.x`, avec `CHANGELOG.md` mis à jour au préalable (format Keep a Changelog).

## Dépendances

Les dépendances directes sont déclarées dans [Cargo.toml](../Cargo.toml) et verrouillées par `Cargo.lock`. Les principales :

| Crate | Rôle |
|---|---|
| `tokio` (features explicites, pas `full`) | Runtime asynchrone du réseau |
| `eframe` / `egui` / `egui_extras` 0.31 | Fenêtre native, interface, chargement d'images animées — **renderer wgpu**, `glow` explicitement retiré des features (cf. ci-dessous) |
| `mimalloc` / `libmimalloc-sys` | Allocateur global, et restitution des pages à l'OS au repli dans le tray |
| `serde` / `serde_json` | Sérialisation des paquets réseau et des données |
| `rusqlite` (`bundled`) | Stockage SQLite embarqué |
| `snow` + `blake2` | Handshake Noise, empreintes et dérivation de PSK |
| `socket2` | Options bas niveau des sockets (broadcast, réuse de port) |
| `image`, `resvg` (optionnel), `rfd`, `walkdir`, `zip` | Décodage d'images, import SVG (feature `avatar-svg`), sélecteurs de fichiers natifs, parcours et archivage de dossiers |
| `tray-icon`, `notify-rust`, `auto-launch` | Icône résidente, notifications système, autostart |
| `rodio` | Sons de notification |
| `ehttp`, `chrono`, `anyhow`, `dirs` | Requêtes Klipy, horodatage, erreurs, chemins par plateforme |

### Renderer : wgpu, pas OpenGL

`eframe` est déclaré avec `default-features = false` **précisément pour retirer `glow`** : OpenGL est déprécié sur macOS et y est émulé au-dessus de Metal, au prix d'un contexte GPU disproportionné pour une interface 2D. Mesures A/B au repos, même machine, même build debug, fenêtre visible :

| | Glow (OpenGL) | wgpu (Metal) | wgpu + mimalloc |
|---|---|---|---|
| `IOAccelerator (graphics)` | 29,6 Mo | 6,2 Mo | 3,9 Mo |
| RSS | 155,8 Mo | 146,0 Mo | 138,9 Mo |
| Empreinte physique (pic) | 132,4 Mo | 110,8 Mo | 110,6 Mo |

Vérifier après toute montée d'`eframe` que `cargo tree | grep -i glow` ne renvoie **rien** : réintroduire les features par défaut relierait silencieusement le backend OpenGL.

**Licence du projet : AGPL-3.0** — alignée le 08/08/2026. `LICENSE` (texte intégral de la GNU AGPL v3) et l'onglet Licence de l'application faisaient déjà foi ; `Cargo.toml` déclarait `MIT` par erreur et déclare désormais `AGPL-3.0-only`. Ce n'était pas une double licence, juste une incohérence. Ressources embarquées : police Inter (OFL-1.1, licence dans `assets/fonts/`), jeu d'emojis type Twemoji.

Outils d'entretien : `cargo audit` (vulnérabilités) et `cargo deny check` (licences, sources, doublons) — les deux exécutés par la CI `dev` **et** `main` ; `cargo outdated` (rapport mensuel automatique), `cargo update` (mises à jour semver). `Cargo.lock` est versionné : builds reproductibles.

## Fichiers de suivi

- [CHANGELOG.md](../CHANGELOG.md) — modifications notables, alimenté en continu, consolidé à chaque release.
- [old/](../old/) — documentation historique (audits, plans, ADR, anciennes versions), conservée en l'état.
