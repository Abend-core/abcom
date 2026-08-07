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

Chaque instance émet toutes les 3 secondes un paquet JSON contenant son pseudo, son port TCP et l'empreinte de sa clé publique. L'émission se fait en broadcast (`255.255.255.255`) et sur le groupe multicast `239.255.42.98` — le broadcast n'étant pas rebouclé localement sur macOS, le multicast avec loopback permet aux instances d'une même machine de se découvrir.

Côté réception, la tâche de découverte tient l'état de présence (dernier signe de vie par pair, timeout à 6 secondes) et n'émet un événement vers l'UI que lorsque quelque chose change : nouveau pair, adresse modifiée, déconnexion, retour en ligne. Au repos, des pairs connectés ne réveillent donc pas le rendu.

L'annonce transporte la clé publique avant toute connexion TCP à titre informatif. Une annonce UDP n'étant pas signée, la source de vérité reste la clé présentée pendant le handshake Noise et épinglée par TOFU.

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

Les messages sont identifiés sur le réseau par un hash FNV-1a déterministe de (expéditeur, destinataire, timestamp epoch, contenu) — stable entre machines et plateformes, contrairement au `DefaultHasher` utilisé à l'origine.

## Modèle de menace

**Couvert (données en transit)** : un observateur du LAN ne peut ni lire le trafic (confidentialité), ni le modifier ou le rejouer (AEAD), ni se faire passer pour un pair connu (authentification par clé + TOFU). Le durcissement `XXpsk3` empêche en plus un inconnu d'établir la moindre session.

**Non couvert, assumé à ce stade** :

- **Chiffrement au repos** : `abcom.db`, le dossier `media/` et les avatars sont en clair sur le disque local. Piste : SQLCipher ou chiffrement fichier (voir [09 — Limites et pistes](09-limites-et-pistes.md)).
- **Métadonnées de découverte** : l'annonce UDP (pseudo + empreinte) est visible par tout le LAN — c'est la fonction même de la découverte.
- **Événements de groupe non signés** : l'auteur de la session est authentifié et ses droits sont vérifiés localement, mais un événement n'est pas transférable avec une preuve cryptographique hors de cette session.
- **Robustesse d'entrée** : le serveur limite la taille des trames et applique un timeout de lecture ; les paquets invalides sont ignorés sans faire tomber le service.

Recommandations d'usage : réserver Abcom aux réseaux de confiance, activer la passphrase de salon sur un réseau partagé, vérifier les empreintes pour les échanges sensibles.
