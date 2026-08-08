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
> Compagnon de [`AUDIT.md`](AUDIT.md) (dette de code) : ce document-ci ne parle
> que de l'écart entre ce que nos dépendances offrent et ce qu'on en tire.

## Verdict en une ligne

Les versions sont toutes au dernier stable. **L'écart n'est pas de fraîcheur,
il est d'usage** : nous réimplémentons à la main plusieurs mécanismes que nos
dépendances fournissent depuis longtemps, et nous laissons de côté des réglages
qui coûtent aujourd'hui de la latence, de la batterie et de la sécurité.

## Tableau de bord

| # | Constat | Impact | Effort |
|---|---------|--------|--------|
| D1 | `TCP_NODELAY` jamais posé sur les sockets de chat | 🔴 latence perçue | trivial |
| D2 | Décodage d'images réseau sans borne de dimensions | 🔴 sécurité | trivial |
| D3 | `PowerPreference::HighPerformance` par défaut (GPU dédié pour une UI 2D) | 🟠 batterie | trivial |
| D4 | `ThemePreference` d'egui réimplémenté à la main | 🟠 dette | faible |
| D5 | `egui::Modal` (dispo depuis 0.30) inutilisé — nos modales sont des `Window` | 🟠 UX | faible |
| D6 | Aucun raccourci via `KeyboardShortcut`/`consume_shortcut` | 🟠 UX | moyen |
| D7 | Glisser-déposer de fichiers dans la fenêtre absent | 🟠 UX | faible |
| D8 | `ScrollArea::show_rows` inutilisé — fenêtrage du fil fait main | 🟠 perf | moyen |
| D9 | Pragmas SQLite manquants (`busy_timeout`, `mmap_size`, `cache_size`) | 🟠 perf/robustesse | trivial |
| D10 | Un seul index SQLite, alors qu'on filtre surtout sur `to_user` | 🟠 perf | trivial |
| D11 | FTS5 compilé dans le binaire mais inexploité (aucune recherche) | 🟠 fonctionnalité | moyen |
| D12 | `rfd::AsyncFileDialog` inutilisé — d'où notre bricolage `pending_picker` | 🟠 dette | faible |
| D13 | `egui_kittest` : test d'interaction et de rendu par capture | 🟠 qualité | moyen |
| D14 | `tracing` sans aucun span (`#[instrument]`) | 🟢 diagnostic | faible |
| D15 | `snow::Builder::prologue` inutilisé | 🟢 sécurité | faible |
| D16 | `try_reserve_many` / `recv_many` (tokio) réimplémentés ou ignorés | 🟢 simplicité | faible |
| D17 | Feature `zip` tirant zopfli pour rien | 🟢 poids | trivial |
| D18 | Pas de journal fichier (`tracing-appender`) | 🟢 diagnostic | faible |
| D19 | `notify-rust` : actions dans les notifications | 🟢 UX | faible |
| D20 | Accessibilité AccessKit : aucun `widget_info` sur les boutons peints | 🟢 a11y | moyen |

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

## Ordre d'attaque conseillé

**Immédiat, effort trivial, gain net :**

1. **D1** `set_nodelay(true)` — latence de chaque message.
2. **D2** bornes de dimensions au décodage d'images — sécurité.
3. **D3** `PowerPreference::LowPower` — batterie.
4. **D10** index `(to_user, id)` + **D9** pragmas — perf de tout l'historique.
5. **D17** feature zip.

**Ensuite, à vraie valeur :**

6. **D20 + D13** : libellés AccessKit puis `egui_kittest`. Dans cet ordre, parce
   que le second interroge l'arbre produit par le premier.
7. **D11** recherche FTS5 — le manque fonctionnel le plus visible.
8. **D4, D5, D6, D7, D12** : remplacer nos réimplémentations par les API d'egui
   et rfd. Chacune supprime du code au lieu d'en ajouter.

**À arbitrer :**

9. **D8** virtualisation du fil : `show_viewport` plutôt que `show_rows`, nos
   lignes n'ayant pas une hauteur uniforme. Vrai gain, vraie complexité.
10. **D15** prologue Noise, **D16** API tokio, **D14/D18** spans et journal
    fichier.

---

*Tout constat de ce document a été vérifié soit dans les sources vendorées de la
dépendance (chemin et ligne cités), soit par `grep` sur `src/`. Les rares
suppositions non vérifiables sans mesure — le gain réel de `mmap_size`, le coût
de `show_viewport` — sont signalées comme telles.*
