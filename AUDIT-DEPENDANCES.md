# Audit d'exploitation des dépendances — abcom

> **Établi le 8 août 2026, branche `dev`.** Question posée : *utilise-t-on
> réellement ce que nos dépendances savent faire ?* — pas seulement les
> nouveautés des dernières versions, mais **tout ce qui existe déjà**, y
> compris ce qui était disponible bien avant qu'on monte de version.
>
> Méthode : lecture des changelogs officiels **et** des sources vendorées dans
> `~/.cargo/registry`, puis recoupement systématique avec notre code. Chaque
> constat marqué « vérifié » l'a été par lecture de source ou `grep` sur `src/`,
> pas de mémoire.
>
> Compagnon de l'audit de dette de code, exécuté et archivé dans [`old/AUDIT-code-2026-08.md`](old/AUDIT-code-2026-08.md) : ce document-ci ne parle
> que de l'écart entre ce que nos dépendances offrent et ce qu'on en tire.
>
> **Complété le 8 août 2026 (2ᵉ passe)** par une remontée d'historique complète
> sur les dépendances lourdes — voir [§7](#7-remontée-dhistorique-sur-les-dépendances-lourdes).
>
> **Appliqué le 9 août 2026 : 26 constats sur 31.** La colonne « État » du
> tableau de bord dit ce qui a été fait ; les deux exceptions (D12, D19) sont
> justifiées en [§8](#8-les-deux-constats-non-appliqués).

## Verdict en une ligne

Les versions sont toutes au dernier stable. **L'écart n'est pas de fraîcheur,
il est d'usage** : nous réimplémentons à la main plusieurs mécanismes que nos
dépendances fournissent depuis longtemps, et nous laissons de côté des réglages
qui coûtent aujourd'hui de la latence, de la batterie et de la sécurité.

## Tableau de bord

| # | Constat | Impact | État |
|---|---------|--------|------|
| D1 | `TCP_NODELAY` jamais posé sur les sockets de chat | 🔴 latence perçue | ✅ appliqué |
| D2 | Décodage d'images réseau sans borne de dimensions | 🔴 sécurité | ✅ appliqué |
| D3 | `PowerPreference::HighPerformance` par défaut (GPU dédié pour une UI 2D) | 🟠 batterie | ✅ appliqué |
| D4 | `ThemePreference` d'egui réimplémenté à la main | 🟠 dette | ✅ appliqué |
| D5 | `egui::Modal` (dispo depuis 0.30) inutilisé — nos modales sont des `Window` | 🟠 UX | ✅ appliqué |
| D6 | Aucun raccourci via `KeyboardShortcut`/`consume_shortcut` | 🟠 UX | ✅ appliqué |
| D7 | Glisser-déposer de fichiers dans la fenêtre absent | 🟠 UX | ✅ appliqué |
| D8 | `ScrollArea::show_rows` inutilisé — fenêtrage du fil fait main | 🟠 perf | ⏸️ écarté, voir §8 |
| D9 | Pragmas SQLite manquants (`busy_timeout`, `mmap_size`, `cache_size`) | 🟠 perf/robustesse | ✅ appliqué |
| D10 | Un seul index SQLite, alors qu'on filtre surtout sur `to_user` | 🟠 perf | ✅ appliqué |
| D11 | FTS5 compilé dans le binaire mais inexploité (aucune recherche) | 🟠 fonctionnalité | ✅ appliqué |
| D12 | `rfd::AsyncFileDialog` inutilisé — d'où notre bricolage `pending_picker` | 🟠 dette | ⏸️ non appliqué |
| D13 | `egui_kittest` : test d'interaction et de rendu par capture | 🟠 qualité | ✅ appliqué |
| D14 | `tracing` sans aucun span (`#[instrument]`) | 🟢 diagnostic | ✅ appliqué |
| D15 | `snow::Builder::prologue` inutilisé | 🟢 sécurité | ✅ appliqué |
| D16 | `try_reserve_many` / `recv_many` (tokio) réimplémentés ou ignorés | 🟢 simplicité | ✅ appliqué |
| D17 | Feature `zip` tirant zopfli pour rien | 🟢 poids | ✅ appliqué |
| D18 | Pas de journal fichier (`tracing-appender`) | 🟢 diagnostic | ✅ appliqué |
| D19 | `notify-rust` : actions dans les notifications | 🟢 UX | ⏸️ non appliqué |
| D20 | Accessibilité AccessKit : aucun `widget_info` sur les boutons peints | 🟢 a11y | ✅ appliqué |
| D21 | `Options::reduce_texture_memory` (egui 0.28) jamais activé | 🟠 mémoire | ✅ appliqué |
| D22 | `PRAGMA optimize` absent (statistiques du planificateur périmées) | 🟠 perf | ✅ appliqué |
| D23 | Schéma SQLite non `STRICT` | 🟠 robustesse | ⏸️ différé, voir §8 |
| D24 | FTS5 « contentless-delete » : recherche sans dupliquer le contenu | 🟠 fonctionnalité | ✅ appliqué |
| D25 | Colonne `media` en JSON texte : opérateurs `->>` inexploités | 🟢 perf | ✅ appliqué (constat partiellement erroné, voir §8) |
| D26 | `RETURNING` inexploité | 🟢 simplicité | ⏸️ sans objet, voir §8 |
| D27 | `ImageReader::into_dimensions()` : rejeter sans décoder | 🔴 sécurité | ✅ appliqué |
| D28 | `ViewportBuilder` : 4 options sur 32 (pas de taille minimale ni de position) | 🟠 UX | ✅ appliqué |
| D29 | Conflit connu winit/COM sous Windows entre glisser-déposer et `rfd` | 🟠 plateforme | ✅ appliqué |
| D30 | `rusqlite` : `prepare_cached` sur 2 requêtes / 20 | 🟢 diagnostic | ✅ partiellement, voir §8 |
| D31 | `TcpSocket` : régler la socket avant connexion | 🟢 perf | ✅ appliqué |

---

## 1. Réseau — ce qui coûte le plus cher, et qui n'a rien à voir avec les versions

### D1 — `TCP_NODELAY` n'est posé nulle part 🔴

**Vérifié** : `grep -rn "set_nodelay" src/` ne renvoie **rien**. Ni dans
`network/pool.rs`, ni dans `network/server.rs`, ni dans `media_stream.rs`.

Conséquence : l'algorithme de Nagle est actif sur toutes nos connexions. Il
retient les petits paquets jusqu'à recevoir l'ACK du précédent ou remplir un
segment. Or **tout notre trafic de chat est constitué de petits paquets** :
message, indicateur de frappe, ACK, accusé de lecture, réaction. Combiné au
delayed-ACK du récepteur, c'est le cas d'école qui produit des **délais de 40 ms
et plus par message**, sur un LAN où la latence réseau réelle est inférieure à
la milliseconde.

C'est disponible depuis toujours dans tokio (`TcpStream::set_nodelay`), et c'est
probablement le meilleur rapport gain/effort de tout ce document.

*À faire : `set_nodelay(true)` après chaque `TcpStream::connect` et sur chaque
socket acceptée. Le streaming média est le seul endroit où l'on peut discuter —
il envoie de gros blocs, Nagle ne le pénalise pas.*

### D15 — `snow::Builder::prologue` inutilisé 🟢

**Vérifié** : `prologue()` existe (`snow-0.10.0/src/builder.rs:149`), nous ne
l'appelons pas.

Le prologue lie le handshake à un contexte que les deux côtés doivent partager :
il est haché dans l'état du handshake, et toute divergence fait échouer la
poignée de main. C'est fait pour transporter exactement ce qu'on met aujourd'hui
dans le `Hello` **après** le handshake : la version de protocole. Y mettre
`abcom-v2` ferait échouer la négociation entre versions incompatibles *avant*
d'établir la session, au lieu de la refuser après.

`rekey_outgoing`/`rekey_incoming` existent également, pour les sessions
longues. Nos connexions expirent au bout de 5 minutes d'inactivité, donc c'est
sans objet aujourd'hui.

### D16 — tokio : deux API qu'on réimplémente 🟢

- `Sender::try_reserve_many(n)` fait exactement ce que notre boucle manuelle de
  [`ui/outbound.rs`](src/ui/outbound.rs) fait à la main (réserver N places avant
  d'émettre, pour ne jamais envoyer une diffusion à moitié).
- `Receiver::recv_many(&mut buf, limit)` permettrait de traiter les paquets par
  lots dans `run_sender` et la tâche d'écriture du pool, au lieu d'un réveil de
  tâche par paquet.

Nous n'utilisons pas non plus `JoinSet` (**vérifié** : 0 occurrence) : nos
tâches sont lancées et oubliées, ce qui explique qu'on en soit réduit à un
`shutdown_timeout` global à l'arrêt plutôt qu'à un arrêt ordonné.

---

## 2. Sécurité — un durcissement gratuit

### D2 — Décodage d'images réseau non borné en dimensions 🔴

**Vérifié** : `image::load_from_memory` appelle `ImageReader` avec
`Limits::default()`, soit `max_alloc: 512 Mo`, `max_image_width: None`,
`max_image_height: None` (`image-0.25.10/src/io/limits.rs:45`).

Nous appelons cette fonction sur des données venues du réseau :

| Site | Source |
|---|---|
| [`ui/avatar.rs:148`](src/ui/avatar.rs#L148) | avatar annoncé par un pair |
| [`ui/media.rs:454`](src/ui/media.rs#L454) | média reçu |

Un pair authentifié peut donc nous faire allouer jusqu'à **512 Mo d'un coup**
avec un fichier de quelques kilo-octets (une image très large et très plate
compresse extrêmement bien). Le plafond existe, mais il est beaucoup trop haut
pour ce qu'on affiche : nos avatars sont recadrés à `AVATAR_PX` et nos vignettes
passent par `thumbnail`.

*À faire : passer par `ImageReader` avec `max_image_width`/`max_image_height`
bornés (8192 suffit très largement) sur les deux chemins réseau.*

---

## 3. egui / eframe — beaucoup de roue réinventée

### D4 — Nous réimplémentons `ThemePreference` 🟠

**Vérifié** : egui expose `ThemePreference { System, Light, Dark }`
(`egui-0.36.1/src/memory/theme.rs:67`) et suit le thème du système. Nous avons
notre propre `enum ThemePreference` dans [`ui/mod.rs`](src/ui/mod.rs), notre
champ `system_dark_mode`, notre `applied_dark_mode` et notre
`apply_theme_preference`.

C'est du code que la bibliothèque tient à jour à notre place, y compris la
détection des changements de thème système en cours d'exécution — que notre
version, elle, ne capte qu'au démarrage (`get_or_insert_with`).

### D5 — `egui::Modal` inutilisé 🟠

**Vérifié** : `egui::Modal` existe (`containers/modal.rs`), **0 occurrence**
chez nous. Nos modales — paramètres, gestion de salon, renommage, alerte de
changement de clé — sont des `egui::Window` ancrées au centre.

Ce qui manque, et que `Modal` fournit : le voile assombri sur le fond, le
piégeage du focus, la fermeture à l'Échap, et l'impossibilité d'interagir avec
l'arrière-plan. C'est exactement l'item « fermer les popups à l'Échap de manière
homogène » de l'audit §12, résolu par un changement de conteneur.

Point non trivial à surveiller : notre alerte de clé changée **doit** rester
bloquante ; `Modal` va dans le bon sens.

### D6 — Aucun raccourci clavier déclaré 🟠

**Vérifié** : `KeyboardShortcut` et `InputState::consume_shortcut` existent,
**0 occurrence** chez nous. Tous nos raccourcis sont du filtrage manuel
d'événements clavier dans le composeur.

Conséquence directe : il n'existe aucun raccourci **global** (changer de
conversation, ouvrir les paramètres, fermer une popup), ce qui est précisément
l'item ouvert §12 de l'audit. `consume_shortcut` gère aussi le conflit entre
plusieurs consommateurs, ce que notre approche manuelle ne fait pas.

### D7 — Pas de glisser-déposer de fichiers 🟠

**Vérifié** : aucune lecture de `i.raw.dropped_files` / `hovered_files`.

Pour envoyer un fichier, il faut passer par le menu « + » puis le sélecteur
natif. egui expose les fichiers déposés dans la fenêtre depuis très longtemps,
et nous avons déjà tout le pipeline média derrière — c'est du câblage, pas de la
fonctionnalité.

### D8 — Fenêtrage du fil fait à la main 🟠

**Vérifié** : `ScrollArea::show_rows` / `show_viewport` : **0 occurrence**. Le
fil utilise `ScrollArea::vertical().show(...)` et nous gérons nous-mêmes le
nombre de messages rendus via `chat_visible_count` et `CHAT_WINDOW_STEP`.

`show_rows` fait la virtualisation pour nous : seules les lignes réellement
visibles sont mises en page. C'est le mécanisme qui répondrait à l'item §7b de
l'audit (« vérifier que le fenêtrage borne bien le layout sur un long
historique »). La difficulté est réelle et il faut la dire : `show_rows` suppose
des lignes de hauteur uniforme, ce que nos messages n'ont pas. `show_viewport`
est le bon outil pour des hauteurs variables, au prix d'un calcul d'offsets.

### D12 — `rfd::AsyncFileDialog` inutilisé 🟠

**Vérifié** : `AsyncFileDialog` existe (`rfd-0.17.2/src/file_dialog.rs:183`),
**0 occurrence** chez nous.

Nous appelons `rfd::FileDialog` (bloquant) depuis le thread UI. C'est
précisément pour ça que le code contient un mécanisme de report — les champs
`pending_picker`, `pending_avatar_pick`, `pending_export` et le commentaire
« must run before egui rendering to avoid conflicting with the AppKit run-loop
on macOS ». La variante asynchrone est faite pour ce cas ; elle supprimerait ce
bricolage et l'à-coup d'une frame.

### D13 — `egui_kittest` : le test qui nous manque 🟠

**Vérifié** : `egui_kittest 0.36.1` et `kittest 0.4.0` existent et suivent la
version d'egui.

Le harnais pilote l'interface via AccessKit (clics, frappe simulés) et fait de
la **régression par capture d'écran** (`Harness::snapshot`, feature `wgpu`,
mise à jour par `UPDATE_SNAPSHOTS=true`). Nos trois tests de rendu headless
maison ne prouvent que l'absence de panique.

C'est aussi la réponse structurelle à un problème rencontré pendant la montée
egui 0.36 : impossible de valider le rendu autrement qu'en demandant à un humain
de regarder.

### D20 — Accessibilité 🟢

**Vérifié** : aucun `widget_info`, aucun usage explicite d'AccessKit — alors que
la feature `accesskit` est activée dans notre `eframe`. Nos boutons peints à la
main (icônes « + », envoi, croix des chips, coches d'accusé) n'exposent donc
aucun libellé à un lecteur d'écran. C'est l'item §12 de l'audit, et c'est aussi
ce qui conditionne D13 : kittest interroge l'arbre AccessKit pour trouver les
widgets. **Les deux se débloquent ensemble.**

### Autres, non utilisés et probablement pertinents

- `Context::set_zoom_factor` : agrandissement global de l'interface (0 occurrence).
- `Response::context_menu` : menus contextuels au clic droit sur un message
  (0 occurrence) — aujourd'hui tout passe par la barre de survol.
- `egui_extras::TableBuilder` (0 occurrence) : la liste des participants et le
  détail des accusés sont bâtis à la main.
- `include_image!` / `ImageSource` (0 occurrence) : nos icônes sont chargées et
  téléversées manuellement.
- `eframe::App::save` + feature `persistence` : la fenêtre ne retient ni sa
  taille ni sa position. *(Nos préférences, elles, vivent très bien dans la
  table `kv` de SQLite — ce point-là n'est pas une dette.)*

---

## 4. SQLite — le gisement le plus sous-exploité

### D9 — Trois pragmas seulement 🟠

**Vérifié** : nous posons `journal_mode=WAL` et `synchronous=NORMAL`, plus
`user_version`. Aucun de ceux-ci :

| Pragma | Pourquoi il nous manque |
|---|---|
| `busy_timeout` | Deux instances locales (`ABCOM_INSTANCE`) sur des bases distinctes, mais un futur accès concurrent renverrait `SQLITE_BUSY` immédiatement plutôt que d'attendre |
| `mmap_size` | Lectures d'historique par mmap, moins de copies — c'est notre chemin de pagination |
| `cache_size` | Le défaut (2 Mo) est petit pour un historique paginé |
| `temp_store=MEMORY` | Nos tris et index temporaires passent par le disque |

### D10 — Un seul index, sur la mauvaise colonne 🟠

**Vérifié** : le schéma ne crée que `idx_messages_hash ON messages(hash)`. Or le
comptage des `WHERE` de `storage.rs` donne :

```
3 × WHERE to_user      ← aucun index
3 × WHERE id           ← clé primaire, OK
2 × WHERE message_hash ← table reactions/receipts
```

Toutes nos requêtes de conversation (`load_older`, `delete_conversation`,
`export_conversation`, `rename_conversation`) filtrent sur `to_user` et font donc
un **parcours complet de la table `messages`**. Un index sur `(to_user, id)` est
une ligne de SQL et transforme la pagination sur un gros historique.

### D11 — FTS5 est déjà dans le binaire 🟠

**Vérifié** : `libsqlite3-sys-0.38.1/build.rs:132` compile SQLite avec
`-DSQLITE_ENABLE_FTS5`, `-DSQLITE_ENABLE_JSON1` et `-DSQLITE_ENABLE_RTREE`.

Nous payons donc déjà le poids binaire de la recherche plein texte sans en
proposer aucune. **L'application n'a pas de fonction « rechercher dans
l'historique »** — c'est sans doute le manque fonctionnel le plus visible pour un
utilisateur, et l'essentiel du travail est du SQL (`CREATE VIRTUAL TABLE … USING
fts5`, triggers de synchronisation), pas du Rust.

### Features rusqlite non activées, par ordre d'intérêt

| Feature | Apport |
|---|---|
| `bundled-sqlcipher` | Le chiffrement au repos de l'audit §5 devient un flag de compilation |
| `blob` | I/O en flux sur avatars et médias, sans charger le buffer entier |
| `backup` | Sauvegarde à chaud propre plutôt qu'une copie de fichier |
| `trace` | Profilage SQL, pour objectiver les chemins chauds §7b |
| `hooks` | Invalidation de cache pilotée par la base, à la place de nos générations manuelles |

---

## 5. Rendu et plateforme

### D3 — Le GPU dédié pour une interface 2D 🟠

**Vérifié** : `egui-wgpu-0.36.1/src/setup.rs:252` établit
`PowerPreference::from_env().unwrap_or(HighPerformance)`. Notre `NativeOptions`
ne touche pas à `wgpu_options` : nous héritons donc de `HighPerformance`.

Sur une machine à double GPU, cela sélectionne la carte dédiée pour peindre du
texte et des rectangles. `LowPower` est le réglage correct pour nous, et c'est
une ligne. Dans le même objet : `present_mode` et
`desired_maximum_frame_latency` sont reconfigurables à chaud.

### D17 — La feature `zip` tire zopfli pour rien 🟢

**Vérifié** : notre `features = ["deflate"]` active à la fois
`deflate-zopfli` et `deflate-flate2-zlib-rs` (`zip-8.6.0/Cargo.toml`). Or au
niveau de compression par défaut, `write.rs:2124` compare le niveau demandé à
`flate2::Compression::best()` et ne bascule sur zopfli qu'au-delà :
**zopfli n'est jamais exercé**, il n'ajoute que du binaire et du temps de
compilation.

`features = ["deflate-flate2-zlib-rs"]` suffit. À noter également : `zstd` et
`xz` sont disponibles si l'on veut compresser les dossiers plus efficacement.

### D19 — Notifications sans actions 🟢

**Vérifié** : `notify_rust::Notification::action()` existe
(`notification.rs:451`), inutilisé. Des boutons « Répondre » ou « Marquer comme
lu » dans la notification système, sans rouvrir la fenêtre — cohérent avec une
application pensée pour vivre dans la barre de menus.

---

## 6. Observabilité

### D14 — `tracing` utilisé comme un `println!` amélioré 🟢

**Vérifié** : aucun `#[instrument]`, aucun span créé à la main. Nous n'émettons
que des événements plats.

Un span par connexion (`peer`, `addr`) ferait apparaître le pair dans **toutes**
les lignes émises pendant le traitement de cette connexion, au lieu de le
répéter manuellement dans chaque message. C'est le bénéfice principal de
`tracing` par rapport à `log`, et nous ne le prenons pas.

### D18 — Pas de journal fichier 🟢

`tracing-appender` (0.2.5) fournit l'écriture en fichier avec rotation. Nous
avons le hook de panique, donc la cause d'un crash — mais rien de ce qui l'a
précédé. Sur un binaire release sans console, c'est la moitié du diagnostic qui
manque.

---

## 7. Remontée d'historique sur les dépendances lourdes

La première passe ne remontait que de quatre versions. Celle-ci reprend
l'historique **depuis l'origine** des dépendances qui pèsent le plus, mesurées
sur trois axes : lignes de source vendorée, items publics exposés, et symboles
qu'on touche réellement.

### Ce que « lourd » veut dire, chiffré

| Dépendance | Lignes | API publique | Ce qu'on touche | Couverture |
|---|---|---|---|---|
| **SQLite 3.53.2** (via `libsqlite3-sys`) | **269 376** (C) | tout le SQL | ~20 appels | quasi nulle |
| **tokio** 1.53.1 | 99 903 | 1 221 items | 33 symboles | ~3 % |
| **winit** 0.30.13 (via eframe) | 57 414 | 2 000 items | indirect | — |
| **egui** 0.36.1 | 51 714 | 1 903 items | 60 symboles | ~3 % |
| **wgpu** 30.0.0 (via eframe) | 37 737 | 1 788 items | 0 direct | — |
| **image** 0.25.10 | 37 680 | 584 items | 9 symboles | ~1,5 % |
| **chrono** 0.4.45 | 33 951 | 447 items | formatage seul | faible |
| **tracing-subscriber** 0.3.23 | 24 251 | 325 items | `fmt` + `EnvFilter` | faible |
| **rusqlite** 0.40.2 | 21 972 | 476 items | 20 appels | faible |
| **epaint / zip / serde_json** | 15,4 k / 15,7 k / 18,4 k | — | — | — |

`eframe` tire à lui seul **404 crates transitives** : c'est l'essentiel de notre
arbre de dépendances.

**Le constat qui recadre tout : SQLite est notre dépendance la plus lourde et
de très loin la moins exploitée.** 269 000 lignes de C compilées dans le
binaire, pilotées par une vingtaine d'appels. La première passe regardait
`rusqlite`, la fine couche Rust, au lieu du moteur qu'elle enveloppe.

### D21 — `Options::reduce_texture_memory` (egui 0.28, jamais activé) 🟠

**Vérifié** : `egui-0.36.1/src/memory/mod.rs:322`, valeur par défaut `false`,
**0 occurrence** chez nous.

Quand il vaut `true`, egui **libère la copie CPU des images une fois téléversées
en GPU**. Notre application est un cas d'usage direct : textures de médias,
avatars, et les 323 emojis désormais décodés à la demande gardent chacun leur
`ColorImage` côté CPU pour rien.

C'est disponible depuis la 0.28 — bien avant nos montées de version — et c'est
un booléen. Le compromis documenté (impossible de re-sérialiser les images ou de
rendre sans GPU) ne nous concerne pas.

### D22 — SQLite : `PRAGMA optimize` (3.20, généralisé en 3.46) 🟠

Nous posons `journal_mode` et `synchronous`, et nous avons ajouté un `VACUUM`
manuel. Il manque `PRAGMA optimize`, dont le rôle est de tenir à jour les
statistiques du planificateur de requêtes. Recommandation de l'amont : l'exécuter
à la fermeture de chaque connexion de longue durée — exactement notre cas, le
thread de stockage vit aussi longtemps que l'application.

Sans lui, le planificateur choisit ses plans sur des statistiques périmées, ce
qui compte d'autant plus une fois l'index de D10 en place.

### D23 — SQLite : tables `STRICT` (3.37) 🟠

Notre schéma est en typage souple : rien n'empêche d'écrire une chaîne dans
`ts_epoch`, et nos hash `u64` transitent en `i64` par transtypage. Les tables
`STRICT` font rejeter l'écriture par le moteur au lieu de la convertir
silencieusement.

Sur une base locale alimentée par des paquets réseau, c'est un filet de sécurité
qui coûte un mot-clé par table — à faire lors d'une migration de schéma, pas à
chaud.

### D24 — SQLite : FTS5 « contentless-delete » (3.43) 🟠

Complète **D11**. L'objection naturelle à un index plein texte est qu'il
duplique tout le contenu des messages sur le disque. Le mode
`contentless_delete=1` maintient un index interrogeable **sans stocker le texte
une seconde fois**, tout en supportant la suppression — ce qui nous est
indispensable puisqu'on efface des conversations et qu'on purge un ring-buffer.

C'est ce qui rend la recherche réellement acceptable pour nous, et c'est
disponible depuis fin 2023.

### D25 — SQLite : opérateurs JSON `->`/`->>` (3.38) et JSONB (3.45) 🟢

**Vérifié** : notre colonne `media` est un `TEXT` contenant du JSON produit par
`serde_json::to_string`. Toute lecture d'un champ impose donc de désérialiser
l'objet entier côté Rust.

Les opérateurs `->>` permettent d'interroger directement (`WHERE media ->> 'id'
= ?`), et JSONB stocke la même donnée sous forme binaire, plus compacte et plus
rapide à parcourir. Notre requête `delete_by_media_id` gagnerait à en profiter.

Gain modeste au volume actuel : je le classe 🟢 et je le signale surtout parce
que la voie « JSON dans une colonne texte » est un choix qu'on n'a jamais
réévalué depuis la migration.

### D26 — SQLite : `RETURNING` (3.35) 🟢

Récupère les valeurs d'une ligne insérée ou modifiée dans la même requête. Nos
insertions de messages qui ont besoin du rowid font aujourd'hui un second
aller-retour. Marginal à notre échelle, mentionné pour complétude.

### D27 — `image` : lire les dimensions sans décoder (0.22) 🔴

**Vérifié** : `ImageReader::into_dimensions()` existe
(`image-0.25.10/src/io/image_reader_type.rs:302`).

C'est la **bonne** façon de corriger **D2**. Plutôt que de décoder puis espérer
que le plafond d'allocation nous sauve, on lit d'abord l'en-tête, on rejette si
les dimensions sont déraisonnables, et on ne décode qu'ensuite. Le coût est
celui de la lecture d'un en-tête.

Disponible depuis la 0.22, soit trois versions majeures avant celle qu'on
utilise.

### D28 — `ViewportBuilder` : 4 options sur 32 utilisées 🟠

**Vérifié** : `egui-0.36.1/src/viewport.rs` expose 32 méthodes `with_*`. Nous en
utilisons quatre — `with_title`, `with_inner_size`, `with_icon`, `with_bytes`.

Les deux manques qui se voient à l'usage :

- **`with_min_inner_size`** : rien n'empêche de réduire la fenêtre à une taille
  où l'interface n'a plus de sens (notre sidebar seule fait 220 px).
- **`with_position`** : couplé à la feature `persistence` d'eframe, c'est la
  restauration de la position de fenêtre entre deux lancements.

Également disponibles et à considérer : `with_always_on_top`, `with_window_level`,
`with_taskbar`, `with_transparent`.

### D29 — `with_drag_and_drop(false)` sous Windows 🟠

Point subtil relevé dans la doc d'egui, qui renvoie à celle de winit : le
glisser-déposer OLE de winit **entre en conflit sous Windows avec les boîtes de
dialogue de fichiers basées sur COM** — c'est-à-dire `rfd`, que nous utilisons.

Nous n'avons jamais testé `scripts/install-windows.ps1` ni le sélecteur de
fichiers sur cette plateforme (item ouvert §11 de l'audit). À vérifier en même
temps que D7 (activer le glisser-déposer) et D12 (passer `rfd` en asynchrone) :
les trois se tiennent.

### D30 — `rusqlite` : intégration date-heure et `trace_v2` 🟢

L'historique 0.30 → 0.40, que je n'avais pas pu lire à la première passe (le
`Changelog.md` du dépôt s'arrête à 2018), a été récupéré via le flux des
releases :

- **0.33** : liaisons sûres pour `sqlite3_trace_v2` — profilage SQL réel, à
  rapprocher de la feature `trace` déjà citée ;
- **0.39** : prise en charge de `chrono`, `jiff` et `time`, variantes horodatage
  Unix comprises. Nous stockons un `ts_epoch INTEGER` et reformatons en Rust à
  chaque affichage ;
- **0.38** : le cache d'instructions préparées est devenu une feature optionnelle
  — nous l'avons par le jeu des défauts, mais nous n'appelons `prepare_cached`
  qu'à **2 endroits** sur une vingtaine de requêtes ;
- **0.40** : correction d'une **injection SQL dans la gestion des noms de
  SAVEPOINT**. Nous sommes en 0.40.2, donc couverts — mais c'est une bonne
  raison de ne pas laisser cette dépendance vieillir.

### D31 — `tokio::net::TcpSocket` pour régler la socket avant connexion 🟢

Complète **D1**. `TcpStream::connect` ne laisse rien régler avant l'établissement
de la connexion. `TcpSocket` (présent depuis le début de la série 1.x) donne
accès à `set_recv_buffer_size`, `set_send_buffer_size`, `set_linger`,
`set_reuseaddr` avant le `connect`.

À traiter dans le même passage que `set_nodelay`, puisqu'il s'agit du même
fichier et du même sujet.

### Ce que j'ai regardé et écarté

Pour être clair sur le périmètre, voici ce que j'ai parcouru sans rien en tirer
qui mérite un constat :

| Dépendance | Historique parcouru | Verdict |
|---|---|---|
| **tokio** `sync`/`net`/`io` | sections « Added » de toute la série 1.x | Rien au-delà de D16 et D31. L'essentiel des gains est interne et nous en bénéficions sans rien faire |
| **wgpu**, **winit** | survol | Pilotés uniquement à travers eframe : la surface actionnable se résume à D3 et D28 |
| **chrono** | série 0.4 | Nous n'utilisons que le formatage, et c'est le bon usage ici |
| **serde / serde_json** | séries complètes | Désérialisation empruntée et `RawValue` sans intérêt à nos tailles de paquets |
| **zip** | 0.5 → 8.x | Rien de plus que D17 ; `zstd` reste une option si les dossiers deviennent lourds |
| **snow** | 0.1 → 0.10 | Rien au-delà de D15 (`prologue`) ; le crate est petit et nous en utilisons l'essentiel |
| **SQLite avant 3.20** | non parcouru | Décision assumée : notre usage est du SQL de base, l'historique ancien n'apporterait rien |

---

## 8. Les constats non appliqués, et pourquoi

Cinq exceptions sur trente et un. Elles sont ici parce qu'un rapport où tout
serait coché serait moins utile qu'un rapport honnête.

### D12 — `rfd::AsyncFileDialog` : non appliqué ⏸️

Le raisonnement du constat tient toujours, mais la conversion est plus risquée
que le gain. Sur macOS, `NSOpenPanel` **doit** s'ouvrir sur le thread principal :
c'est précisément pour cela que le code actuel reporte l'ouverture d'une frame
au lieu d'appeler depuis un thread de fond. La variante asynchrone gère ce
dispatch elle-même, mais son `Future` demande un exécuteur que l'UI n'a pas sous
la main — le runtime tokio vit dans `main` et l'interface n'en garde pas de
handle.

Autrement dit : convertir demande de faire remonter un handle de runtime
jusqu'à l'UI, pour supprimer **une frame de latence**. Et l'échec se
manifesterait par un sélecteur de fichiers cassé — une fonction centrale — que
je ne peux vérifier par aucun test automatisé.

Gain : une frame. Risque : le sélecteur de fichiers. J'ai laissé en l'état.

### D19 — Actions dans les notifications : non appliqué ⏸️

`notify-rust` 4.18 les gère bien sur macOS (`mac_notification_sys`), donc le
constat était juste. Deux obstacles à l'application :

1. sur macOS, `show()` **bloque en attendant la réponse** de l'utilisateur — il
   faudrait donc router cette réponse depuis le thread détaché jusqu'à l'UI ;
2. les boutons d'action n'apparaissent de façon fiable que si l'application est
   un bundle `.app` avec identifiant — or notre binaire n'est ni bundlé ni signé
   (item ouvert §11 de `AUDIT.md`).

Livrer un bouton « Répondre » qui n'apparaît pas serait pire que pas de bouton.
À reprendre en même temps que la signature macOS.

### D8 — Virtualisation du fil : écarté après examen ⏸️

Le constat disait « `show_rows` inutilisé ». Après examen, `show_rows` suppose
des lignes de **hauteur uniforme**, ce que nos messages n'ont pas (markdown,
médias, citations, réactions). `show_viewport` gère les hauteurs variables mais
exige de connaître les offsets à l'avance — ce qui reviendrait à reconstruire
notre fenêtrage sous une autre forme.

Or notre fenêtrage manuel (`chat_visible_count`, `CHAT_WINDOW_STEP`) **remplit
déjà exactement le même rôle** : il borne le nombre de lignes mises en page. Ce
n'était donc pas une lacune mais une implémentation différente d'un même besoin.
Constat requalifié.

### D23 — Tables `STRICT` : différé ⏸️

Techniquement faisable — nos colonnes utilisent déjà des types acceptés. Mais
passer une table existante en `STRICT` impose de la recréer et d'y recopier les
données : c'est une migration sur l'historique réel d'un utilisateur, pour un
gain de robustesse à l'écriture, sur une base alimentée par notre seul code.

À faire lors de la prochaine migration de schéma déjà nécessaire pour une autre
raison, pas isolément.

### D26 — `RETURNING` : sans objet ⏸️

Vérification faite, aucun appelant n'a besoin du rowid après insertion. Le
constat listait une capacité disponible, pas un manque réel.

### D25 et D30 — constats partiellement erronés, corrigés ✅

Honnêteté sur mes propres erreurs :

- **D25** affirmait qu'on désérialise le JSON en Rust pour interroger la colonne
  `media`. C'est faux : `delete_by_media_id` utilisait déjà `json_extract`. Seule
  la modernisation en opérateur `->>` a été appliquée.
- **D30** annonçait `prepare_cached` sur « 2 requêtes sur 20 ». C'était juste,
  et c'est corrigé pour les requêtes réellement répétées (pagination, export,
  GC des médias, recherche). Les requêtes de démarrage restent en `prepare` :
  les mettre en cache n'a aucun sens pour un appel unique. L'intégration
  date-heure de rusqlite et la feature `trace` n'ont **pas** été adoptées : notre
  `ts_epoch INTEGER` convient, et `trace` est un outil de diagnostic à activer
  ponctuellement, pas en permanence.

---

## 9. Moteur de stockage : rester sur SQLite, ou passer à un moteur Rust ?

Étude demandée le 9 août 2026 : *« un projet qui ne consomme pas de ressources,
fiable et compatible SQLite — sans réinventer la roue »*, plus le chiffrement.

### Ce qui existe réellement

| Candidat | Version | Compatible SQLite | Verdict |
|---|---|---|---|
| **Turso Database** (ex-limbo) | `0.8.0-pre.3` | dialecte SQL, **format de fichier** et API C | Le seul candidat sérieux |
| **libSQL** | production | fork de SQLite | Écrit en **C** : ne répond pas à la demande |
| **GlueSQL** | 0.19 | SQL partiel, pas le format de fichier | Réécriture complète de notre couche |
| **redb / sled / fjall** | — | aucune | Ce sont des magasins clé-valeur, pas du SQL |

Turso est le seul projet qui coche « compatible SQLite » au sens fort : même
dialecte, **même format de fichier** — une base `abcom.db` existante s'ouvrirait
sans migration.

### Pourquoi je ne le propose pas maintenant

Trois faits, vérifiés :

1. **Pas de 1.0.** 119 versions publiées et la dernière est une `0.8.0-pre.3`.
   L'amont recommande lui-même de « garder des sauvegardes indépendantes »
   jusqu'à la 1.0, et reconnaît que la compatibilité n'est pas à 100 %.
2. **La recherche sauterait.** Turso n'implémente pas FTS5 : sa recherche
   plein texte passe par `tantivy` et reste marquée expérimentale. Nous
   venons de livrer la recherche sur FTS5 avec `contentless_delete` — elle
   serait à réécrire, et sans l'astuce qui évite de dupliquer l'historique.
3. **Son chiffrement est expérimental et non audité.** Or c'est précisément
   ce qu'on veut obtenir.

Le gain serait réel — 269 000 lignes de C en moins, et la MSRV libérée (elle est
à 1.95 uniquement à cause du build script de `libsqlite3-sys`). Mais il se paie
en fiabilité sur **l'historique de messages de l'utilisateur**, pour une
application dont c'est la donnée centrale.

**Proposition : recontrôler à la 1.0.** Le format de fichier étant compatible,
la bascule restera possible plus tard sans migration de données — le coût
d'attendre est donc faible, et c'est ce qui rend l'attente raisonnable.

### Chiffrement au repos : trois voies

| Voie | Ce que ça chiffre | Effet sur la recherche | Coût |
|---|---|---|---|
| **SQLCipher** (`bundled-sqlcipher`) | **tout le fichier**, page par page, de façon transparente | **aucun** : FTS5 continue de fonctionner | macOS utilise CommonCrypto (propre) ; Linux et Windows exigent OpenSSL, ou la feature `vendored-openssl` qui embarque tout OpenSSL |
| Chiffrement applicatif du contenu | seulement le champ `content` | **détruit la recherche** : on n'indexe pas du texte chiffré | faible, mais on perd ce qu'on vient de livrer |
| Chiffrement Turso | tout le fichier | FTS5 absent de toute façon | expérimental et non audité |

**Proposition : SQLCipher**, seule voie qui chiffre l'ensemble *sans* sacrifier
la recherche. Points à traiter avant de l'activer :

- **migration** des bases existantes : SQLCipher ne lit pas un fichier en clair,
  il faut exporter/réimporter au premier lancement (`sqlcipher_export`) ;
- **origine de la clé** : dériver de `ABCOM_PASSPHRASE` par KDF lierait le
  chiffrement du disque à la passphrase de salon, deux choses différentes. Mieux
  vaut une clé propre, stockée avec `identity.key` (déjà en 0600 / ACL
  restreinte) — ce qui protège des autres comptes de la machine, pas d'un accès
  physique. À écrire noir sur blanc dans le modèle de menace, sous peine de
  promettre plus que ce qui est livré ;
- **OpenSSL sur Linux/Windows** : c'est le vrai coût, à mesurer sur la CI
  multi-OS avant de s'engager.

---

## Ordre d'attaque conseillé

**Immédiat, effort trivial, gain net :**

1. **D1 + D31** `set_nodelay(true)` et réglages de socket — latence de chaque message.
2. **D2 + D27** rejeter sur les dimensions **avant** de décoder — sécurité.
3. **D3** `PowerPreference::LowPower` — batterie.
4. **D10 + D9 + D22** index `(to_user, id)`, pragmas et `PRAGMA optimize` — perf de tout l'historique.
5. **D21** `reduce_texture_memory` — un booléen, libère les copies CPU des images.
6. **D17** feature zip, **D28** taille minimale de fenêtre.

**Ensuite, à vraie valeur :**

6. **D20 + D13** : libellés AccessKit puis `egui_kittest`. Dans cet ordre, parce
   que le second interroge l'arbre produit par le premier.
7. **D11 + D24** recherche FTS5 en mode contentless-delete — le manque fonctionnel le plus visible, sans dupliquer l'historique sur le disque.
8. **D4, D5, D6, D7** : appliqués. **D12** reste écarté (cf. §8).

**À arbitrer :**

9. **D8** virtualisation du fil : `show_viewport` plutôt que `show_rows`, nos
   lignes n'ayant pas une hauteur uniforme. Vrai gain, vraie complexité.
10. **D15** prologue Noise, **D16** API tokio, **D14/D18** spans et journal
    fichier, **D23** tables `STRICT` à la prochaine migration de schéma.
11. **D29** à vérifier lors du premier vrai test sous Windows, en même temps que
    D7 et D12 — les trois se tiennent.

---

*Tout constat de ce document a été vérifié soit dans les sources vendorées de la
dépendance (chemin et ligne cités), soit par `grep` sur `src/`. Les rares
suppositions non vérifiables sans mesure — le gain réel de `mmap_size`, le coût
de `show_viewport` — sont signalées comme telles.*
