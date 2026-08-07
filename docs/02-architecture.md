# 02 — Architecture

## Vue d'ensemble

Abcom est un binaire unique. Trois mondes cohabitent dans le processus :

- **le thread principal**, occupé par la boucle egui/eframe (rendu et interactions) ;
- **un runtime tokio à 2 workers**, qui porte toutes les tâches réseau : découverte UDP, serveur TCP, expéditeurs, streaming des médias ;
- **des threads dédiés** pour ce qui ne doit jamais bloquer l'UI : écritures SQLite, lecture des sons, GC du cache disque des médias, décodage des emojis au démarrage.

La communication entre ces mondes passe par des canaux `mpsc` tokio. L'état applicatif (`AppState`) est partagé sous `Arc<Mutex<...>>` entre l'UI et les tâches, mais le rendu par frame n'en dépend plus directement : il lit des caches dérivés reconstruits uniquement quand l'état change (voir plus bas).

```
                    ┌────────────────────────────────────────────┐
                    │              Thread principal              │
                    │   eframe/egui : update() → rendu, saisie   │
                    │   caches dérivés (ChatCache, SidebarCache) │
                    └─────▲──────────────────────────┬───────────┘
        AppEvent (mpsc,   │                          │  NetworkSendRequest (mpsc)
        réveil du repaint)│                          │  + MediaSendJob (mpsc)
                    ┌─────┴──────────────────────────▼───────────┐
                    │           Runtime tokio (2 workers)        │
                    │  discovery UDP · serveur TCP · pool de     │
                    │  connexions chiffrées · expéditeurs ·      │
                    │  serveur/expéditeur médias                 │
                    └─────┬──────────────────────────────────────┘
                          │ StorageCmd (canal)
                    ┌─────▼───────────────┐
                    │ Thread stockage     │  abcom.db (SQLite, WAL)
                    └─────────────────────┘
```

## Carte des modules

| Module | Rôle |
|---|---|
| [lib.rs](../src/lib.rs) | Racine réutilisable du cœur applicatif, utilisée par le binaire et les tests d'intégration externes |
| [main.rs](../src/main.rs) | Amorçage : lecture de `.env`, création des canaux, ouverture du stockage, chargement de l'identité, lancement des tâches puis de l'UI |
| [config.rs](../src/config.rs) | Ports et répertoire de données, dérivés de `ABCOM_INSTANCE` (multi-instances locales) ; clé API Klipy |
| [identity.rs](../src/identity.rs) | Paire X25519 de la machine (`identity.key`, permissions 0600), empreinte BLAKE2s |
| [discovery.rs](../src/discovery.rs) | Annonce UDP périodique (broadcast + multicast), suivi de présence, événements émis uniquement au changement d'état |
| [notify.rs](../src/notify.rs) | `UiSender` : un `mpsc::Sender` couplé au contexte egui — chaque événement relayé réveille le rendu (`request_repaint`) |
| [network/](../src/network/mod.rs) | Transport chiffré : `secure.rs` (handshake Noise, TOFU, PSK), `pool.rs` (connexions persistantes par pair), `server.rs` (réception), `sender.rs` (expéditeurs par type de paquet), `media_stream.rs` (streaming des fichiers par tranches) |
| [message/](../src/message/mod.rs) | Types échangés et événements internes : `ChatMessage`, `NetworkPacket` (enum taggé), `GroupAction`, réactions, accusés, avatars, médias, `AppEvent` |
| [app/](../src/app/mod.rs) | État applicatif : messages et conversations, pairs et alias, groupes, réactions, accusés, transferts, frappe, avatars ; `storage.rs` (SQLite et son thread d'écriture) |
| [ui/](../src/ui/mod.rs) | Interface : `chat_panel` (fil), `sidebar` (pairs et salons), `input_bar` et `composer/` (saisie), `markdown`, pickers emoji/GIF, `group_modal`, `settings`, `snapshot` (caches dérivés), `tray` (icône résidente), `media` (vignettes, visionneuse), `sound` |
| [klipy.rs](../src/klipy.rs) | Client de l'API Klipy (GIF, mèmes, stickers) : recherche avec anti-rebond, pagination |
| [archive.rs](../src/archive.rs) | Compression ZIP d'un dossier pour l'envoyer comme un fichier |
| [autostart.rs](../src/autostart.rs) | Lancement à l'ouverture de session (Launch Agent, clé Run, `.desktop`) |
| [emoji_registry.rs](../src/emoji_registry.rs) | 323 emojis PNG embarqués dans le binaire, décodés dans un thread au démarrage |
| [tests/](../src/tests/) | 288 tests unitaires : un fichier par module testé, réseau testé sur de vraies sockets |
| [tests/p2p_e2e.rs](../tests/p2p_e2e.rs) | Scénario d'intégration headless entre deux pairs authentifiés |

## Flux d'un message reçu

1. Le serveur TCP accepte la connexion (ou réutilise la session Noise existante) et déchiffre une trame.
2. Le JSON est désérialisé en `NetworkPacket`, converti en `AppEvent` et poussé dans le canal vers l'UI. L'envoi passe par `notify::UiSender`, qui appelle `request_repaint()` : l'UI se réveille immédiatement, il n'y a pas de polling.
3. Au `update()` suivant, `process_events` dépile l'événement, met à jour `AppState`, incrémente le compteur de génération et envoie un `StorageCmd` au thread de stockage (l'insertion SQLite se fait hors du thread UI).
4. La frame suivante détecte le changement de génération, reconstruit les caches dérivés concernés et affiche le message.

## Rendu : réveil par événement et caches dérivés

egui est un framework en mode immédiat : tout ce qui est affiché est reconstruit à chaque frame. Deux mécanismes évitent que cela coûte quoi que ce soit au repos.

**Pas de frame sans raison.** L'UI ne programme aucun repaint périodique court : elle est réveillée par les événements (réseau, tray, stockage) via `notify.rs`, avec un simple repli lent pour les tâches d'entretien. Fenêtre cachée dans le tray, `update()` traite les événements et ressort sans rien construire — zéro rendu, zéro repaint programmé.

**Pas de recalcul sans changement.** `AppState` maintient deux compteurs de génération : contenu (messages, réactions, accusés, avatars, alias, groupes) et présence (pairs en ligne, frappe). Les caches de [ui/snapshot.rs](../src/ui/snapshot.rs) s'y adossent :

- `ChatCache` — les lignes du fil, pré-calculées : Markdown parsé (memoïsé par hash de message), regroupements par auteur et par jour, citations résolues, accusés, réactions. Reconstruit uniquement quand la génération de contenu, la conversation ou le jour change. Le rendu d'une frame ne prend aucun verrou sur `AppState`.
- `SidebarCache` — pairs, alias, salons et compteurs non-lus ; dépend des deux générations.

**Fenêtrage du fil.** Seuls les ~100 derniers messages sont rendus ; remonter près du haut charge les 100 précédents depuis SQLite (pagination keyset), avec compensation de l'offset de scroll pour éviter tout saut visuel — le comportement de Discord. L'historique complet vit en base, la RAM ne contient que la fenêtre affichée.

**Textures bornées.** Les images reçues sont réduites à 1024 px maximum avant création de texture (la pleine résolution est réservée à la visionneuse, libérée à sa fermeture) ; le cache de textures est un LRU de 32 entrées. Les GIF hors écran ne sont pas émis comme widgets (leur emplacement garde sa hauteur) : ils ne décodent rien et ne forcent aucun repaint ; leurs frames sont libérées (`forget_image`) quand ils sortent de la fenêtre chargée ou à la fermeture du picker.

## Cycle de vie de la fenêtre (mode résident)

```
Croix / Cmd-W ──► fermeture interceptée ──► fenêtre cachée + purge des textures
                                            (macOS : l'icône Dock disparaît)
Tray ▸ Ouvrir ────► fenêtre visible + focus + invalidation des caches de rendu
Tray ▸ Quitter ───► fermeture réelle ──► flush du stockage (on_exit)
```

Fenêtre cachée, l'état et la base restent tenus à jour au fil des événements réseau ; les messages déclenchent une notification système native et le badge non-lus sur l'icône du tray. La réouverture n'est donc qu'une resynchronisation des caches de rendu. Sur Linux, si aucun tray n'est disponible (shell sans StatusNotifier), la croix quitte l'application comme avant — repli sûr.

## Empreinte au repos

Valeurs mesurées en build release sur macOS après la passe d'optimisation de juillet 2026 (détail : [08 — Historique et audits](08-historique-et-audits.md)) :

| Axe | Avant | Après |
|---|---|---|
| CPU au repos | 22 % | ~0,2 % |
| RAM (RSS) | 443 Mo | ~155 Mo |
| Threads | 15 | 8 |
| Binaire release | — | ~11 Mo (LTO thin, strip, `panic=abort`) |
| Écriture disque par message | réécriture JSON complète dans le thread UI | un INSERT SQLite hors thread UI |
| Disque `media/` | croissance illimitée | orphelins purgés, plafond 2 Go |
