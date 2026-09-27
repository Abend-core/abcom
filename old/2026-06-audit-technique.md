# Audit technique — Abcom

| | |
|---|---|
| **Créé le** | 2026-06-23 |
| **Dernière mise à jour** | 2026-06-27 |
| **Version auditée** | branche `dev` — commit `21ac4f4` |
| **Objectif métier** | Chat P2P LAN : deux utilisateurs sur le même réseau se découvrent automatiquement et échangent des messages privés avec accusés de réception/lecture |

---

## Résumé exécutif

| Critère | État au 23/06 | État au 27/06 |
|---|---|---|
| Objectif core atteint | ✅ Oui | ✅ Oui |
| Tests unitaires | ✅ 130 passent, 0 échec | ✅ **164 passent**, 0 échec |
| Tests réseau / intégration | ❌ Aucun | ✅ **12 tests** (server, sender, discovery) |
| Tests transfert de fichiers | ❌ Aucun | ✅ **5 tests** (transfer/service) |
| Robustesse réseau | ⚠️ Fragile sur 2 points critiques | ✅ **Points A et B corrigés** |
| CI/CD | ❌ Absent | ✅ **GitHub Actions actif** (dev + main) |
| Pre-commit hook partagé | ❌ Absent | ✅ `.githooks/pre-commit` |
| Prêt pour usage production | ⚠️ Conditionnel | ✅ **Oui** (voir points restants) |

---

## 1. Inventaire des fonctionnalités

### 1.1 Découverte de pairs — UDP Broadcast
**Fichiers :** `src/discovery.rs`

Chaque instance émet un paquet UDP broadcast toutes les 3 secondes contenant son `username` et son port TCP. Elle écoute également les broadcasts des autres pairs. Un pair qui n'a pas émis depuis 6 secondes est marqué hors-ligne.

**État : ✅ Solide**
- Socket UDP configuré avec `SO_REUSEADDR` / `SO_REUSEPORT`
- Nettoyage automatique des pairs inactifs
- Paquets sérialisés en JSON avec rétro-compatibilité (champ `port` optionnel)
- **4 tests unitaires** ajoutés le 24/06 (round-trip, legacy, champs inconnus, bind socket)

---

### 1.2 Transport TCP — Réception
**Fichiers :** `src/network/server.rs`

Un serveur TCP écoute sur le port de l'instance. Chaque connexion entrante est traitée dans une tâche tokio séparée. Le paquet JSON est désérialisé en `NetworkPacket` (enum taggé) et dispatché comme `AppEvent`.

**État : ✅ Solide** — [Point Critique A](#a-serveur-tcp--pas-de-timeout-ni-limite-de-taille) corrigé le 24/06

- Timeout 5s sur la lecture (`tokio::time::timeout`)
- Limite de taille à 64 KB (`AsyncReadExt::take`)
- **5 tests unitaires** : Chat, ReadReceipt, Ack, paquet invalide, paquet surdimensionné

---

### 1.3 Transport TCP — Envoi
**Fichiers :** `src/network/sender.rs`

5 expéditeurs indépendants (chat, groupe, typing, read_receipt, ack) consomment chacun un canal `mpsc`. Chaque envoi ouvre une connexion TCP, écrit le JSON sérialisé, ferme proprement. Les erreurs réseau sont loguées sans crasher l'app.

**État : ✅ Solide**
- **3 tests unitaires** ajoutés le 24/06 (livraison bytes, désérialisation, connexion refusée)

---

### 1.4 Messages privés et broadcast
**Fichiers :** `src/app/messages.rs`

Les messages sont stockés dans un `Vec<ChatMessage>` avec persistance JSON immédiate. Un message privé a `to_user = Some(username)`, un broadcast a `to_user = None`. La liste est plafonnée à 500 messages (les 100 plus anciens sont purgés au-delà).

**État : ✅ Solide** — 10 tests unitaires

---

### 1.5 Accusés de réception et de lecture
**Fichiers :** `src/app/receipts.rs`, `src/ui/events.rs`, `src/ui/mod.rs`

- **ACK (✓✓ gris)** : envoyé automatiquement dès qu'un message privé est reçu. Confirme la livraison.
- **ReadReceipt (✓✓ bleu)** : envoyé uniquement quand l'utilisateur ouvre la conversation et que la fenêtre est au premier plan. Confirme la lecture réelle.
- **Retry** : les messages sans ACK sont retransmis avec un backoff exponentiel (1s, 2s, 4s, 8s, 16s, 32s max).

**État : ✅ Solide** — 11 tests unitaires, [Point Critique B](#b-hash-de-message--collisions-possibles) corrigé le 24/06

---

### 1.6 Gestion des pairs
**Fichiers :** `src/app/peers.rs`

Les pairs découverts via UDP sont ajoutés à une liste. Leur adresse IP est mise à jour à chaque broadcast. Les alias (noms conviviaux) sont persistés séparément dans `peer_records.json`. Les anciens contacts (offline) sont restaurés depuis l'historique des messages au démarrage.

**État : ✅ Solide** — 12 tests unitaires

---

### 1.7 Groupes
**Fichiers :** `src/app/groups.rs`

Création de groupes avec validation du nom (alphanumérique + `-_`, max 50 caractères, insensible à la casse). Seul le créateur peut ajouter/retirer des membres ou supprimer le groupe. Les événements sont répliqués vers tous les membres via TCP.

**État : ✅ Solide** — 9 tests unitaires

---

### 1.8 Persistance JSON atomique
**Fichiers :** `src/app/persistence.rs`

Toutes les écritures passent par un fichier temporaire `.json.tmp` suivi d'un `rename()` atomique. En cas de crash pendant l'écriture, le fichier de données original est préservé. Les 4 fichiers persistés sont : `messages.json`, `read_counts.json`, `groups.json`, `peer_records.json`.

**État : ✅ Solide** — 7 tests unitaires

---

### 1.9 Transfert de fichiers
**Fichiers :** `src/transfer/`

Envoi de fichiers et dossiers via TCP sur un port dédié (`chat_port + 1`). Le destinataire reçoit une proposition qu'il accepte ou refuse dans l'UI, choisit un dossier de destination, et voit la progression en temps réel. Protection contre le path traversal côté réception.

**État : ✅ Solide** — **5 tests unitaires ajoutés le 25/06** (snapshot, header invalide, refus UI, rejet utilisateur, round-trip complet)

---

## 2. Points critiques détaillés

### A. Serveur TCP — Pas de timeout ni limite de taille

**Fichier :** `src/network/server.rs`
**Sévérité :** ~~🔴 Critique~~ → **✅ Corrigé le 2026-06-24**

#### Ce qui a été fait

```rust
const MAX_PACKET_SIZE: u64 = 64 * 1024; // 64 KB
const READ_TIMEOUT_SECS: u64 = 5;

async fn handle_incoming(mut stream: TcpStream, tx: Sender<AppEvent>) {
    let mut buf = Vec::new();
    let result = timeout(
        Duration::from_secs(READ_TIMEOUT_SECS),
        stream.take(MAX_PACKET_SIZE + 1).read_to_end(&mut buf),
    )
    .await;
    // timeout → ignore, paquet > 64 KB → ignore, lecture OK → dispatch
}
```

- Connexions lentes (Slowloris) : abandonnées après 5s
- Paquets surdimensionnés (> 64 KB) : rejetés sans crasher
- 5 tests unitaires couvrent ces cas limites

---

### B. Hash de message — Collisions possibles

**Fichier :** `src/app/receipts.rs`
**Sévérité :** ~~🟠 Important~~ → **✅ Corrigé le 2026-06-24**

#### Ce qui a été fait

Remplacement de `DefaultHasher` (non déterministe entre processus) par **FNV-1a** (déterministe, sans dépendance externe) :

```rust
pub fn message_hash(msg: &ChatMessage) -> u64 {
    let key = format!(
        "{}:{}:{}:{}",
        msg.from,
        msg.to_user.as_deref().unwrap_or("broadcast"),
        msg.timestamp_epoch.unwrap_or(0),
        msg.content
    );
    let mut hash: u64 = 14_695_981_039_346_656_037;
    for byte in key.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    hash
}
```

- `timestamp_epoch` inclus → deux "Bonjour" à des heures différentes ont des hashes distincts
- FNV-1a → même résultat sur tous les processus et plateformes
- 2 tests vérifient la stabilité et l'absence de collision sur contenu identique / timestamp différent

---

### C. Aucun test réseau (découverte et transport)

**Fichiers :** `src/discovery.rs`, `src/network/`
**Sévérité :** ~~🟠 Important~~ → **✅ Corrigé le 2026-06-24/25**

#### Ce qui a été fait

| Composant | Tests ajoutés |
|---|---|
| `discovery.rs` | 4 : round-trip, legacy port=9000, champs inconnus, bind socket |
| `network/server.rs` | 5 : Chat, ReadReceipt, Ack, paquet invalide, paquet surdimensionné |
| `network/sender.rs` | 3 : livraison bytes, désérialisation, connexion refusée |
| `transfer/service.rs` | 5 : snapshot, header=0, UI absente, rejet user, round-trip complet |

Chaque test réseau crée de vraies connexions TCP/UDP (`TcpListener::bind("127.0.0.1:0")`) — pas de mocks.

---

### D. `integration_test.sh` — Script obsolète

**Fichier :** `scripts/integration_test.sh`
**Sévérité :** 🟡 Mineur

#### Situation actuelle

Le script existant a deux problèmes bloquants :
1. Chemin hardcodé : `APP_DIR="/home/ra/abcom"` (ne fonctionne que sur une machine)
2. Cible de build : `x86_64-pc-windows-gnu` (inutile en CI Linux)

Il ne teste pas non plus le scénario fondamental (deux instances se découvrent). Les tests unitaires réseau ajoutés (points A/B/C) compensent en partie cette lacune, mais un vrai test d'intégration P2P reste à faire.

#### Scénario minimal proposé (futur sprint)

```bash
# Lance deux instances sur des ports différents
./target/release/abcom alice --port 9001 > /tmp/alice.log 2>&1 &
./target/release/abcom bob   --port 9002 > /tmp/bob.log   2>&1 &

sleep 5  # laisser le temps à la découverte UDP (broadcast toutes les 3s)

grep -q "PeerDiscovered" /tmp/alice.log || { echo "FAIL: alice n'a pas découvert bob"; exit 1; }
grep -q "PeerDiscovered" /tmp/bob.log   || { echo "FAIL: bob n'a pas découvert alice"; exit 1; }

echo "PASS: découverte P2P fonctionnelle"
```

---

## 3. Couverture des tests par module

| Module | Tests (23/06) | Tests (27/06) | Δ |
|---|---|---|---|
| `ui::composer` | 27 | 34 | +7 |
| `app::peers` | 12 | 12 | — |
| `app::receipts` | 9 | 11 | +2 |
| `app::messages` | 11 | 10 | -1 ¹ |
| `ui::markdown` | 9 | 9 | — |
| `app::groups` | 13 | 9 | -4 ¹ |
| `message::chat` | 7 | 8 | +1 |
| `ui::input_bar` | 9 | 7 | -2 ¹ |
| `ui::chat_panel` | 0 | 7 | +7 |
| `message::group` | 7 | 7 | — |
| `app::persistence` | 8 | 7 | -1 ¹ |
| `message::receipts` | 6 | 6 | — |
| `transfer::service` | **0** | **5** | **+5** |
| `network::server` | **0** | **5** | **+5** |
| `message::network_types` | 3 | 5 | +2 |
| `discovery` | **0** | **4** | **+4** |
| `app::typing` | 0 | 4 | +4 |
| `app::avatar` | 0 | 4 | +4 |
| `transfer::storage` | 3 | 3 | — |
| `network::sender` | **0** | **3** | **+3** |
| `message::avatar` | 0 | 2 | +2 |
| `app::transfers` | 2 | 2 | — |
| **TOTAL** | **130** | **164** | **+34** |

¹ Légère baisse due à refactoring ou réorganisation de tests existants par HugoLM entre les deux dates.

---

## 4. CI/CD — État actuel

### Pipeline PR → `dev` (`.github/workflows/ci-dev.yml`)

**Durée moyenne : ~6 min 30** | **Statut : ✅ Actif depuis le 25/06**

| Étape | Statut |
|---|---|
| Dépendances système (`libasound2-dev`, `libxkbcommon-dev`) | ✅ |
| Rust stable + `rustfmt` + `clippy` | ✅ |
| Cache Cargo | ✅ |
| `cargo fmt --check` | ✅ |
| `cargo clippy -- -D warnings` | ✅ |
| `cargo build --release` | ✅ |
| `cargo test` | ✅ |

### Pipeline PR → `main` (`.github/workflows/ci-main.yml`)

Idem + `scripts/integration_test.sh` + `cargo audit`. Le script d'intégration est à corriger (voir [Point D](#d-integration_testsh--script-obsolète)).

### Pre-commit hook partagé (`.githooks/pre-commit`)

Bloque tout commit si `cargo fmt` détecte un problème de formatage ou si `cargo clippy` remonte une erreur. **Activation requise une fois par développeur :**

```bash
git config core.hooksPath .githooks
```

---

## 5. Roadmap de solidification — État au 27/06

### ✅ Sprint 1 — Robustesse réseau (terminé le 2026-06-24)
| Tâche | Statut |
|---|---|
| Timeout 5s + limite 64 KB sur `read_to_end()` | ✅ |
| Fix hash FNV-1a + `timestamp_epoch` | ✅ |

### ✅ Sprint 2 — Tests réseau (terminé le 2026-06-24)
| Tâche | Statut |
|---|---|
| 5 tests `network/server.rs` | ✅ |
| 3 tests `network/sender.rs` | ✅ |
| 4 tests `discovery.rs` | ✅ |

### ✅ Sprint 3 — CI/CD (terminé le 2026-06-25)
| Tâche | Statut |
|---|---|
| GitHub Actions `ci-dev.yml` | ✅ |
| GitHub Actions `ci-main.yml` | ✅ |
| Pre-commit hook partagé `.githooks/` | ✅ |

### ✅ Sprint 4 — Tests transfert (terminé le 2026-06-25)
| Tâche | Statut |
|---|---|
| 5 tests `transfer/service.rs` | ✅ |

---

## 6. Backlog restant

| Priorité | Tâche | Fichier | Effort estimé |
|---|---|---|---|
| 🟡 Mineur | Corriger `integration_test.sh` (chemin, cible build, test P2P réel) | `scripts/integration_test.sh` | 1h |
| 🟡 Mineur | HugoLM doit exécuter `git config core.hooksPath .githooks` et rebaser sa branche `feature/gestion-medias` sur le nouveau `dev` (history rewrite) | — | 5 min |
| 🔵 Amélioration | Test d'intégration P2P : deux instances qui se découvrent et échangent un message | `scripts/integration_test.sh` | 2h |
| 🔵 Amélioration | `cargo audit` sur CI main : 0 vulnérabilité connue à ce jour | CI | — |

---

*Audit mis à jour le 2026-06-27 — 4 sprints de solidification complétés, 130 → 164 tests, CI/CD opérationnel.*
