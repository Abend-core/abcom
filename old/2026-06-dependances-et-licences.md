# Audit des dépendances

Projet **abcom** — messagerie P2P LAN (licence **MIT**).
Dernière mise à jour : 2026-06-23.

- Dépendances **directes** : 15 (déclarées dans [`Cargo.toml`](../Cargo.toml)).
- Total du graphe résolu (transitif) : **544** paquets.
- Génération : `cargo metadata`, versions issues de [`Cargo.lock`](../Cargo.lock).

## Dépendances directes

| Crate | Version | Rôle | Licence |
|-------|---------|------|---------|
| `tokio` | 1.52 | Runtime asynchrone (réseau TCP/UDP, tâches) | MIT |
| `serde` | 1.0 | (Dé)sérialisation des messages et de l'état | MIT OR Apache-2.0 |
| `serde_json` | 1.0 | Format JSON (réseau + persistance disque) | MIT OR Apache-2.0 |
| `eframe` | 0.31 | Cadre d'application egui (fenêtre native) | MIT OR Apache-2.0 |
| `egui` | 0.31 | Interface graphique immédiate | MIT OR Apache-2.0 |
| `chrono` | 0.4 | Horodatage, dates locales, séparateurs de jour | MIT OR Apache-2.0 |
| `anyhow` | 1.0 | Gestion d'erreurs ergonomique | MIT OR Apache-2.0 |
| `dirs` | 5.0 | Répertoire de données par plateforme | MIT OR Apache-2.0 |
| `image` | 0.25 | Décodage/encodage PNG/JPEG, normalisation des avatars | MIT OR Apache-2.0 |
| `rfd` | 0.15 | Sélecteurs de fichiers natifs (avatar, transferts) | MIT |
| `walkdir` | 2.5 | Parcours récursif de dossiers (transferts) | Unlicense OR MIT |
| `socket2` | 0.5 | Options socket bas niveau (broadcast/réuse de port) | MIT OR Apache-2.0 |
| `resvg` | 0.47 | Rasterisation des avatars **SVG** | MPL-2.0 |
| `rodio` | 0.19 | Lecture des sons de notification (cible non-Windows / Windows) | MIT OR Apache-2.0 |
| `image` (assets) | — | voir aussi police embarquée ci-dessous | — |

`rodio` est déclaré par plateforme dans `Cargo.toml` (feature `symphonia-wav`
sous Windows). `resvg` ré-exporte `usvg` (0.47) et `tiny-skia` (0.11) utilisés
directement dans [`src/ui/avatar.rs`](../src/ui/avatar.rs).

## Dépendances transitives notables

| Crate | Via | Rôle | Licence |
|-------|-----|------|---------|
| `usvg` | resvg | Analyse de l'arbre SVG | MPL-2.0 |
| `tiny-skia` | resvg | Rendu 2D logiciel | BSD-3-Clause |
| `rustybuzz` | resvg | Façonnage de texte SVG | MIT |
| `glow` | eframe | Liaison OpenGL (rendu) | MIT OR Apache-2.0 |
| `winit` | eframe | Fenêtrage multiplateforme | Apache-2.0 |
| `symphonia` | rodio | Décodage audio | MPL-2.0 |

## Synthèse des licences (graphe complet)

Répartition des 544 paquets :

- ~96 % sous licences permissives **MIT** et/ou **Apache-2.0** (et variantes
  `MIT/Apache-2.0`, `Zlib`, `BSD`, `Unlicense`, `ISC`, `BSL-1.0`, `CC0`).
- **18** paquets `Unicode-3.0` (tables Unicode — permissif).
- **7** paquets **MPL-2.0** (famille `resvg`/`usvg`, `symphonia`).

### Point d'attention : MPL-2.0

La MPL-2.0 est un copyleft **au niveau du fichier** : aucune obligation de
relicencier abcom (qui reste MIT) ni de publier notre code. La seule contrainte
est que, si l'on **modifie** un fichier source d'un crate MPL-2.0, ce fichier
modifié doit rester sous MPL-2.0. Nous ne modifions aucun de ces crates : nous
les consommons tels quels. **Aucune action requise.**

Aucune dépendance sous licence fortement copyleft (GPL/AGPL) n'est présente dans
le graphe de compilation.

## Ressources embarquées (hors crates)

| Ressource | Emplacement | Licence |
|-----------|-------------|---------|
| Police **Inter Bold** | [`assets/fonts/Inter-Bold.ttf`](../assets/fonts/Inter-Bold.ttf) | SIL **OFL-1.1** ([licence](../assets/fonts/Inter-OFL.txt)) |
| Jeu d'**emojis** | [`assets/emoji/`](../assets/emoji/) | voir la source du jeu (Twemoji-like) |

L'OFL-1.1 autorise l'intégration et la redistribution de la police, y compris
embarquée dans le binaire, tant que le fichier de licence accompagne le projet.

## Maintenance & sécurité

- Vérifier les vulnérabilités connues : `cargo audit` (nécessite
  `cargo install cargo-audit`).
- Lister les licences en détail : `cargo install cargo-license && cargo license`.
- Mettre à jour dans les bornes semver : `cargo update`.
- `Cargo.lock` est versionné : les builds sont reproductibles.
