> [🏠 Accueil](../README.md) > [🔗 Groupes](2026-07-spec-groupes.md)

> 📅 **Généré le** : 2026-07-05 — Phase 10 du projet
> 🔖 **Décisions actées** : messages de salon adressés aux seuls membres · clé de conversation `#<nom>` · succession du propriétaire au départ · départ = effacement de l'historique local, conservation attribuée chez les autres
> 🔄 **À régénérer si** : identifiant de groupe stable (UUID) à la place du nom, chiffrement de salon dédié, accusés de lecture multi-destinataires

# Phase 10 — Groupes : création, messagerie, membres, historique

> ✅ **Implémenté le 2026-07-05** (226 tests verts). Corrige au passage le
> gel de l'application à la création d'un groupe (deadlock, cf. §8.1) et
> plusieurs règles métier manquantes (messages diffusés à tout le réseau,
> fil de salon invisible, compteurs non-lus absents).

## 1. Objectif et périmètre

Permettre à un utilisateur de :

- **créer un groupe** avec un nom personnalisé et y inclure directement des
  pairs du réseau local ;
- **échanger des messages** visibles par les seuls membres du groupe ;
- **gérer les membres** : ajout par le propriétaire, exclusion par le
  propriétaire, départ volontaire de n'importe quel membre ;
- comprendre ce que devient **l'historique** quand quelqu'un part (§6).

Abcom est une application **pair-à-pair sans serveur** : il n'existe aucune
autorité centrale qui détienne « la » vérité d'un groupe. Chaque pair stocke
sa propre copie de la liste des groupes (SQLite, table `groups`) et de
l'historique (table `messages`). La cohérence est obtenue par des événements
de synchronisation envoyés en TCP chiffré (Noise), comme le reste du trafic.

## 2. Modèle de données

### 2.1 Le groupe ([src/message/group.rs](../src/message/group.rs))

```rust
pub struct Group {
    pub name: String,        // identifiant ET nom d'affichage (unique, insensible à la casse)
    pub owner: String,       // username du propriétaire courant
    pub members: Vec<String>,// usernames, propriétaire inclus, ordre d'arrivée
    pub created_at: String,  // "AAAA-MM-JJ HH:MM:SS" locale du créateur
}
```

Le **nom sert d'identifiant** : 1 à 50 octets, alphanumériques Unicode plus
`-` et `_` (validation `validate_group_name`, refus des doublons sans tenir
compte de la casse). C'est une limite assumée du modèle (voir §9).

### 2.2 La clé de conversation `#<nom>`

Un message de salon est un `ChatMessage` ordinaire dont `to_user` vaut
`"#<nom du groupe>"`. Le préfixe `#` est réservé : les usernames ne peuvent
pas en contenir, il n'y a donc aucune ambiguïté avec une conversation
privée. Cette clé irrigue tout le code :

| `to_user` | Signification |
|---|---|
| `None` | fil public « Tous » (broadcast) |
| `Some("bob")` | message privé pour bob |
| `Some("#equipe")` | message du salon `equipe` |

Conséquence agréable : le stockage, la pagination, les réactions, les
réponses et le markdown fonctionnent pour les salons **sans aucun code
spécifique** — seul le filtrage par conversation distingue les trois cas
([src/app/messages.rs](../src/app/messages.rs), `get_conversation_messages`,
`unread_count`, `mark_conversation_read`, `clear_conversation_history`).

### 2.3 Persistance

- table `groups` : une ligne par groupe, JSON complet en colonne `data`,
  remplacée intégralement à chaque mutation (`StorageCmd::ReplaceGroups`,
  la table est petite) ;
- table `messages` : rien de nouveau, `to_user` porte la clé `#<nom>` ;
- compteurs de lecture : table `read_counts`, clé `#<nom>` pour les salons ;
- deux commandes de stockage ajoutées :
  `DeleteConversation { conv: Some("#nom") }` (purge d'un salon) et
  `RenameConversation { old, new }` (migration d'historique au renommage).

## 3. Synchronisation réseau

### 3.1 Les événements ([src/message/group.rs](../src/message/group.rs))

```rust
pub enum GroupAction {
    Create { group: Group },                          // création OU re-synchronisation complète
    AddMember { group_name, username },               // ajout incrémental
    RemoveMember { group_name, username },            // départ volontaire OU exclusion
    Rename { group_name, new_name },                  // renommage (protocole prêt, pas d'UI)
    Delete { group_name },                            // suppression par le propriétaire
}
```

Les événements circulent dans des paquets `NetworkPacket::Group` sur les
mêmes connexions chiffrées que les messages (canal mpsc `send_group_tx` →
`run_sender_group` → pool de connexions).

### 3.2 Qui envoie quoi, à qui

**Règle générale : les événements et messages d'un salon ne sont envoyés
qu'aux membres du salon** (`group_member_addrs` : membres en ligne, moi
exclu, adresses joignables). Avant cette phase, la création était diffusée
à *tous* les pairs du réseau — n'importe qui apprenait l'existence et la
composition de chaque groupe.

| Action locale | Événements émis |
|---|---|
| Créer le groupe | `Create` → membres en ligne |
| Ajouter un membre | `AddMember` → anciens membres · `Create` (état complet) → le nouveau, qui ne connaît pas encore le salon |
| Exclure un membre | `RemoveMember` → tous les membres, **exclu compris** (adresses relevées avant le retrait) |
| Quitter le groupe | `RemoveMember(moi)` → les autres membres |
| Supprimer le groupe | `Delete` → tous les membres |

### 3.3 Rattrapage des absents

Les événements ne sont ni persistés côté émetteur ni rejoués : un membre
hors ligne au moment d'une mutation la manquerait. Le rattrapage est fait
par le **propriétaire** : à chaque `PeerDiscovered` (un pair apparaît ou
réapparaît sur le réseau), il renvoie un `Create` complet de chaque groupe
qu'il possède et dont ce pair est membre
([src/ui/events.rs](../src/ui/events.rs)). Côté récepteur, `Create` est un
*upsert* : le groupe reçu remplace la copie locale du même nom — l'état du
propriétaire fait foi.

### 3.4 Règles d'application côté récepteur

- `Create` : ignoré si je ne figure pas dans `members` (un salon qui ne me
  concerne pas ne doit pas apparaître chez moi) ; sinon upsert.
- `AddMember` : ignoré si le groupe est inconnu (le nouveau membre, lui,
  reçoit un `Create`) ; ajout dédoublonné sinon.
- `RemoveMember` : applique la même logique que localement, succession
  comprise (§5.3) ; si le retiré, c'est moi → départ local complet (§6).
- `Rename` : validé (charte du nom, doublon) puis historique migré (§2.3).
- `Delete` : groupe et historique local supprimés.
- **Message de salon inconnu ou quitté : ignoré** — il n'est ni stocké ni
  notifié. Cela protège d'un émetteur retardataire (il n'a pas encore reçu
  mon départ) comme d'un pair non membre qui forgerait la clé.

## 4. Messagerie de salon

### 4.1 Envoi ([src/ui/input_bar.rs](../src/ui/input_bar/mod.rs))

Conversation `#nom` sélectionnée → le message part vers
`group_member_addrs(nom)` : **les membres en ligne du groupe, et personne
d'autre**. (Avant : `to_user` était bien la clé du salon, mais faute de
branche dédiée le message partait en broadcast à tous les pairs du réseau.)
L'indicateur de frappe « écrit… » suit la même règle, ainsi que les
transferts de fichiers/médias (`selected_transfer_targets`, qui gérait déjà
les groupes).

Un membre hors ligne au moment de l'envoi **ne recevra pas ce message**
(pas de file d'attente de réémission — même comportement que le fil
« Tous », cf. §9).

### 4.2 Réception et affichage

- le fil du salon montre tous les messages `to_user == "#nom"`, quel qu'en
  soit l'auteur, avec les couleurs par auteur du mode multi-personnes
  (comme « Tous ») ;
- compteur **non-lus** par salon dans la barre latérale (messages des
  autres, décompte remis à zéro à l'ouverture du salon) ;
- notification `#salon · auteur : contenu`, **sourdine par salon**
  fonctionnelle (la conversation source d'un message de groupe est le
  salon, plus l'expéditeur) ;
- **pas d'accusés** de livraison/lecture en salon : les ACK et ReadReceipts
  restent réservés aux conversations privées (un reçu par destinataire
  n'aurait pas de sémantique claire à N participants — hors périmètre).

## 5. Gestion des membres

### 5.1 Rôles

Deux rôles : **propriétaire** (👑, un seul) et **membre**. Le propriétaire
est le créateur, jusqu'à son départ (succession, §5.3).

| Capacité | Propriétaire | Membre |
|---|---|---|
| Envoyer/recevoir des messages | ✔ | ✔ |
| Ajouter un membre (pair connu) | ✔ | ✘ |
| Exclure un membre | ✔ (sauf lui-même) | ✘ |
| Quitter le groupe | ✔ (succession) | ✔ |
| Supprimer le groupe | ✔ | ✘ |

Ces règles sont vérifiées **chez l'initiateur** (`add_member_to_group`,
`remove_member_from_group`, `delete_group` exigent `owner == moi`) ; les
récepteurs appliquent les événements reçus (pas d'authentification du rôle
au niveau protocole, voir §9).

### 5.2 L'UI ([src/ui/group_modal.rs](../src/ui/group_modal.rs))

- **Création** (`+` de la barre latérale) : nom validé en direct (charte,
  doublon, compteur `n/50`), sélection des membres par cases à cocher avec
  pastille de présence et alias, créateur inclus d'office, `Entrée` ou
  bouton « Créer le groupe » (actif seulement si tout est valide), le salon
  s'ouvre aussitôt créé.
- **Gestion** (menu Actions → « ⚙ Gérer le groupe ») : membres avec
  présence/couronne/« (vous) », exclusion `✕` (propriétaire), section
  « Ajouter un membre » listant les pairs connus non membres
  (propriétaire), « Quitter le groupe » (tous), « Supprimer le groupe »
  (propriétaire) — les deux dernières confirmées en deux temps avec rappel
  de la conséquence sur l'historique.
- Barre latérale : ligne de salon avec nombre de membres et badge non-lus ;
  popup « Participants » listant les membres réels du salon (présence,
  couronne) et non plus la liste de tous les pairs.

### 5.3 Succession du propriétaire

Quand le propriétaire quitte (ou est retiré par un événement réseau), le
**premier membre restant dans l'ordre d'arrivée** hérite du groupe
(`apply_member_removal`). La règle est déterministe et appliquée à
l'identique par chaque réplique — pas de négociation réseau nécessaire.
S'il ne reste personne, le groupe disparaît.

## 6. Historique au départ d'un membre — politique retenue

**Question** : quand un membre quitte (ou est exclu), que deviennent les
messages ?

**Décision** (« quitter, c'est partir ») :

1. **Chez celui qui part** : le salon disparaît de sa barre latérale et son
   **historique local est effacé** (mémoire + SQLite, compteur de lecture
   compris ; s'il avait le salon ouvert, retour au fil « Tous »). Partir
   n'est donc pas anodin — d'où la confirmation en deux temps qui l'annonce
   explicitement.
2. **Chez les membres restants** : **rien ne change**. Les messages de
   l'ancien membre restent affichés, **attribués à son nom**. L'historique
   d'une conversation est un fait partagé : chaque réplique appartient à
   celui qui la détient, et réécrire le passé (anonymiser, effacer chez les
   autres) serait à la fois trompeur et inapplicable en pair-à-pair — rien
   n'empêcherait une réplique de conserver sa copie.
3. **Après le départ** : l'ex-membre n'envoie plus rien au salon et ses
   éventuels messages retardataires sont **ignorés** par les membres (§3.4) ;
   symétriquement, il ignorerait ceux d'un émetteur pas encore au courant.

**Alternatives évaluées et écartées** :

- *Conserver l'accès en lecture seule chez le partant* : séduisant
  (« j'étais là, j'ai le droit au passé »), mais demande un état de salon
  « archivé » dans toute l'UI (envoi bloqué, membres figés, badge dédié)
  pour un gain faible — le partant qui veut garder une trace peut la copier
  avant de partir. Réévaluable plus tard sans changer le protocole.
- *Anonymiser les messages du partant chez les autres* : impossible à
  garantir (répliques locales souveraines) et nuisible à l'intelligibilité
  du fil ; écarté.

La **suppression du groupe** par le propriétaire applique la règle 1 à tout
le monde : chaque membre reçoit `Delete` et purge salon + historique local.
L'**exclusion** est vécue par l'exclu exactement comme un départ (règle 1).

## 7. Corrections apportées par cette phase

| Symptôme | Cause | Correction |
|---|---|---|
| **Gel de l'application** au clic « Créer » | Le `MutexGuard` du scrutinee d'un `if let` vit jusqu'à la fin du bloc (édition 2021) ; le corps reprenait le même verrou → auto-deadlock sur `std::sync::Mutex` | Un seul passage sous verrou (création + adresses), envoi réseau hors verrou ([group_modal.rs](../src/ui/group_modal.rs)) |
| Messages de salon envoyés à **tous les pairs** du réseau | Pas de branche « groupe » à l'envoi : conversation sans adresse ⇒ broadcast | Envoi restreint à `group_member_addrs` (§4.1) |
| Fil de salon **vide** (les messages des autres n'apparaissaient jamais) | `get_conversation_messages` sans cas `#nom` | Filtrage par clé de salon (§2.2) |
| Création/mutation de groupe **invisible** jusqu'au message suivant | `save_groups` ne bumpait pas la génération des caches dérivés | `save_groups` invalide la barre latérale |
| Groupes annoncés à des non-membres | `Create` diffusé à tous les pairs en ligne | Événements restreints aux membres (§3.2) |
| ACK privé émis pour des messages de salon | `to_user.is_some()` confondait privé et salon | ACK/ReadReceipt réservés au privé (§4.2) |
| Effacer l'historique d'un salon ne touchait pas la base | `delete_conversation` SQL sans cas `#nom` | Branche dédiée `to_user = '#nom'` |

## 8. Tests

`src/tests/test_app_groups.rs` (13 tests) couvre : validation du nom,
création (succès, doublon, membre inconnu, nom vide), rôles
(`is_group_owner`, ajout refusé pour un pair inconnu), destinataires
(`group_member_addrs` : membres en ligne seulement, moi exclu),
départ (purge de l'historique et de la sélection), succession du
propriétaire, disparition du groupe vidé, suppression réservée au
propriétaire, renommage (validation + migration de l'historique et de la
conversation sélectionnée), filtrage du fil et compteur non-lus d'un salon.

Rejouer : `cargo test app::groups` (ou `cargo test` complet — 226 verts).

## 9. Limites connues et pistes

- **Le nom est l'identifiant** : deux pairs créant chacun un groupe
  `equipe` ont deux groupes distincts qui se percuteront (l'upsert `Create`
  du §3.3 fera foi du dernier propriétaire vu). Piste : identifiant UUID +
  nom d'affichage libre — changement de protocole, à faire d'un bloc.
- **Pas d'authentification du rôle dans le protocole** : un pair modifié
  pourrait forger un événement de groupe (le transport, lui, est authentifié
  par Noise + PSK de salon éventuel). Cohérent avec le modèle de menace
  actuel (réseau local de confiance, cf. [04-securite-globale.md](2026-04-securite-globale.md)).
- **Pas de réémission** : un membre hors ligne manque les messages émis
  pendant son absence (les mutations de membres, elles, sont rattrapées par
  le propriétaire, §3.3). Piste : journal par salon avec offset par membre.
- **Renommage sans UI** : protocole et migration d'historique prêts
  (`Rename`), pas de bouton — le nom se choisit à la création.
- Si **aucun membre n'est en ligne**, un message de salon n'atteint
  personne (il reste dans l'historique local de l'auteur) ; la barre de
  saisie reste active, contrairement au privé qui affiche « hors ligne ».
