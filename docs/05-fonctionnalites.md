# 05 — Fonctionnalités en détail

Ce document décrit le comportement attendu de l'application, fonctionnalité par fonctionnalité. C'est la référence à consulter avant de modifier une règle métier.

## Conversations

Trois types de fils, distingués par le champ `to_user` du message :

| `to_user` | Fil | Qui reçoit |
|---|---|---|
| `None` | « Tous » | Tous les pairs en ligne (broadcast) |
| `Some("bob")` | Privé | Le seul destinataire |
| `Some("#equipe")` | Salon | Les membres en ligne du groupe, personne d'autre |

Le préfixe `#` est réservé aux salons (les pseudos ne peuvent pas en contenir). Cette convention fait que stockage, pagination, réactions, réponses et Markdown fonctionnent à l'identique pour les trois cas — seul le filtrage par conversation les distingue.

Dans les fils multi-participants (« Tous » et salons), chaque auteur a une couleur stable. Le fil regroupe les messages consécutifs d'un même auteur et insère des séparateurs de jour. Les messages sont horodatés en heure locale.

## Rédaction et affichage des messages

- **Markdown** : gras, italique, code inline et bloc, liens.
- **Emojis** : picker par catégories, recherche, saisie par `:shortcode:` avec menu de suggestions ; un message composé uniquement d'emojis est affiché en grand. Jeu de 323 emojis embarqué dans le binaire.
- **Réactions** : barre au survol d'un message ; les réactions des autres s'agrègent sous le message et se togglent au clic.
- **Réponses** : répondre cite le message d'origine (auteur + extrait), cliquable pour y remonter.
- **Frappe** : « écrit… » apparaît dans la barre de saisie (rangée des boutons) pendant que l'interlocuteur tape ; en salon, l'indicateur n'est envoyé qu'aux membres.
- **Sélection** : le texte du fil se sélectionne au clic-glisser.

### Raccourcis clavier de la zone de saisie

| Raccourci | Action |
|---|---|
| `Entrée` ou `Maj+Entrée` | Nouvelle ligne |
| `Cmd+Entrée` (macOS) / `Ctrl+Entrée` | Envoyer le message |
| `Entrée` (menu de shortcodes ouvert) | Insérer l'emoji sélectionné |
| `Tab` | Compléter le shortcode avec la première suggestion |
| `↑` / `↓` (menu de shortcodes ouvert) | Naviguer dans les suggestions |
| `Option+⌫` (macOS) / `Ctrl+⌫` | Supprimer le mot précédent |
| `Option+Suppr` (macOS) / `Ctrl+Suppr` | Supprimer le mot suivant |
| `Cmd+⌫` (macOS) | Supprimer jusqu'au début de la ligne |
| `Option+←/→` (macOS) / `Ctrl+←/→` | Se déplacer de mot en mot |
| `Cmd+←/→` (macOS), `Début`/`Fin` | Aller au début / à la fin de la ligne |
| `Cmd/Ctrl+A` | Tout sélectionner |
| `Cmd/Ctrl+C`, `X`, `V` | Copier, couper, coller |
| `Maj+flèches/clic` | Étendre la sélection |

## Accusés de réception (conversations privées uniquement)

- ✓ : message envoyé.
- ✓✓ gris : livré — le destinataire renvoie un `Ack` automatique à la réception.
- ✓✓ bleu : lu — le `ReadReceipt` n'est envoyé que lorsque le destinataire a la conversation ouverte **et** la fenêtre au premier plan (lecture réelle, pas simple réception).
- Sans `Ack`, le message est retransmis avec un backoff exponentiel (1 s, 2 s, 4 s… plafonné à 32 s).

Les salons et le fil « Tous » n'ont ni accusés ni retransmission : un reçu par destinataire n'a pas de sémantique claire à N participants (choix assumé, voir [09](09-limites-et-pistes.md)).

## Groupes (salons)

Abcom étant sans serveur, il n'existe pas de « vérité centrale » d'un groupe : chaque pair stocke sa copie (table `groups`) et la cohérence vient d'événements de synchronisation chiffrés comme le reste du trafic.

### Modèle

Un groupe ([message/group.rs](../src/message/group.rs)) : un nom (identifiant **et** libellé, 1 à 50 caractères alphanumériques Unicode plus `-` et `_`, unicité insensible à la casse), un propriétaire, la liste des membres dans l'ordre d'arrivée, une date de création.

### Rôles

| Capacité | Propriétaire | Membre |
|---|---|---|
| Envoyer / recevoir | ✔ | ✔ |
| Ajouter un membre (pair connu) | ✔ | ✘ |
| Exclure un membre | ✔ (sauf lui-même) | ✘ |
| Quitter le groupe | ✔ (avec succession) | ✔ |
| Supprimer le groupe | ✔ | ✘ |

Ces règles sont vérifiées chez l'initiateur ; les récepteurs appliquent les événements reçus (le rôle n'est pas authentifié au niveau protocole — cohérent avec le modèle de menace).

### Synchronisation

Cinq événements (`Create`, `AddMember`, `RemoveMember`, `Rename`, `Delete`) circulent **uniquement vers les membres** du groupe. `Create` sert aussi de resynchronisation complète : c'est un upsert, l'état du propriétaire fait foi. Cas notables :

- ajout d'un membre : les anciens membres reçoivent `AddMember`, le nouveau reçoit un `Create` complet (il ne connaissait pas le salon) ;
- exclusion : `RemoveMember` part vers tous les membres, **exclu compris** (adresses relevées avant le retrait) ;
- **rattrapage des absents** : les événements ne sont pas rejoués ; à chaque réapparition d'un pair sur le réseau, le propriétaire lui renvoie un `Create` complet de chaque groupe le concernant ;
- côté récepteur, un `Create` où je ne figure pas est ignoré ; un message d'un salon inconnu ou quitté est ignoré (ni stocké, ni notifié) — protection contre les émetteurs retardataires comme contre une clé forgée.

### Succession et historique

- **Succession** : si le propriétaire part, le premier membre restant dans l'ordre d'arrivée hérite du groupe. Règle déterministe, appliquée à l'identique par chaque réplique, sans négociation réseau. Plus personne → le groupe disparaît.
- **« Quitter, c'est partir »** : celui qui quitte (ou est exclu) perd le salon **et son historique local** (mémoire, base, compteur de lecture) — d'où une confirmation en deux temps qui l'annonce. Chez les membres restants, rien ne change : les messages de l'ex-membre restent affichés et attribués. Réécrire le passé chez les autres serait à la fois trompeur et inapplicable en pair-à-pair.
- **Suppression** par le propriétaire : chaque membre purge salon et historique.
- Un membre hors ligne au moment d'un envoi **ne recevra pas ce message** (pas de file de réémission ; les mutations de membres, elles, sont rattrapées par le propriétaire).
- Le renommage existe dans le protocole (avec migration d'historique) mais n'a pas encore de bouton dans l'UI.

### UI

Création par le `+` de la barre latérale (validation du nom en direct, cases à cocher des membres avec présence). Gestion par le menu Actions du salon : membres avec couronne 👑 et présence, exclusion, ajout, « Quitter » et « Supprimer » confirmés en deux temps. La barre latérale montre le nombre de membres et le badge non-lus par salon ; une sourdine par salon coupe ses notifications.

## Fichiers et médias

- **Envoi** : fichier ou dossier (compressé en ZIP à la volée, [archive.rs](../src/archive.rs)), vers un pair ou les membres d'un salon. Aucune limite pratique de taille (streaming par tranches de 60 Ko, accepté au-delà de 1 Go).
- **Acceptation** : au-delà de 1 Go, le destinataire reçoit une proposition qu'il accepte ou refuse (sans réponse sous 120 s, le transfert est abandonné) ; en deçà, la réception est automatique. Progression affichée des deux côtés. Le fichier reçu est rangé par l'application dans son dossier `media/` ; depuis la visionneuse, le bouton « ⬇ Télécharger » en dépose une copie dans le dossier Téléchargements du système.
- **Images** : vignette dans le fil (360×300 max, ratio préservé), visionneuse plein écran au clic.
- **GIF, mèmes, stickers** : sélecteur Klipy à trois onglets (recherche avec anti-rebond de 300 ms, scroll infini). Le GIF est transmis **par URL** : chaque pair le charge depuis le CDN Klipy. Nécessite la clé `ABCOM_KLIPY_API_KEY` ; sans elle, le bouton GIF affiche une notification au lieu du sélecteur. Attribution « Powered by KLIPY » dans le pied du sélecteur, crédits détaillés dans Paramètres.

## Avatars et alias

Chaque utilisateur peut choisir un avatar (PNG/JPEG ; l'import SVG existe derrière la feature Cargo `avatar-svg`, désactivée par défaut car elle embarque resvg). L'avatar est normalisé en 256×256 et annoncé aux pairs à la découverte. On peut donner un alias local à un pair (« Alice compta »), stocké en base et jamais transmis.

## Notifications et présence

- Fenêtre visible : toast interne + son (thread audio unique, lancé au premier bip).
- Fenêtre cachée ou minimisée : **notification système native** (macOS, Windows, Linux/D-Bus), avec deux formats réglables : aperçu (`Alice : début du message…`, défaut) ou discret (`Nouveau message de Alice`).
- Badge non-lus sur l'icône du tray ; compteurs par conversation dans la barre latérale, remis à zéro à l'ouverture.
- Sourdine par conversation (privée ou salon).

## Mode résident

- La croix et Cmd/Ctrl-W **cachent** la fenêtre au lieu de quitter ; sur macOS l'icône disparaît aussi du Dock (politique Accessory). On quitte par le menu du tray.
- Menu du tray : « Ouvrir Abcom », « Quitter ». Sous Windows le clic gauche ouvre directement la fenêtre.
- **Autostart** à l'ouverture de session : activé par défaut au premier lancement d'un build release (jamais en debug), interrupteur dans Paramètres → Général. Implémentation par plateforme : Launch Agent (macOS), clé de registre Run (Windows), `.desktop` (Linux).
- Sous Linux sans tray disponible, la croix quitte normalement.

## Paramètres

Thème clair/sombre/système, langue FR/EN, format des notifications, autostart, profil (pseudo, avatar, empreinte de clé, état de la passphrase de salon), crédits et licences.
