# 08 — Historique du projet et audits

Ce document retrace les phases du projet, les audits menés et leurs résultats mesurés. Les documents de travail d'origine (audits complets, plans d'exécution détaillés, ADR) sont conservés tels quels dans [old/](../old/) — s'y référer pour le détail constat par constat.

## Chronologie

### Avril 2026 — Première version

Chat LAN fonctionnel mais rudimentaire : découverte UDP, une connexion TCP par paquet, JSON en clair, persistance par réécriture complète de `messages.json`, cinq modules monolithiques, aucun test. Les documents de cette époque (architecture, ADR sur le choix Rust et du P2P) sont dans `old/docs/`.

### Mai–juin 2026 — Fonctionnalités et refonte modulaire

Développement des fonctionnalités de messagerie : architecture découpée en sous-modules (`app/`, `ui/`, `network/`, `message/`), Markdown, picker emoji et shortcodes, interface FR/EN, accusés de réception puis correctif de leur logique, transfert de fichiers avec acceptation, puis refonte des médias en streaming par tranches (fichiers > 1 Go, vignettes, visionneuse), sélecteur Klipy (GIF/mèmes/stickers). Corrections marquantes : boucle infinie CPU sur `has_unread`, son de notification qui bloquait le thread UI.

### 23–27 juin 2026 — Audit technique et solidification

Premier audit formel (`old/docs/AUDIT.md`), suivi de quatre sprints :

| Sprint | Contenu |
|---|---|
| Robustesse réseau | Timeout de lecture 5 s + limite de taille des paquets ; hash de message FNV-1a déterministe (remplace `DefaultHasher`, non stable entre processus) avec timestamp dans la clé |
| Tests réseau | Couverture de `server`, `sender`, `discovery`, `transfer` sur de vraies sockets |
| CI/CD | Workflows GitHub Actions `dev` et `main`, hook pre-commit partagé |
| Tests transfert | Round-trip complet, refus, en-têtes invalides |

Bilan : 130 → 164 tests, robustesse réseau corrigée, CI opérationnelle. Verdict de l'audit : cœur fonctionnel atteint, prêt pour usage réel sur LAN de confiance.

### 4 juillet 2026 — Audit performance et passe d'optimisation

L'application, destinée à tourner en permanence, consommait beaucoup trop au repos. Baseline mesurée : **443 Mo de RAM, 22 % de CPU, 9,7 % de GPU, 15 threads**.

L'audit (`old/docs/06-audit-performance.md`) a identifié les causes structurelles : re-rendu et re-parsing de tout le fil à chaque frame avec repaint permanent, réécriture du JSON complet à chaque message dans le thread UI, caches d'images jamais purgés, dossier `media/` sans limite, réglages de build absents. Le plan d'exécution (`old/docs/07-plan-optimisation.md`) a été réalisé en trois phases :

- **A — extinction au repos** : profil release optimisé, réveil de l'UI par événement au lieu du polling à 500 ms, throttle de la progression des transferts, thread audio pérenne ;
- **B — coût par frame indépendant de l'historique** : compteurs de génération, caches dérivés du fil et de la barre latérale, Markdown memoïsé, fenêtrage du fil avec pagination au scroll ;
- **C — bornage mémoire et disque** : gel des GIF hors écran, downscale des images avant texture, LRU de textures, GC du dossier `media/` (orphelins + plafond 2 Go), persistance débouncée hors thread UI.

Résultats mesurés le jour même : **CPU ~0,6 %, RSS ~156 Mo, 8 threads**, plus aucune écriture disque par message.

Décisions produit actées à cette occasion : objectifs chiffrés (~0 % CPU au repos, RSS < 150 Mo), migration vers SQLite, GIF affichés en HD dans le fil (le bornage passe par le gel hors écran, pas par la résolution), pagination sans bouton façon Discord, et — aucune release n'étant publiée — liberté de casser le protocole réseau.

### 4 juillet 2026 (suite) — Seconde passe et sécurisation du transport

La seconde passe (`old/docs/08-seconde-passe-et-securisation.md`) a traité les finitions (découverte silencieuse hors changement d'état, générations contenu/présence scindées, `resvg` optionnel) puis livré les deux gros chantiers :

- **SQLite** : historique complet en base, thread d'écriture dédié, migration automatique des JSON (vérifiée sur 401 messages réels), pagination depuis la base, avatars en BLOB — fin du plafond de 500 messages et du bug des avatars > 64 Ko rejetés par la limite de paquet.
- **Chiffrement du transport** : identité X25519 par machine, connexions TCP persistantes par pair, handshake Noise XX (chat **et** médias), TOFU avec refus sur clé changée, passphrase de salon optionnelle (`XXpsk3`). TLS et une simple PSK avaient été évalués et écartés (PKI inadaptée au P2P ; pas d'identité ni de forward secrecy).

Mesures après implémentation : **CPU ~0,2 %, RSS ~155 Mo, binaire 10,8 Mo, 216 tests**. Vérifié en conditions réelles : un client en clair est rejeté au handshake ; sans la bonne passphrase, aucun handshake n'aboutit.

L'atlas d'emojis, envisagé, a été abandonné après analyse : le gel du premier frame venait du décodage des 323 PNG (désormais fait dans un thread), pas du nombre de textures ; l'atlas n'aurait rien gagné en mémoire.

### 4 juillet 2026 (fin) — Runner résident

Spécification puis implémentation du mode « toujours ouvert » (`old/docs/09-runner-resident.md`) : fermer = se replier dans le tray (un seul processus, le daemon séparé a été écarté), zéro rendu fenêtre cachée avec purge des textures, notifications système natives réglables, badge non-lus, autostart activé par défaut en release. Choix de fiabilité : la politique de rendu ne s'appuie que sur des signaux sûrs (caché / minimisé / focus) — la détection d'occlusion, non fiable, est hors périmètre. Ajusté ensuite : retrait du Dock macOS quand la fenêtre est cachée ; la pause des GIF hors focus, d'abord livrée, a été retirée.

### 5 juillet 2026 — Phase 10 : groupes

Refonte complète des salons (`old/docs/10_groupe.md`, désormais consolidé dans [05 — Fonctionnalités](05-fonctionnalites.md)) : messages réservés aux membres, gestion des membres (ajout, exclusion, départ avec succession du propriétaire, suppression), rattrapage des absents par le propriétaire, politique d'historique « quitter, c'est partir », compteurs non-lus et sourdine par salon.

Corrections apportées par cette phase — l'ancienne implémentation diffusait les messages de salon à **tout le réseau** et le fil des salons restait vide :

| Symptôme | Cause |
|---|---|
| Gel de l'application à la création d'un groupe | Auto-deadlock : le `MutexGuard` du scrutinee d'un `if let` vit jusqu'à la fin du bloc (édition 2021), et le corps reprenait le même verrou |
| Messages de salon envoyés à tous les pairs | Pas de branche « groupe » à l'envoi → broadcast par défaut |
| Fil de salon vide | Filtrage de conversation sans cas `#nom` |
| Groupes annoncés à des non-membres | `Create` diffusé à tout le réseau |
| ACK privés émis pour des messages de salon | `to_user.is_some()` confondait privé et salon |

Bilan : 226 tests verts.

## Récapitulatif des mesures

| Axe | Avril (baseline) | Après optimisation + sécurisation |
|---|---|---|
| CPU au repos | 22 % | ~0,2 % |
| RAM (RSS) | 443 Mo | ~155 Mo |
| Threads | 15 | 8 |
| Binaire release | — | ~11 Mo |
| Tests | 0 | 226 |
| Transport | JSON en clair, 1 connexion/paquet | Noise XX, connexions persistantes |
| Persistance | JSON réécrit à chaque message | SQLite WAL, écritures hors thread UI |

Reste à mesurer en usage réel : GPU au repos après la passe finale, RSS après navigation GIF intensive, débit d'un transfert > 1 Go (voir [09 — Limites et pistes](09-limites-et-pistes.md)).
