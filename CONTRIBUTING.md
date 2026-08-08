# Contribuer à Abcom

Merci de passer par ici avant d'ouvrir une PR — les règles tiennent en une page.

## Mise en place

```bash
git clone <repo> && cd abcom
git config core.hooksPath .githooks   # pre-commit : fmt + clippy bloquants
cp .env.example .env                  # optionnel : clé Klipy, passphrase de salon
cargo build
```

Détails d'installation et d'outillage : [docs/07-developpement.md](docs/07-developpement.md).

## La barrière verte

Aucun commit ne part sans ces quatre commandes au vert — c'est exactement ce que la CI exige :

```bash
cargo fmt --all
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## Branches et commits

```
main          ← production stable (PR uniquement depuis dev)
 └── dev      ← intégration (PR uniquement depuis feature/)
      └── feature/<nom>   ← une fonctionnalité complète
           └── task/<nom> ← une sous-tâche
```

Push direct interdit sur `main` et `dev`. Nommage en kebab-case.

Messages : `type(scope): description courte en français`, à l'impératif, sans majuscule après le `:` ni point final. **Un commit = une intention** : jamais un `feat` et un `fix` ensemble.

Types : `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`.

## Conventions de code

- **Commentaires : une ligne**, sauf cas exceptionnel (invariant subtil, `SAFETY`, décision contre-intuitive). Un commentaire dit *pourquoi*, pas *quoi* ; le contexte long va dans `docs/`, `AUDIT.md` ou le message de commit.
- **Tests dans `src/tests/`**, raccordés au module testé par `#[path = "../tests/test_<module>.rs"] mod tests;`. En ajoutant un module, ajouter son fichier de tests.
- **Pas de `.lock().unwrap()`** sur un `std::sync::Mutex` : utiliser `util::MutexExt::lock_safe()`, qui survit à l'empoisonnement.
- **Pas de `println!`/`eprintln!`** en production : `tracing::{debug,info,warn,error}`.
- Toute perte de paquet réseau se compte (`metrics::record_packet_dropped`) et se journalise.

## Pour les agents IA

- Toujours partir de `dev` à jour ; branche `feature/` ou `task/` selon la portée.
- **Jamais de `git push` sans accord explicite.**
- **Pas de trailer `Co-Authored-By`.**
- `AVANCEMENT.md` se met à jour **sur `dev` uniquement** : c'est ce qui le garde sans conflits.

## Licence

Le projet est sous **AGPL-3.0** ([LICENSE](LICENSE)). En contribuant, vous acceptez que votre code soit distribué sous cette licence.
