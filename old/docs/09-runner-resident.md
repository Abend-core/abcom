> [🏠 Accueil](../README.md) > [🛰 Runner résident](09-runner-resident.md)

> 📅 **Généré le** : 2026-07-04 — spécification validée avec l'utilisateur avant réalisation
> 🔖 **Décisions actées** : un seul processus (fenêtre cachée) · notifications réglables avec aperçu par défaut · autostart activé par défaut en release · GIFs en pause hors focus (approche fiable uniquement)
> 🔄 **À régénérer si** : passage à une architecture daemon+GUI, changement de crate tray/notifications

# Spécification — runner résident (tray) & politique de visibilité

> ✅ **Implémenté le 2026-07-04** (T1–T4 + pause GIFs, 219 tests verts,
> smoke test macOS : tray créé sans erreur, RSS ~153 Mo au lancement).
> **À valider par l'utilisateur** : parcours complet macOS (cacher/rouvrir/
> badge/notifications/autostart), compilation et tray **Windows/Linux**
> (non testables dans l'environnement de dev), packaging release (§9).

## 1. Objectif

Abcom doit être **toujours joignable** : l'app tourne en permanence, reçoit
les messages et notifie même fenêtre fermée, se relance à l'ouverture de
session, et **ne consomme rien quand elle n'est pas affichée**. « Fermer »
la fenêtre ne quitte plus : l'app se replie dans la zone système (barre de
menus macOS, zone de notification Windows, StatusNotifier Linux).

**Architecture retenue : un seul processus.** L'exécutable actuel reste en
vie fenêtre cachée — le réseau chiffré, le stockage SQLite et la découverte
sont déjà indépendants de l'UI. Un daemon séparé (IPC) a été évalué et
écarté : semaines de plomberie pour ~50 Mo de RAM, récupérables autrement
(purge des textures au moment de cacher). La frontière `NetContext`/storage
existante permettra ce découpage plus tard si un client mobile le justifie.

## 2. États de visibilité et politique de rendu

Principe : ne s'appuyer **que sur des signaux fiables**. La détection
d'occlusion (fenêtre recouverte) est non fiable sur les trois OS — elle est
explicitement hors périmètre (c'est la source des problèmes passés).

| État | Signal (fiabilité) | Rendu | Événements réseau |
|---|---|---|---|
| **Cachée** (tray) | on l'a cachée nous-mêmes (certain) | **aucun** : `update()` traite les événements et ressort sans construire d'UI, aucun repaint programmé | traités : état + SQLite à jour, **notification native** |
| **Minimisée** | `viewport().minimized` (fiable Win/mac, WM-dépendant Linux) | comme cachée (UI non construite) | idem |
| **Visible, sans focus** | `focused` (fiable partout) | rendu normal, repli 30 s, **GIFs en pause** | traités + toast interne |
| **Visible, focalisée** | — | comportement actuel (réveil par événement, repli 5 s) | traités + toast |

**Réouverture = resynchro** : l'état étant tenu à jour en permanence, il
suffit d'invalider les caches de rendu (`ChatCache`, textures) et de
recharger paresseusement. Concrètement au `show` : `Visible(true)` + focus,
`chat_cache.invalidate()`, relance du décodeur d'emojis, remise à zéro des
non-lus de la conversation affichée.

**Purge mémoire au `hide`** : textures médias (LRU), avatars, visionneuse,
textures emoji (re-décodées en arrière-plan au `show`), `forget_image` des
GIFs — objectif RSS caché nettement sous la valeur fenêtre ouverte.

## 3. Cycle de vie

```
Croix / Cmd-W ──► close_requested intercepté ──► CancelClose
                                              ──► Visible(false) + purge ── état « cachée »
Tray ▸ Ouvrir / clic icône (Win) ─────────────► Visible(true) + Focus + resync
Tray ▸ Quitter ───────────────────────────────► really_quit = true ──► Close réel
                                                   └─► on_exit : flush SQLite (existant)
```

- L'interception utilise `viewport().close_requested()` +
  `ViewportCommand::CancelClose` (egui 0.31). Un booléen `really_quit`
  laisse passer la fermeture réelle.
- macOS : l'icône Dock reste visible en v1 (politique d'activation
  `Regular`). Le retrait du Dock quand caché (`Accessory`) est une
  amélioration ultérieure (nécessite objc à chaud).

## 4. Icône résidente (crate `tray-icon`, équipe Tauri)

- **Création** : sur macOS l'icône doit être créée sur le thread principal,
  event loop démarrée → création paresseuse au premier `update()`, handle
  conservé dans `AbcomApp` (drop = disparition de l'icône).
- **Menu** (`muda`, embarqué) : « Ouvrir Abcom » · séparateur · « Quitter ».
- **Clic** : macOS = le clic ouvre le menu (`show_menu_on_left_click`) ;
  Windows = clic gauche ouvre la fenêtre, droit ouvre le menu ; Linux =
  selon le shell (StatusNotifier).
- **Réveil sans rendu** : quand la fenêtre est cachée, l'UI ne tourne pas —
  les événements tray/menu arrivent par callbacks
  (`TrayIconEvent::set_event_handler`, `MenuEvent::set_event_handler`) qui
  appellent `ctx.request_repaint()` via le `UiContext` partagé (même
  mécanisme que le réveil réseau). L'`update()` suivant dépile les
  événements tray et agit.
- **Badge non-lus** : deux icônes RGBA générées au démarrage depuis
  `app_icon.png` (32×32, avec/sans pastille rouge) ; `set_icon` au
  changement de `has_unread`.
- **Linux** : nécessite un shell StatusNotifier/AppIndicator (GNOME avec
  extension, KDE, XFCE ok). Sans tray, l'app reste utilisable : la fenêtre
  ne se cache que si l'icône tray a pu être créée — sinon la croix quitte
  comme avant (repli sûr).

## 5. Notifications natives (crate `notify-rust`)

- Émises **uniquement fenêtre cachée/minimisée** (sinon : toast interne
  actuel). Envoyées depuis un thread détaché (l'appel peut bloquer).
- Contenu **réglable** (Paramètres, persisté en kv) : « aperçu » (défaut) =
  `Alice : début du message…` ; « discret » = `Nouveau message de Alice`.
- Plateformes : Linux D-Bus (XDG), macOS NSUserNotification, Windows WinRT.
- **Limites assumées v1** : pas de clic-pour-ouvrir fiable multi-OS (on
  ouvre via le tray) ; sur macOS, un binaire nu est attribué au terminal —
  l'attribution propre exige le bundle `.app` (cf. §7). Le bip rodio est
  conservé fenêtre visible, remplacé par la notification native cachée.

## 6. Autostart (crate `auto-launch`)

- macOS : Launch Agent ; Windows : clé de registre Run ; Linux :
  `~/.config/autostart/*.desktop`.
- **Défaut activé en release** : au premier lancement d'un build release
  (`!debug_assertions`) et si la préférence kv est absente → activation +
  persistance. Jamais en debug/`cargo run`. Interrupteur dans Paramètres →
  Général (lit/écrit la préférence kv **et** l'état système).

## 7. Préférences persistées (table `kv` SQLite, déjà en place)

| Clé | Valeurs | Défaut |
|---|---|---|
| `notif_preview` | `1`/`0` | `1` (aperçu) |
| `autostart` | `1`/`0` | `1` posé au 1er lancement release |

Nouveau `StorageCmd::SetKv` + lecture dans `LoadedState`.

## 8. GIFs en pause hors focus

Impossible de « geler » proprement une image animée egui (le loader avance
avec l'horloge) ; la seule pause fiable est de **ne pas émettre le widget
animé**. Fenêtre sans focus : l'emplacement du GIF (fil et picker) affiche
un cadre statique estompé avec « ▶ GIF » — reprise instantanée de
l'animation au focus. C'est le même mécanisme, éprouvé, que le gel hors
écran. Compromis visuel assumé (validé : « en pause si fiable »).

## 9. Packaging release (documentation, hors code de cette passe)

Pour des notifications macOS correctement attribuées et un autostart
propre : bundle **`.app`** (cargo-bundle/cargo-packager) avec identifiant
`com.abend.abcom` ; Windows : exécutable installé (l'autostart pointe vers
le chemin d'installation) ; Linux : fichier `.desktop` + icône (les scripts
`contrib/`/`scripts/` existants s'y prêtent). À traiter au moment de la
première release.

## 10. Découpage & critères d'acceptation

| Étape | Livrable | Critère |
|---|---|---|
| T1 | Croix = cacher (si tray dispo), politique de visibilité, purge/resync | caché : 0 repaint (CPU/GPU ~0, RSS en baisse) ; réouverture : fil à jour sans artefact |
| T2 | Tray + menu + badge | ouvrir/quitter depuis le tray sur les 3 OS ; badge sur non-lu |
| T3 | Notifications natives réglables | message reçu fenêtre cachée → notification système ; réglage aperçu/discret respecté |
| T4 | Autostart | release : activé au 1er lancement, toggle fonctionnel ; debug : jamais |
| — | GIFs pause hors focus | fenêtre sans focus avec GIF visible : 0 repaint continu |

**À tester par l'utilisateur** (je ne peux pas le faire d'ici) : le tray
sous Windows et Linux (compilation croisée non testable dans cet
environnement), l'attribution des notifications macOS, l'autostart réel.
