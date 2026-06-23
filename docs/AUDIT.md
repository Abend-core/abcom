# Audit technique — Abcom

| | |
|---|---|
| **Créé le** | 2026-06-23 |
| **Dernière mise à jour** | 2026-06-23 |
| **Version auditée** | branche `dev` — commit `183d94d` |
| **Objectif métier** | Chat P2P LAN : deux utilisateurs sur le même réseau se découvrent automatiquement et échangent des messages privés avec accusés de réception/lecture |

---

## Résumé exécutif

| Critère | État |
|---|---|
| Objectif core atteint | ✅ Oui |
| Tests unitaires | ✅ 130 passent, 0 échec |
| Tests réseau / intégration | ❌ Aucun |
| Robustesse réseau | ⚠️ Fragile sur 2 points critiques |
| Prêt pour usage production | ⚠️ Conditionnel (voir points critiques) |

---

## 1. Inventaire des fonctionnalités

### 1.1 Découverte de pairs — UDP Broadcast
**Fichiers :** `src/discovery.rs`

Chaque instance émet un paquet UDP broadcast toutes les 3 secondes contenant son `username` et son port TCP. Elle écoute également les broadcasts des autres pairs. Un pair qui n'a pas émis depuis 6 secondes est marqué hors-ligne.

**État : ✅ Solide**
- Socket UDP configuré avec `SO_REUSEADDR` / `SO_REUSEPORT`
- Nettoyage automatique des pairs inactifs
- Paquets sérialisés en JSON avec rétro-compatibilité (champ `port` optionnel)

---

### 1.2 Transport TCP — Réception
**Fichiers :** `src/network/server.rs`

Un serveur TCP écoute sur le port de l'instance. Chaque connexion entrante est traitée dans une tâche tokio séparée. Le paquet JSON est désérialisé en `NetworkPacket` (enum taggé) et dispatché comme `AppEvent`.

**État : ⚠️ Fragile** — voir [Point Critique A](#a-serveur-tcp--pas-de-timeout-ni-limite-de-taille)

---

### 1.3 Transport TCP — Envoi
**Fichiers :** `src/network/sender.rs`

5 expéditeurs indépendants (chat, groupe, typing, read_receipt, ack) consomment chacun un canal `mpsc`. Chaque envoi ouvre une connexion TCP, écrit le JSON sérialisé, ferme proprement. Les erreurs réseau sont loguées sans crasher l'app.

**État : ✅ Solide**

---

### 1.4 Messages privés et broadcast
**Fichiers :** `src/app/messages.rs`

Les messages sont stockés dans un `Vec<ChatMessage>` avec persistance JSON immédiate. Un message privé a `to_user = Some(username)`, un broadcast a `to_user = None`. La liste est plafonnée à 500 messages (les 100 plus anciens sont purgés au-delà).

**État : ✅ Solide** — 11 tests unitaires

---

### 1.5 Accusés de réception et de lecture
**Fichiers :** `src/app/receipts.rs`, `src/ui/events.rs`, `src/ui/mod.rs`

- **ACK (✓✓ gris)** : envoyé automatiquement dès qu'un message privé est reçu. Confirme la livraison.
- **ReadReceipt (✓✓ bleu)** : envoyé uniquement quand l'utilisateur ouvre la conversation et que la fenêtre est au premier plan. Confirme la lecture réelle.
- **Retry** : les messages sans ACK sont retransmis avec un backoff exponentiel (1s, 2s, 4s, 8s, 16s, 32s max).

**État : ✅ Solide** — 9 tests unitaires

---

### 1.6 Gestion des pairs
**Fichiers :** `src/app/peers.rs`

Les pairs découverts via UDP sont ajoutés à une liste. Leur adresse IP est mise à jour à chaque broadcast. Les alias (noms conviviaux) sont persistés séparément dans `peer_records.json`. Les anciens contacts (offline) sont restaurés depuis l'historique des messages au démarrage.

**État : ✅ Solide** — 12 tests unitaires

---

### 1.7 Groupes
**Fichiers :** `src/app/groups.rs`

Création de groupes avec validation du nom (alphanumérique + `-_`, max 50 caractères, insensible à la casse). Seul le créateur peut ajouter/retirer des membres ou supprimer le groupe. Les événements sont répliqués vers tous les membres via TCP.

**État : ✅ Solide** — 13 tests unitaires

---

### 1.8 Persistance JSON atomique
**Fichiers :** `src/app/persistence.rs`

Toutes les écritures passent par un fichier temporaire `.json.tmp` suivi d'un `rename()` atomique. En cas de crash pendant l'écriture, le fichier de données original est préservé. Les 4 fichiers persistés sont : `messages.json`, `read_counts.json`, `groups.json`, `peer_records.json`.

**État : ✅ Solide** — 8 tests unitaires

---

### 1.9 Transfert de fichiers
**Fichiers :** `src/transfer/`

Envoi de fichiers et dossiers via TCP sur un port dédié (`chat_port + 1`). Le destinataire reçoit une proposition qu'il accepte ou refuse dans l'UI, choisit un dossier de destination, et voit la progression en temps réel. Protection contre le path traversal côté réception.

**État : ⚠️ Fragile** — Aucun test, fonctionnalité secondaire par rapport à l'objectif core.

---

## 2. Points critiques détaillés

### A. Serveur TCP — Pas de timeout ni limite de taille

**Fichier :** `src/network/server.rs`, ligne 31
**Sévérité :** 🔴 Critique

#### Le code actuel

```rust
async fn handle_incoming(mut stream: TcpStream, tx: Sender<AppEvent>) {
    let mut buf = Vec::new();
    if stream.read_to_end(&mut buf).await.is_ok() && !buf.is_empty() {
        // traitement du paquet...
    }
}
```

#### Ce que fait `read_to_end()`

`read_to_end()` lit **tout ce qui arrive sur le socket jusqu'à ce que la connexion soit fermée par l'émetteur**. Il n'y a aucune limite de temps ni de taille. Le `Vec::new()` s'agrandit dynamiquement en RAM pour accommoder tout ce qui arrive.

#### Problème 1 — Blocage infini (Slowloris)

Imaginons qu'un pair envoie des données très lentement — une technique d'attaque connue sous le nom de "Slowloris" :

```
Pair → Serveur : 1 octet toutes les 30 secondes
```

Depuis le point de vue du serveur, la connexion est toujours ouverte et il y a encore des données à lire. `read_to_end()` attend indéfiniment. La tâche tokio reste bloquée pendant des heures. Si plusieurs pairs font ça simultanément, toutes les tâches sont bloquées et plus aucun message ne peut être reçu.

Sur un réseau LAN de confiance c'est peu probable, mais une simple app qui plante (sans fermer proprement son socket) peut provoquer le même effet involontairement.

#### Problème 2 — Saturation mémoire (OOM)

Sans limite de taille, `read_to_end()` accepte n'importe quelle quantité de données :

```
Pair → Serveur : fichier de 500 MB envoyé sur le port de chat
```

Le `Vec<u8>` grossit jusqu'à 500 MB en RAM. Si le système n'a pas assez de mémoire, l'OS tue le processus (`OOM Killer` sur Linux). En conditions normales les paquets font quelques centaines d'octets, mais rien n'empêche un pair buggé d'envoyer des données corrompues de taille arbitraire.

#### Solution proposée

```rust
use tokio::time::{timeout, Duration};
use tokio::io::AsyncReadExt;

const MAX_PACKET_SIZE: usize = 65_536; // 64 KB largement suffisant pour un message JSON
const READ_TIMEOUT: Duration = Duration::from_secs(5);

async fn handle_incoming(mut stream: TcpStream, tx: Sender<AppEvent>) {
    let mut buf = vec![0u8; MAX_PACKET_SIZE];

    let n = match timeout(READ_TIMEOUT, stream.read(&mut buf)).await {
        Ok(Ok(n)) if n > 0 => n,
        _ => return, // timeout, erreur ou connexion vide → on ignore proprement
    };

    match serde_json::from_slice::<NetworkPacket>(&buf[..n]) {
        Ok(packet) => { /* dispatch normal */ }
        Err(_) => eprintln!("[network] Paquet invalide ({} bytes)", n),
    }
}
```

**Pourquoi 64 KB ?** Le plus gros paquet légitime est un message avec beaucoup de contenu. Un message texte de 64 KB représente environ 16 000 mots — bien au-delà de ce qu'un utilisateur pourrait taper. C'est une limite raisonnable qui protège sans bloquer l'usage normal.

---

### B. Hash de message — Collisions possibles

**Fichier :** `src/app/receipts.rs`, ligne 20
**Sévérité :** 🟠 Important

#### Le code actuel

```rust
pub fn message_hash(msg: &ChatMessage) -> u64 {
    let content = format!("{}:{}", msg.from, msg.content);
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}
```

#### Le rôle du hash

Ce hash est utilisé comme identifiant unique d'un message pour les ACK et les ReadReceipts. Quand Alice envoie un message, son hash est stocké dans `pending_messages`. Quand Bob envoie un ACK, il inclut ce hash. Alice retrouve le message par ce hash et le marque comme livré.

#### Problème 1 — Deux messages identiques du même expéditeur

Si Alice envoie "Bonjour" à 14:00 puis "Bonjour" à 14:05 :

```
hash("alice:Bonjour") = 0xABCD1234
hash("alice:Bonjour") = 0xABCD1234  ← identique !
```

Les deux messages ont le même hash. Quand Bob accuse réception du deuxième message, Alice marque **les deux** comme lus (car le hash est identique). Visuellement, le premier "Bonjour" passe directement de ✓ à ✓✓ bleu sans être réellement lu.

#### Problème 2 — `DefaultHasher` non déterministe entre processus

Depuis Rust 1.36, `DefaultHasher` utilise une graine aléatoire par processus pour prévenir les attaques HashDoS. Cela signifie que :

```
Processus A : hash("alice:Bonjour") = 0xABCD1234
Processus B : hash("alice:Bonjour") = 0x9876FEDC  ← différent !
```

Dans notre cas, le hash est calculé **des deux côtés** : Alice calcule le hash de son message pour le mettre dans `pending_messages`, et Bob calcule le hash du message reçu pour l'inclure dans l'ACK. Comme ils sont dans des processus différents, les hashes sont différents → **l'ACK ne matche jamais**.

En pratique, les ACK semblent fonctionner, ce qui suggère que le hash est recalculé côté Alice sur le message reçu en echo — mais c'est fragile et dépend de détails d'implémentation.

#### Solution proposée

Inclure tous les champs discriminants + utiliser un hash stable (pas `DefaultHasher`) :

```rust
pub fn message_hash(msg: &ChatMessage) -> u64 {
    // FNV-1a : hash simple, stable, déterministe entre processus
    let mut hash: u64 = 14_695_981_039_346_656_037;
    let key = format!(
        "{}:{}:{}:{}",
        msg.from,
        msg.to_user.as_deref().unwrap_or("broadcast"),
        msg.timestamp,
        msg.content
    );
    for byte in key.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    hash
}
```

**Pourquoi FNV-1a ?** C'est un algorithme de hash non cryptographique, sans dépendance externe, déterministe entre tous les processus et plateformes. Parfait pour identifier des messages dans un protocole applicatif simple.

---

### C. Aucun test réseau (découverte et transport)

**Fichiers :** `src/discovery.rs`, `src/network/`
**Sévérité :** 🟠 Important

#### La situation

130 tests passent — mais ils couvrent uniquement la logique métier (messages, peers, groupes, persistence). La couche réseau — qui est le cœur de l'application — n'a **aucun test**.

Si demain quelqu'un modifie `server.rs` et casse le dispatch des paquets, les 130 tests continueront de passer et la régression ne sera détectée qu'à l'exécution manuelle.

#### Ce qui n'est pas testé

| Composant | Cas non testés |
|---|---|
| `discovery.rs` | Émission d'un broadcast UDP, réception, timeout 6s, cleanup |
| `server.rs` | Réception d'un `NetworkPacket::Chat`, d'un `ReadReceipt`, d'un paquet invalide |
| `sender.rs` | Envoi d'un message, gestion d'une connexion refusée |
| Intégration | Deux instances qui se découvrent et échangent un message |

#### Tests unitaires réseau proposés

Pour `server.rs` : lier un vrai `TcpListener` sur un port local, connecter un `TcpStream`, écrire un `NetworkPacket` sérialisé, vérifier que le bon `AppEvent` est reçu via le channel mpsc.

```rust
#[tokio::test]
async fn test_server_receives_chat_message() {
    let (tx, mut rx) = mpsc::channel(8);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        handle_incoming(stream, tx).await;
    });

    let packet = NetworkPacket::Chat(ChatMessage {
        from: "alice".to_string(),
        content: "hello".to_string(),
        timestamp: "14:00".to_string(),
        to_user: None,
    });

    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(&serde_json::to_vec(&packet).unwrap()).await.unwrap();
    stream.shutdown().await.unwrap();

    let event = rx.recv().await.unwrap();
    assert!(matches!(event, AppEvent::MessageReceived(m) if m.content == "hello"));
}
```

---

### D. `integration_test.sh` ne teste pas le vrai scénario

**Fichier :** `scripts/integration_test.sh`
**Sévérité :** 🟡 Mineur mais bloquant pour CI/CD

#### Ce que fait le script aujourd'hui

1. `cargo check` — vérifie que ça compile
2. `cargo test` — lance les 130 tests unitaires
3. `cargo build --release` — compile en release
4. Vérifie que les fichiers de données existent après un démarrage bref
5. Vérifie que l'app démarre sans crasher immédiatement

Ce script est utile mais ne teste **pas le scénario fondamental** : est-ce que deux instances se découvrent et peuvent s'envoyer un message ?

#### Scénario de test d'intégration minimal proposé

```bash
# Lance deux instances
ABCOM_INSTANCE=1 ./target/release/abcom alice > /tmp/alice.log 2>&1 &
ABCOM_INSTANCE=2 ./target/release/abcom bob   > /tmp/bob.log   2>&1 &

sleep 5  # laisser le temps à la découverte UDP (broadcast toutes les 3s)

# Vérifier que chaque instance a découvert l'autre
grep -q "PeerDiscovered" /tmp/alice.log || { echo "FAIL: alice n'a pas découvert bob"; exit 1; }
grep -q "PeerDiscovered" /tmp/bob.log   || { echo "FAIL: bob n'a pas découvert alice"; exit 1; }

echo "PASS: découverte P2P fonctionnelle"
```

Pour tester l'envoi de messages sans UI, il faudrait exposer une commande CLI ou un socket de contrôle — c'est une amélioration future, pas immédiate.

---

## 3. Couverture des tests par module

| Module | Tests | Cas couverts |
|---|---|---|
| `app::messages` | 11 | Add, unread, broadcast/privé, cap 500, clear |
| `app::receipts` | 9 | Hash déterministe, mark read/acked, retry backoff |
| `app::peers` | 12 | Add/update, cleanup, online/offline, display name |
| `app::groups` | 13 | Validation nom, create/delete, membres, owner |
| `app::persistence` | 8 | Écriture atomique, round-trip JSON, fichier absent |
| `message::chat` | 7 | Sérialisation broadcast/privé, legacy compat |
| `message::receipts` | 6 | TypingIndicator, ReadReceipt, MessageAck |
| `message::group` | 7 | Create/Delete/AddMember/Rename |
| `message::network_types` | 3 | DiscoveryPacket, PeerRecord, legacy |
| `ui::composer` | 27 | Text ops, emoji multibyte, shortcodes |
| `ui::input_bar` | 9 | Garde message vide, attachements, touche Entrée |
| `ui::markdown` | 9 | Gras, italique, code, liens, titres |
| `transfer::storage` | 3 | Résolution de chemin, protection path traversal |
| `transfer::transfers` | 2 | Filtrage des cibles de transfert |
| **`network::server`** | **0** | **❌ Aucun test** |
| **`network::sender`** | **0** | **❌ Aucun test** |
| **`discovery`** | **0** | **❌ Aucun test** |
| **`transfer::service`** | **0** | **❌ Aucun test** |
| **TOTAL** | **130** | 0 échec |

---

## 4. Plan CI/CD proposé

### Pipeline pour les PR → `dev`
Rapide (~2 min), bloque le merge si l'un échoue.

```yaml
# .github/workflows/ci-dev.yml
name: CI — dev

on:
  pull_request:
    branches: [dev]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      - name: Format
        run: cargo fmt --check

      - name: Lint
        run: cargo clippy -- -D warnings

      - name: Build release
        run: cargo build --release

      - name: Tests unitaires
        run: cargo test
```

### Pipeline pour les PR → `main`
Plus complet (~5 min), inclut les tests d'intégration et l'audit de sécurité.

```yaml
# .github/workflows/ci-main.yml
name: CI — main

on:
  pull_request:
    branches: [main]

jobs:
  full-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      - name: Format
        run: cargo fmt --check

      - name: Lint
        run: cargo clippy -- -D warnings

      - name: Build release
        run: cargo build --release

      - name: Tests unitaires
        run: cargo test

      - name: Tests d'intégration
        run: bash scripts/integration_test.sh

      - name: Audit sécurité dépendances
        run: |
          cargo install cargo-audit --quiet
          cargo audit
```

---

## 5. Roadmap de solidification

### Sprint 1 — Robustesse réseau (priorité haute)
| Tâche | Fichier | Effort estimé |
|---|---|---|
| Timeout + limite taille sur `read_to_end()` | `src/network/server.rs` | 30 min |
| Fix hash de message (FNV-1a + timestamp) | `src/app/receipts.rs` | 20 min |

### Sprint 2 — Tests réseau (priorité haute)
| Tâche | Fichier | Effort estimé |
|---|---|---|
| Tests unitaires `server.rs` (4 cas) | `src/network/server.rs` | 1h |
| Tests unitaires `sender.rs` (3 cas) | `src/network/sender.rs` | 45 min |
| Améliorer `integration_test.sh` | `scripts/integration_test.sh` | 1h |

### Sprint 3 — CI/CD (priorité haute)
| Tâche | Fichier | Effort estimé |
|---|---|---|
| GitHub Actions CI dev | `.github/workflows/ci-dev.yml` | 30 min |
| GitHub Actions CI main | `.github/workflows/ci-main.yml` | 30 min |

### Sprint 4 — Tests transfert (priorité basse)
| Tâche | Fichier | Effort estimé |
|---|---|---|
| Tests unitaires `transfer/service.rs` | `src/transfer/service.rs` | 2h |

---

*Audit réalisé le 2026-06-23 — à mettre à jour après chaque sprint de solidification.*
