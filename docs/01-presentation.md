# 01 — Présentation du projet

## Ce qu'est Abcom

Abcom est un client de messagerie instantanée conçu pour un réseau local : un bureau, un domicile, un événement, un hotspot de téléphone. Chaque machine exécute le même binaire ; il n'y a ni serveur, ni compte, ni inscription. Dès que deux instances tournent sur le même réseau, elles se voient et peuvent discuter.

Trois principes guident le projet :

1. **Autonomie totale** — aucune dépendance à Internet ni à une infrastructure. La seule fonctionnalité en ligne est le sélecteur de GIF (API Klipy), et elle est optionnelle.
2. **Application résidente** — Abcom est fait pour rester ouvert en permanence, comme un client mail. L'empreinte au repos (CPU, GPU, RAM, disque) a fait l'objet d'un travail d'optimisation dédié : environ 0,2 % de CPU et 155 Mo de RAM au repos, fenêtre repliée dans la zone système.
3. **Confidentialité par défaut** — tout le trafic (messages, fichiers, réactions, accusés) est chiffré de bout en bout, chaque machine possède une identité cryptographique, et les clés des pairs sont épinglées à la première rencontre.

## Comment ça marche, en bref

Au lancement, l'application annonce sa présence sur le réseau par un paquet UDP toutes les 3 secondes (broadcast et multicast). Les autres instances qui reçoivent cette annonce ajoutent le pair à leur barre latérale ; un pair silencieux pendant 6 secondes est marqué hors ligne.

Quand un utilisateur envoie un message, une connexion TCP directe et persistante est établie vers chaque destinataire. Cette connexion est chiffrée par un handshake Noise XX : les deux machines s'authentifient mutuellement par leur clé X25519, puis échangent des trames ChaCha20-Poly1305. La clé publique d'un pair est mémorisée à la première connexion (modèle TOFU) ; si elle change, la connexion est refusée et l'utilisateur est alerté.

Chaque machine conserve son propre historique dans une base SQLite locale. Il n'existe aucune copie centrale : une conversation est l'ensemble des répliques que chaque participant détient.

## Ce que l'application sait faire

**Converser** — trois types de fils : le fil public « Tous » (visible par tout le réseau), les conversations privées entre deux pairs, et les salons de groupe réservés à leurs membres. Les messages supportent le Markdown, les emojis, les réactions et les réponses citées. Un indicateur montre quand l'interlocuteur écrit.

**Confirmer** — en conversation privée, l'expéditeur voit si son message est parti (✓), livré (✓✓ gris) ou lu (✓✓ bleu). Les messages non livrés sont retransmis automatiquement avec un délai croissant.

**Partager** — fichiers et dossiers de toute taille (les dossiers sont zippés à l'envoi), avec acceptation explicite du destinataire et barre de progression. Les images s'affichent en vignette dans le fil et s'ouvrent dans une visionneuse. Un sélecteur Klipy propose GIF animés, mèmes et stickers.

**Rester joignable** — fermer la fenêtre ne quitte pas l'application : elle se replie dans la barre de menus (macOS), la zone de notification (Windows) ou le tray (Linux). Les messages reçus fenêtre cachée déclenchent une notification système et un badge sur l'icône. Le lancement à l'ouverture de session est activé par défaut sur les builds release.

## Décisions fondatrices

Ces choix structurent le projet ; les remettre en cause revient à le refondre.

**Rust, avec tokio et egui.** Le besoin : un binaire natif léger, sûr en mémoire, capable de gérer la concurrence réseau et une interface graphique fluide. Electron a été écarté (trop lourd pour un chat LAN), Python/Node aussi (distribution d'un binaire statique difficile), Qt/GTK également (complexité disproportionnée). Le binaire release fait environ 11 Mo.

**Pair-à-pair sur LAN, sans serveur.** Un serveur central de découverte n'apporterait rien sur un réseau local et créerait un point de défaillance. La découverte se fait par UDP broadcast (complété par un groupe multicast pour que plusieurs instances d'une même machine se voient), les échanges par TCP direct.

**SQLite comme unique stockage.** La première version persistait tout en fichiers JSON réécrits intégralement à chaque message — intenable au-delà de quelques centaines de messages. Depuis juillet 2026, tout vit dans `abcom.db` (mode WAL, thread d'écriture dédié) : messages, réactions, compteurs, groupes, clés des pairs, avatars, préférences.

**Connexions persistantes et chiffrement Noise, livrés ensemble.** Le protocole initial ouvrait une connexion TCP par paquet et transmettait du JSON en clair. Aucune release n'ayant été publiée, la compatibilité n'avait pas à être préservée : le protocole a été remplacé d'un bloc par des connexions persistantes par pair avec trames chiffrées. Noise XX a été préféré à TLS (pas de PKI à simuler en P2P) et à une simple passphrase partagée (pas d'identité par pair, pas de forward secrecy).

**Un seul processus pour le mode résident.** Un découpage daemon + interface a été évalué et écarté : des semaines de plomberie IPC pour un gain mémoire récupérable autrement (purge des textures quand la fenêtre se cache). Le réseau, le stockage et la découverte sont déjà indépendants de l'UI ; la frontière existante permettrait ce découpage plus tard si un client mobile le justifiait.

**Un groupe est identifié par un identifiant immuable, pas par son nom.** Le nom a d'abord servi d'identifiant, pour la lisibilité. C'était un défaut de fond : cette clé entre dans le hash des messages, donc renommer un salon orphelinait d'un coup ses réactions, ses accusés et son repère de lecture. Le nom est désormais un simple libellé, librement modifiable ; les salons antérieurs retrouvent un identifiant dérivé de leurs données immuables, identique chez tous les pairs.

## Vocabulaire

| Terme | Définition |
|---|---|
| **Pair** (peer) | Une autre instance d'Abcom détectée sur le réseau, identifiée par son pseudo et sa clé publique |
| **Découverte** | Annonce UDP périodique par laquelle les instances se signalent mutuellement |
| **Fil « Tous »** | Conversation publique : les messages sont diffusés à tous les pairs en ligne |
| **Salon** (groupe) | Conversation réservée à une liste de membres ; identifiée en interne par la clé `#<id>`, où `id` est immuable — le nom n'est qu'un libellé |
| **Propriétaire** | Créateur d'un salon (ou son successeur) ; seul habilité à ajouter, exclure et supprimer |
| **ACK / accusé de lecture** | Confirmations réseau de livraison et de lecture d'un message privé |
| **Noise XX** | Motif de handshake cryptographique : authentification mutuelle par clés statiques X25519, secret de session éphémère |
| **TOFU** (Trust On First Use) | Modèle de confiance : la clé d'un pair est enregistrée à la première connexion et exigée ensuite |
| **Empreinte** | Condensé BLAKE2s de la clé publique, affiché dans Paramètres → Profil pour vérification manuelle |
| **Passphrase de salon** | Secret partagé optionnel (`ABCOM_PASSPHRASE`) qui restreint le handshake aux machines qui le connaissent |
| **Tray** | Icône résidente (barre de menus macOS, zone de notification Windows, StatusNotifier Linux) |
| **Fenêtre de messages** | Portion de l'historique chargée en mémoire pour l'affichage ; le reste demeure en base et se charge au scroll |
