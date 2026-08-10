# 03 — Réseau et sécurité

## Ports

| Port | Protocole | Usage |
|---|---|---|
| 9001/udp | UDP broadcast + multicast | Découverte des pairs — partagé par toutes les instances |
| 9000/tcp | TCP, chiffré Noise | Messages, réactions, accusés, frappe, avatars, événements de groupe |
| 9001/tcp | TCP, chiffré Noise | Streaming des fichiers et médias (toujours `port chat + 1`) |

Pour tester plusieurs instances sur une même machine, `ABCOM_INSTANCE=N` décale les ports TCP (9010/9011, 9020/9021, …) et le répertoire de données (`abcom-N`) ; le port UDP de découverte reste partagé pour que les instances se voient ([config.rs](../src/config.rs)).

Sur un pare-feu Linux : `sudo ufw allow 9000:9001/tcp && sudo ufw allow 9001/udp`.

## Découverte des pairs

Chaque instance émet toutes les 3 secondes un paquet JSON **signé** contenant son pseudo, son port TCP, ses clés publiques et un horodatage. L'émission se fait en broadcast (`255.255.255.255`) et sur le groupe multicast `239.255.42.98` — le broadcast n'étant pas rebouclé localement sur macOS, le multicast avec loopback permet aux instances d'une même machine de se découvrir.

Côté réception, la tâche de découverte tient l'état de présence (dernier signe de vie par pair, timeout à 6 secondes) et n'émet un événement vers l'UI que lorsque quelque chose change : nouveau pair, adresse modifiée, déconnexion, retour en ligne. Au repos, des pairs connectés ne réveillent donc pas le rendu. Un pair expiré est aussi signalé au pool de connexions, qui libère la connexion correspondante.

### Constantes de découverte et leur coût

| Constante | Valeur | Effet si on l'augmente | Effet si on la diminue |
|---|---|---|---|
| Groupe multicast | `239.255.42.98` | — | — |
| Intervalle d'annonce | 3 s | Moins de trafic et de réveils radio (meilleure autonomie), détection plus lente | Détection plus réactive, mais un paquet toutes les N secondes par instance sur tout le LAN |
| Timeout de présence | 6 s (= 2 annonces) | Moins de faux « hors ligne » sur un réseau qui perd des paquets, pairs fantômes plus longtemps | Détection de coupure plus rapide, mais un seul paquet perdu suffit à faire clignoter la présence |
| Balayage des expirés | 2 s | — | Réveils plus fréquents de la tâche |
| Buffer de réception | 1 024 octets | — | Un pseudo très long tronquerait l'annonce (JSON invalide, paquet ignoré) — d'où le plafond de 64 caractères sur le pseudo |

Le coût réseau au repos est de deux datagrammes (multicast + broadcast) toutes les 3 secondes et par instance ; c'est le poste qui empêche la carte réseau de rester en veille prolongée. Les valeurs sont dans [discovery.rs](../src/discovery.rs) et [protocol.rs](../src/protocol.rs).

### Annonces signées

Chaque annonce porte une clé Ed25519 de vérification, un horodatage et une signature de `(pseudo, port, clé X25519, clé Ed25519, horodatage)`. La clé de signature est **dérivée de l'identité Noise** par BLAKE2s avec un domaine dédié : `identity.key` garde son format, et une identité produit toujours la même clé de signature.

Une annonce dont la signature ne vérifie pas, ou dont l'horodatage s'écarte de plus de 60 secondes de l'heure locale, est ignorée. Cela ferme trois choses : les annonces fabriquées pour une clé qu'on ne possède pas (pairs fantômes injectés sur le LAN), le détournement du port annoncé, et le rejeu d'annonces capturées.

Cela ne ferme **pas** la première rencontre : un pair peut toujours annoncer le pseudo d'un autre avec sa propre clé, correctement signée (voir le modèle de menace). La source de vérité pour la conversation reste la clé présentée pendant le handshake Noise et épinglée par TOFU.

## Identité et confiance

**Identité.** Au premier lancement, une paire de clés X25519 est générée et stockée dans `identity.key` (permissions 0600) dans le répertoire de données. L'empreinte BLAKE2s de la clé publique est affichée dans Paramètres → Profil ; deux utilisateurs peuvent la comparer de vive voix pour vérifier une identité.

**TOFU (Trust On First Use).** À la première connexion avec un pair, le couple (pseudo, clé publique) est enregistré dans la table `peers` de la base. À chaque connexion suivante, la clé présentée au handshake doit correspondre à la clé épinglée : en cas de divergence, la connexion est refusée et l'UI affiche une alerte « la clé de X a changé », avec une action explicite pour faire confiance à la nouvelle clé (réinstallation légitime, par exemple).

## Transport chiffré

**Connexions persistantes.** Un pool ([network/pool.rs](../src/network/pool.rs)) maintient une connexion TCP par pair, établie à la demande et réutilisée pour tout le trafic de chat. Le pseudo reçu dans le `Hello` doit correspondre au destinataire attendu pour l'adresse découverte ; une adresse annonçant un autre pair est rejetée.

**Handshake Noise XX.** Chaque connexion commence par un handshake `Noise_XX_25519_ChaChaPoly_BLAKE2s` (crate `snow`, 3 messages, 1,5 aller-retour — négligeable sur un LAN). Le `Hello` qui suit porte le pseudo, la version de protocole et les capacités ; une version incompatible est rejetée explicitement. Chaque paquet reçu est ensuite recoupé avec l'auteur authentifié par la session.

**Passphrase de salon (optionnelle).** Si la variable `ABCOM_PASSPHRASE` est définie (environnement ou fichier `.env`), le handshake passe en `XXpsk3` avec un secret pré-partagé dérivé de la passphrase (BLAKE2s). Sans la bonne passphrase, aucun handshake n'aboutit : c'est un moyen simple de cloisonner un groupe de machines sur un réseau partagé. L'état (actif ou non) est visible dans Paramètres → Profil. Tous les pairs doivent partager la même valeur.

**Médias.** Le port média utilise la même mécanique (handshake Noise, trames chiffrées). Les fichiers sont découpés en tranches de 60 Ko ; la progression est signalée à l'UI au plus toutes les 100 ms pour ne pas plafonner le débit sur la boucle de rendu.

## Types de paquets

Tous les échanges de chat sont des `NetworkPacket` (enum JSON taggé, [message/network_types.rs](../src/message/network_types.rs)) :

| Paquet | Contenu |
|---|---|
| `Chat` | Message (fil public, privé ou salon — distingués par le champ `to_user`) |
| `Group` | Événement de groupe : création, ajout/retrait de membre, renommage, suppression |
| `Typing` | Indicateur de frappe |
| `Ack` | Accusé de livraison d'un message privé |
| `ReadReceipt` | Accusé de lecture d'un message privé |
| `Reaction` | Ajout ou retrait d'une réaction emoji |
| `Avatar` | Annonce de l'avatar de l'expéditeur |

La version de protocole vaut **2** depuis l'ajout des annonces signées : un pair d'une version différente est rejeté au `Hello`.

Les messages sont identifiés sur le réseau par un hash FNV-1a déterministe de (expéditeur, destinataire, timestamp epoch, contenu) — stable entre machines et plateformes, contrairement au `DefaultHasher` utilisé à l'origine.

## Modèle de menace

**Couvert (données en transit)** : un observateur du LAN ne peut ni lire le trafic (confidentialité), ni le modifier ou le rejouer (AEAD), ni se faire passer pour un pair connu (authentification par clé + TOFU). Le durcissement `XXpsk3` empêche en plus un inconnu d'établir la moindre session.

**Non couvert, assumé à ce stade** :

- **La toute première rencontre** : le TOFU protège les connexions *suivantes*, pas la première. Tant qu'aucune clé n'est épinglée pour un pseudo, un pair malveillant peut annoncer le pseudo de quelqu'un d'autre **avec sa propre clé**, et signer cette annonce sans difficulté — la signature prouve la possession de la clé annoncée, pas le droit d'utiliser ce pseudo. La parade est la vérification d'empreinte hors-bande (Paramètres → Profil) au premier contact, et l'usage de la passphrase de salon sur un réseau ouvert.
- **Chiffrement au repos** : `abcom.db` (messages, avatars, clés épinglées), le dossier `media/` et le dossier de travail `scratch/` sont en clair sur le disque local. Ils sont dans le répertoire de données de l'utilisateur (`identity.key` est en 0600, ACL restreinte sous Windows), donc protégés des *autres comptes* de la machine, mais pas d'un accès physique au disque ni d'une sauvegarde. Piste : SQLCipher ou chiffrement fichier (voir [09 — Limites et pistes](09-limites-et-pistes.md)).
- **Métadonnées de découverte** : l'annonce UDP (pseudo + empreinte) est visible par tout le LAN — c'est la fonction même de la découverte.
- **Événements de groupe non signés** : l'auteur de la session est authentifié et ses droits sont vérifiés localement, mais un événement n'est pas transférable avec une preuve cryptographique hors de cette session.
- **Identifiants de messages devinables** : les réactions, réponses et accusés ciblent un message par son hash FNV-1a, qui n'est pas cryptographique. Un pair authentifié peut donc forger un identifiant plausible pour cibler un message qu'il n'a pas reçu. L'impact est limité (il faut déjà être un pair authentifié), mais c'est une raison de ne pas transformer ce hash en identifiant de sécurité.
- **Robustesse d'entrée** : le serveur limite la taille des trames et applique un timeout de lecture ; les paquets invalides sont ignorés sans faire tomber le service.

### Passphrase de salon : ce qu'elle protège, et ce qu'elle ne protège pas

| Question | Réponse |
|---|---|
| Qui la connaît ? | Tous les membres du salon, à l'identique — c'est un secret **partagé**, pas une identité |
| Comment se distribue-t-elle ? | Hors-bande, par un canal de confiance (de vive voix, gestionnaire de mots de passe). Elle est lue depuis `ABCOM_PASSPHRASE` ou le `.env` local, jamais transmise sur le réseau |
| Que protège-t-elle ? | Elle empêche un inconnu du LAN d'**établir la moindre session** (le handshake `XXpsk3` échoue sans elle) : ni connexion, ni paquet, ni découverte utile |
| Que ne protège-t-elle pas ? | Elle n'authentifie **personne à l'intérieur** du salon : un membre qui la connaît reste identifié par sa clé et son épinglage TOFU, pas par la passphrase. Elle ne se révoque pas individuellement — retirer l'accès à quelqu'un impose de la changer partout |
| Fuite de la passphrase | Elle ne compromet **pas** le contenu des sessions passées (les clés de session viennent du handshake, pas du PSK) : elle redonne seulement la capacité de se connecter |

Recommandations d'usage : réserver Abcom aux réseaux de confiance, activer la passphrase de salon sur un réseau partagé, vérifier les empreintes pour les échanges sensibles — en particulier au premier contact.
