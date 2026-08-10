# Plan de maintenabilité — exécutable pas à pas

> Compagnon opérationnel de [`AUDIT.md`](AUDIT.md). Découpé en **phases
> indépendantes**, de la moins risquée à la plus structurante. Chaque phase est
> une unité de travail autonome : une branche, un commit, des critères
> d'acceptation vérifiables. Dérouler dans l'ordre ; ne pas enchaîner deux phases
> dans le même commit.
>
> Révisé le 5 août 2026 (branche de référence `dev`).
>
> **✅ Exécuté intégralement le 5 août 2026** — P1 à P6 (dont les 6 sous-étapes
> de P6) déroulées sur `dev`, un commit par phase, barrière verte à chaque étape.
> Voir le tableau de bord en fin de document pour le détail avant/après.

## Règles du jeu (à respecter à chaque phase)

- **Git** : travailler sur une branche dédiée par phase (`chore/maint-p1`, …),
  **ne jamais `git push`**, **ne pas ajouter de co-auteur** dans les commits.
  Commiter uniquement quand une phase est verte de bout en bout.
- **Barrière verte obligatoire** avant tout commit (c'est la CI qui bloque sinon) :
  ```bash
  cargo fmt --all
  cargo fmt --all --check
  cargo clippy --all-targets -- -D warnings
  cargo test
  ```
  Aucun warning clippy toléré : la CI (`ci-main.yml`) tourne `clippy -D warnings`.
- **Périmètre** : ces phases sont du refactor **sans changement de comportement**.
  Si un test doit changer de valeur attendue, c'est un signal d'alerte — s'arrêter
  et documenter pourquoi, ne pas « ajuster » le test à l'aveugle.
- **Une phase = un diff relisible.** Préférer plusieurs petits commits internes à
  un commit fleuve. Ne pas mélanger renommage mécanique et changement de logique.

---

## Phase 1 — Hygiène du dépôt (risque nul, ~30 min)

**Objectif :** supprimer les résidus et fiabiliser `.gitignore` / `Cargo.toml`.

**Étapes :**
1. ~~Supprimer `old/`~~ — **annulé après vérification** : `old/` est référencé et
   décrit explicitement comme archive historique volontaire par
   `README.md`, `docs/07-developpement.md` et `docs/08-historique-et-audits.md`
   (« conservé tel quel », renvois précis type `old/docs/06-audit-performance.md
   §6 »). Le supprimer casserait ces liens documentés et retirerait une
   référence assumée — ce n'est pas un résidu. **Ne pas toucher `old/`.**
2. Supprimer le dossier vide `font 2/` à la racine (non suivi) : `rm -rf "font 2"`. ✅
3. Compléter `.gitignore` — ajouter `.DS_Store` et `*.log` (le reste est déjà là :
   `/target`, `.env`, `/dist`, `*.zip`, `nohup.out`). ✅
4. `Cargo.toml` : corriger le champ `repository` — remote réel confirmé
   `https://github.com/Abend-core/abcom` (`git remote -v`). ✅ Ne **pas** toucher
   au numéro de version dans cette phase (décision de versionnage à part).

**Acceptation :**
- `git status` propre, `font 2/` absent, `old/` conservé intentionnellement.
- `cargo build` OK.

---

## Phase 2 — Verrou anti-empoisonnement (risque faible, fort levier)

**Objectif :** neutraliser les **55 `lock().unwrap()`** qui font tomber toute
l'app si un thread panique en tenant le verrou (mutex empoisonné). Pas de nouvelle
dépendance (rester sur `std::sync::Mutex`).

**Étapes (exécutées telles quelles) :**
1. Créer `src/util.rs` (et déclarer `mod util;` dans `main.rs`) avec une
   extension de trait :
   ```rust
   use std::sync::{Mutex, MutexGuard};

   /// Verrouillage tolérant à l'empoisonnement : si un thread a paniqué en
   /// tenant le verrou, on récupère la donnée plutôt que de propager la panique
   /// (l'état reste cohérent au grain de nos mutations, toutes courtes).
   pub trait MutexExt<T> {
       fn lock_safe(&self) -> MutexGuard<'_, T>;
   }

   impl<T> MutexExt<T> for Mutex<T> {
       fn lock_safe(&self) -> MutexGuard<'_, T> {
           self.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
       }
   }
   ```
2. Remplacer **mécaniquement** tous les `.lock().unwrap()` hors tests par
   `.lock_safe()` et `use crate::util::mutex::MutexExt;` dans chaque fichier
   concerné. Sites connus : tout `ui/` (avatar, mod, input_bar, events…),
   `klipy.rs` (mutex interne), `network/secure.rs:281` (`TrustStore`).
   Recensement : `grep -rn 'lock().unwrap()' src --include='*.rs' | grep -v /tests/`.
3. **Ne pas** toucher aux `.lock().await` (mutex tokio de `network/pool.rs`) :
   ce sont des `tokio::sync::Mutex`, pas concernés par l'empoisonnement.

**Acceptation :**
- `grep -rn 'lock().unwrap()' src --include='*.rs' | grep -v /tests/` renvoie **0**.
- `cargo clippy --all-targets -- -D warnings` vert, `cargo test` vert.
- Diff purement mécanique (aucune logique modifiée).

---

## Phase 3 — Logging structuré (risque faible)

**Objectif :** remplacer les **34 `eprintln!`/`println!`** de production par du
logging à niveaux, horodaté, redirigeable vers un fichier — condition d'un
diagnostic à distance et prérequis de R3 (remontée d'erreurs à l'utilisateur).

**Étapes :**
1. Ajouter les dépendances : `tracing` + `tracing-subscriber` (feature
   `env-filter`). Init dans `main.rs` avant tout spawn :
   ```rust
   tracing_subscriber::fmt()
       .with_env_filter(
           tracing_subscriber::EnvFilter::try_from_default_env()
               .unwrap_or_else(|_| "abcom=info".into()),
       )
       .init();
   ```
2. Remplacer chaque `eprintln!` par le macro de niveau adéquat :
   - échec réseau / handshake / envoi perdu → `tracing::warn!` ou `error!`
   - retry, cycle de vie, découverte → `tracing::info!`/`debug!`
   Conserver le contexte (`%addr`, `%peer`) en champs structurés plutôt qu'en
   interpolation brute.
3. Recensement : `grep -rn 'eprintln!\|println!' src --include='*.rs' | grep -v /tests/`.
   Laisser tels quels les éventuels `println!` d'un binaire CLI d'aide s'il y en a.

**Acceptation :**
- Plus aucun `eprintln!` de production (les `println!` légitimes documentés).
- `RUST_LOG=abcom=debug cargo run` affiche des lignes horodatées à niveaux.
- CI verte.

> Après cette phase, le stub de retry (`events.rs:418`, item R1) et les échecs
> `pool.rs` (R3) deviennent des `warn!`/`error!` traçables — mais **brancher le
> retry réel et la bannière UI reste hors périmètre maintenabilité** (voir AUDIT
> §4/§13, à traiter séparément).

---

## Phase 4 — Purge du code mort (risque faible)

**Objectif :** supprimer les **16 `#[allow(dead_code)]`** : chaque symbole est
soit branché, soit supprimé. Un `allow(dead_code)` ment sur la couverture réelle.

**Étapes :**
1. Lister : `grep -rn 'allow(dead_code)\|allow(unused' src --include='*.rs'`.
2. Pour chaque symbole, retirer l'attribut et laisser le compilateur signaler.
   Décision au cas par cas :
   - Fonction jamais appelée et sans usage prévu → **supprimer** (ex. candidats
     cités par l'audit : `app/receipts.rs::is_message_pending`,
     `app/messages.rs::get_conversations`, module `composer/cursor.rs` — vérifier
     d'abord qu'ils sont vraiment orphelins via `grep`).
   - Symbole réellement utile mais non branché → **le brancher** (rare ; si le
     câblage est non trivial, le sortir de cette phase et ouvrir une note).
3. Recompiler sans warning après chaque suppression.

**Acceptation :**
- `grep -rn 'allow(dead_code)' src --include='*.rs'` renvoie **0** (ou une liste
  résiduelle **justifiée en commentaire** au-dessus de chaque attribut restant).
- `cargo clippy --all-targets -- -D warnings` vert (aucun `dead_code` réapparu).
- `cargo test` vert.

---

## Phase 5 — Centraliser le thème et dédupliquer le scan d'emojis (risque faible)

**Objectif :** deux nids de duplication cités à l'audit §2.

**Étapes A — `ui/theme.rs` :**
1. Créer `src/ui/theme.rs` regroupant les constantes visuelles dispersées :
   `LINE_HEIGHT: f32 = 22.0` (dupliqué dans le composeur), et les couleurs
   récurrentes (gris 140/150/160, bleu 80-180-255, liseré 96-96-100) sous des
   noms parlants (`TEXT_MUTED`, `ACCENT`, `SEPARATOR`…).
2. Remplacer les littéraux en dur dans `chat_panel.rs`, `input_bar.rs`,
   `sidebar.rs`, `composer/*` par ces constantes. Repérer les doublons :
   `grep -rn '22.0\|Color32::from_rgb' src/ui`.

**Étapes B — itérateur emoji commun :**
1. Extraire l'itération « séquence de 2 caractères puis 1 dans `emoji_map` »
   réimplémentée dans `markdown.rs::is_text_emoji_only`,
   `emoji_picker.rs::render_inline` et `composer/mod.rs::composer_caret_positions`.
2. La factoriser (fonction ou itérateur) dans un module partagé, et faire
   consommer les trois appelants.

**Acceptation :**
- Plus de `22.0` en dur pour la hauteur de ligne ; couleurs nommées.
- Le scan d'emoji n'existe qu'en un seul endroit (les 3 tests emoji restent verts :
  `test_ui_markdown`, `test_ui_emoji_picker`, `test_ui_composer_mod`).
- CI verte, **rendu identique** (comparer visuellement `make run2` avant/après).

---

## Phase 6 — Éclater l'objet-dieu `AbcomApp` (risque moyen, plus structurant)

**Objectif :** `AbcomApp` (`ui/mod.rs`) a **90 champs** à plat — principal frein à
la maintenabilité. Regrouper par sous-états cohérents. **Phase la plus délicate :
la faire seule, en dernier, par petits pas compilables.**

**Découpage cible (sous-structs `Default`-ables) :**
- `NetworkChannels` — les 9 `mpsc::Sender<*Request>` + `event_rx`.
- `EmojiPickerState` — `emoji_textures*`, `emoji_map`, `emoji_category`,
  `emoji_aliases`, `shortcode_selected`, `emoji_decode_rx`…
- `GifPickerState` — `show_gif_picker`, `gif_picker_tab`, `gif_query`,
  `gif_feed`/`meme_feed`/`sticker_feed`, `gif_last_input`.
- `ComposerState` — `input`, `input_cursor_char`, `input_selection_anchor`,
  `input_has_focus`, `input_scroll_lines`, `drafts`, `pending_attachments`.
- `ModalsState` — group modal, rename, settings, participants, confirmations.
- `MediaState` — `media_textures`, `media_viewer`, avatars de rendu.

**Méthode (impérative pour rester vert) :**
1. **Une sous-struct à la fois**, dans son propre commit. Créer la struct,
   déplacer les champs, `impl Default`, l'ajouter à `AbcomApp`.
2. Mettre à jour les accès : `self.input` → `self.composer.input`, etc. Le
   compilateur guide exhaustivement — traiter toutes les erreurs avant de compiler
   proprement.
3. `cargo test` + `clippy -D warnings` vert **après chaque sous-struct**, jamais
   plusieurs en vol.
4. Ne **pas** changer la logique de `update()` dans cette phase : uniquement les
   chemins d'accès aux champs.

**Acceptation :**
- `AbcomApp` passe de 90 champs à ~6-8 sous-structs + quelques champs racine.
- Comportement inchangé (`make run2` : envoi, emoji, GIF, médias, groupes OK).
- CI verte.

> **Suite naturelle (hors de ce plan, à ouvrir ensuite) :** une fois `AbcomApp`
> assaini, découper les gros fichiers `chat_panel.rs` (1 522) / `input_bar.rs`
> (1 164) en sous-modules sur le modèle de `composer/`. C'est plus simple une fois
> l'état regroupé.

---

## Ordre recommandé & parallélisme

```
P1 (hygiène) ─┐
P2 (mutex)   ─┼─ indépendantes, mergeables dans n'importe quel ordre
P3 (logging) ─┤
P4 (dead)    ─┘
P5 (thème/emoji)  ← après P4 (moins de bruit)
P6 (AbcomApp)     ← en dernier, seul, gros diff mécanique
```

P1→P4 sont mécaniques et sûres : idéales pour enchaîner en premier et faire chuter
la dette mesurable de l'audit. P6 est la seule qui demande de la vigilance —
la traiter isolément.

## Tableau de bord — état final (5 août 2026)

| Phase | Signal cible | Avant | Après | Commit |
|-------|-------------|-------|-------|--------|
| P1 | `font 2/`, `.gitignore`, URL repo | résidu + gitignore incomplet | nettoyé (`old/` conservé, décision documentée) | `691a614` |
| P2 | `lock().unwrap()` hors tests | 55 | **0** | `addbe14` |
| P3 | `eprintln!`/`println!` prod | 34 | **0** (`tracing`) | `f5bf425` |
| P4 | `#[allow(dead_code)]` | 16 | **4** (justifiés en commentaire) | `5d36a52` |
| P5 | scan emoji dupliqué / constantes visuelles | 4 copies / dispersées | **1** (`match_emoji_at`) / `ui/theme.rs` | `d18c188` |
| P6a | `AbcomApp` : canaux réseau | 9 champs à plat | `net: NetworkChannels` | `3b287f5` |
| P6b | `AbcomApp` : picker emoji | 8 champs à plat | `emoji: EmojiPickerState` | `283ecc5` |
| P6c | `AbcomApp` : sélecteur Klipy | 8 champs à plat | `gif_picker: GifPickerState` | `0ca4353` |
| P6d | `AbcomApp` : zone de saisie | 7 champs à plat | `composer: ComposerState` | `d736eaa` |
| P6e | `AbcomApp` : modales | 10 champs à plat | `modals: ModalsState` | `b114e19` |
| P6f | `AbcomApp` : caches média | 8 champs à plat | `media: MediaState` | `38695c6` |
| **P6 total** | **champs de `AbcomApp`** | **90** | **46** (en 6 sous-structs) | — |

Tous les commits sur `dev`, non poussés. `cargo test` : 257/257 verts à l'état final
(272 → 257 : suppression des tests dédiés au code mort purgé en P4).

**Reste ouvert (hors périmètre de ce plan, voir `AUDIT.md`) :** découper les gros
fichiers `chat_panel.rs`/`input_bar.rs`/`ui/mod.rs` en sous-modules (P6 pose le
préalable) — c'est le **seul chantier de maintenabilité pure encore ouvert**.

---

## Suites exécutées hors de ce plan (7-8 août 2026)

Ce plan couvrait la maintenabilité ; la dette réseau/sécurité/CI qu'il renvoyait
à `AUDIT.md` a été traitée dans deux passes distinctes, avec la même barrière
verte à chaque étape :

| Passe | Contenu | Où c'est décrit |
|-------|---------|-----------------|
| 07/08 (`0e5720e`) | Crate en lib, `protocol.rs`, `app/conversation.rs`, `ui/outbound.rs`, versionnage du protocole, retry réel, anti-usurpation, unification des expéditeurs, `UiRuntimeChannels`, test e2e P2P | `AUDIT.md` §3-§5, §8 |
| 08/08 | Durcissement du pool (taille, éviction), remontée des échecs réseau à l'UI, métriques de session, ré-appairage TOFU, `.env`, hook de panique, purge des sauvegardes, transactions de lot SQLite, delta des accusés de lecture, `cargo audit`/`deny` sur `dev`, MSRV, release, documentation | `AUDIT.md` §1, §4-§7, §9, §11, §13 |

**Versionnage Cargo** (point resté en suspens à la fin de ce plan) : réglé —
`1.0.0-beta.1` publiée, plus `rust-version = "1.95"` vérifiée en CI.
