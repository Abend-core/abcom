# Plan de maintenabilité — exécutable pas à pas

> Compagnon opérationnel de [`AUDIT.md`](AUDIT.md). Découpé en **phases
> indépendantes**, de la moins risquée à la plus structurante. Chaque phase est
> une unité de travail autonome : une branche, un commit, des critères
> d'acceptation vérifiables. Dérouler dans l'ordre ; ne pas enchaîner deux phases
> dans le même commit.
>
> Révisé le 5 août 2026 (branche de référence `dev`).

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
1. Supprimer le dossier legacy `old/` (24 fichiers suivis par git) :
   `git rm -r old/`. Vérifier au préalable qu'aucun fichier de `docs/` ne
   référence `old/` (`grep -rn "old/" docs/ README.md`). S'il reste un lien,
   le rediriger vers l'équivalent dans `docs/`.
2. Supprimer le dossier vide `font 2/` à la racine (non suivi) : `rm -rf "font 2"`.
3. Compléter `.gitignore` — ajouter `.DS_Store` et `*.log` (le reste est déjà là :
   `/target`, `.env`, `/dist`, `*.zip`, `nohup.out`).
4. `Cargo.toml` : corriger le champ `repository` (`github.com/rxdy/abcom` →
   l'URL réelle de l'org `Abend-core`) après confirmation de l'URL exacte. Ne
   **pas** toucher au numéro de version dans cette phase (décision de versionnage
   à part).

**Acceptation :**
- `git status` propre, `old/` et `font 2/` absents.
- `cargo build` OK (aucun de ces fichiers n'était référencé par le code).
- `git grep -n "old/" -- docs README.md` ne renvoie plus de lien mort.

---

## Phase 2 — Verrou anti-empoisonnement (risque faible, fort levier)

**Objectif :** neutraliser les **55 `lock().unwrap()`** qui font tomber toute
l'app si un thread panique en tenant le verrou (mutex empoisonné). Pas de nouvelle
dépendance (rester sur `std::sync::Mutex`).

**Étapes :**
1. Créer `src/util/mutex.rs` (et déclarer `mod util;` dans `main.rs`) avec une
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

## Tableau de bord (mettre à jour après chaque phase)

| Phase | Signal cible | Avant | Après |
|-------|-------------|-------|-------|
| P1 | `old/` suivi, `font 2/` | 24 fichiers + dossier | 0 |
| P2 | `lock().unwrap()` hors tests | 55 | 0 |
| P3 | `eprintln!`/`println!` prod | 34 | ~0 |
| P4 | `#[allow(dead_code)]` | 16 | 0 (ou justifiés) |
| P5 | scan emoji dupliqué | 3 copies | 1 |
| P6 | champs de `AbcomApp` | 90 | ~8 sous-structs |
