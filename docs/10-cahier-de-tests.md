# Cahier de tests

Tests manuels d'abcom. La suite automatique (`make test`, 329 tests) couvre la
logique pure ; ce cahier couvre ce qu'elle ne peut pas voir : le réseau réel,
l'interface, le système de fichiers et les trois OS.

## Comment lancer

```bash
make run2        # 2 fenêtres : alice, bob
make rung        # 3 fenêtres : alice, bob, carol
make run-multi N=4
```

Chaque instance a son `ABCOM_INSTANCE` (ports 9000, 9010, 9020…) et son propre
répertoire de données. Les GIF exigent `ABCOM_KLIPY_API_KEY` dans `.env`.

**Repartir de zéro** — indispensable avant les tests de migration et de TOFU :

| OS | Répertoire à supprimer |
|---|---|
| Linux | `~/.local/share/abcom*` |
| macOS | `~/Library/Application Support/abcom*` |
| Windows | `%APPDATA%\abcom*` |

(Instance 0 = `abcom`, instance _n_ = `abcom-n`.)

## Conventions

Chaque test : **préconditions → actions → attendu**. Un test échoue si
l'attendu diffère, *ou* si un `tracing::error!` apparaît dans la console.

Priorités : **P0** bloque une livraison, **P1** défaut visible, **P2** confort.

---

# 1. Découverte et présence

| # | Test | Actions | Attendu | P |
|---|---|---|---|---|
| 1.1 | Découverte mutuelle | `make run2` | Chacun voit l'autre en ligne (pastille verte) sous 5 s | P0 |
| 1.2 | Déconnexion | Fermer bob | Alice le passe hors ligne sous ~10 s, la carte reste | P0 |
| 1.3 | Retour | Relancer bob | Repasse en ligne, l'historique est intact | P0 |
| 1.4 | Changement d'IP | Basculer de Wi-Fi à Ethernet | La nouvelle adresse est prise en compte, l'envoi repart | P1 |
| 1.5 | Horloge reculée | Reculer l'horloge système de 2 min, attendre 10 s | **Les pairs restent en ligne.** Régression : ils disparaissaient tous d'un coup | P1 |

> 1.5 vérifie le débordement corrigé dans `discovery.rs`. Remettre l'horloge à
> l'heure après le test.

# 2. Messages

| # | Test | Actions | Attendu | P |
|---|---|---|---|---|
| 2.1 | Privé | Alice → bob | Reçu sous 1 s, horodaté | P0 |
| 2.2 | Diffusion « Tous » | Alice écrit dans « Tous » | Tous les pairs en ligne reçoivent | P0 |
| 2.3 | Salon | Écrire dans un groupe | Seuls les membres reçoivent | P0 |
| 2.4 | Non-membre | Carol hors du groupe | Ne reçoit rien, pas d'erreur | P0 |
| 2.5 | Message long | Coller > 4 000 caractères | Le fil se replie avec « Afficher la suite » | P2 |
| 2.6 | Collage énorme | Coller ~1 Mo de texte | Converti en pièce jointe `.txt`, pas de gel | P1 |
| 2.7 | Plafond de saisie | Dépasser la limite | Compteur rouge, envoi refusé | P2 |
| 2.8 | Réponse | Répondre à un message | Citation affichée des deux côtés | P1 |
| 2.9 | Doublon | Couper le réseau de bob à l'envoi, le rétablir | Le message n'apparaît qu'**une** fois chez bob | P0 |

# 3. Accusés de réception et de lecture

| # | Test | Actions | Attendu | P |
|---|---|---|---|---|
| 3.1 | Livraison | Alice → bob (fenêtre bob en arrière-plan) | Alice voit « reçu » | P0 |
| 3.2 | Lecture | Bob ouvre la conversation | Alice voit « lu » | P0 |
| 3.3 | **Retour du focus** | Fenêtre de bob **déjà ouverte sur le salon** mais en arrière-plan. Alice envoie. Bob **clique sur sa fenêtre sans changer de conversation** | Alice voit « lu » sous 1 s. Régression : il fallait sortir du salon et y revenir | P0 |
| 3.4 | Retour du tray | Bob replie dans le tray, alice envoie, bob restaure | Accusé de lecture émis à la restauration | P1 |
| 3.5 | Pas de réémission | Alt-tab plusieurs fois sur une conversation lue | Aucun accusé réémis (vérifier les logs) | P2 |
| 3.6 | Détail en salon | Cliquer « … » sur un message de salon | Liste nominative « reçu par » / « lu par » | P1 |
| 3.7 | **Accusé après commit** | Rendre la base non inscriptible (voir §11.2), alice → bob | Bob **n'acquitte pas**, alice réémet. Rien n'est annoncé « reçu » à tort | P0 |

# 4. Groupes

| # | Test | Actions | Attendu | P |
|---|---|---|---|---|
| 4.1 | Création | Alice crée « equipe » avec bob | Le salon apparaît chez les deux | P0 |
| 4.2 | Nom invalide | Espaces, > 50 caractères, doublon | Refusé avec message | P1 |
| 4.3 | Ajout de membre | Alice ajoute carol | Carol reçoit le salon **et son historique reste vide** | P0 |
| 4.4 | Exclusion | Alice exclut carol | Le salon disparaît chez carol | P0 |
| 4.5 | Départ volontaire | Bob quitte | Historique local effacé chez bob, conservé chez alice | P0 |
| 4.6 | Succession | Le propriétaire part | Le premier membre restant devient propriétaire | P1 |
| 4.7 | Suppression | Alice supprime le salon | Disparaît chez tous les membres | P0 |
| 4.8 | Non-propriétaire | Bob tente de supprimer | Refusé | P0 |

## 4.9 — Renommage (le cœur de la refonte)

**Préconditions** : salon « equipe » avec alice et bob, une dizaine de messages,
au moins une **réaction**, un message **lu** (accusé visible) et une **réponse**.

1. Alice renomme le salon en « equipe-2 ».

**Attendu, chez alice et chez bob :**

- le nom change partout : barre latérale, titre du fil, popup participants ;
- **l'historique reste entier** — aucun message ne disparaît ;
- **les réactions restent attachées** à leurs messages ;
- **les accusés « reçu / lu » restent affichés** ;
- **le compteur de non-lus reste juste** (pas de saut à zéro ni de réapparition) ;
- envoyer un nouveau message continue de fonctionner, avec son accusé.

> C'est le scénario qui cassait : le nom entrait dans le hash des messages, donc
> le renommage orphelinait réactions, accusés et repère de lecture. **P0.**

**4.10 — Renommage puis redémarrage** : après 4.9, fermer et relancer les deux
instances. Tout doit être identique. Vérifie que la migration en base a suivi.

# 5. Réactions

| # | Test | Actions | Attendu | P |
|---|---|---|---|---|
| 5.1 | Ajout | Réagir à un message | Visible des deux côtés | P0 |
| 5.2 | Bascule | Recliquer le même emoji | Retiré des deux côtés | P0 |
| 5.3 | Cumul | Alice et bob même emoji | Compteur à 2 | P1 |
| 5.4 | Persistance | Redémarrer | Réactions conservées | P0 |
| 5.5 | Plafond | Empiler > 32 emojis distincts sur un message | Plafonné, pas de croissance infinie | P2 |

# 6. Médias

| # | Test | Actions | Attendu | P |
|---|---|---|---|---|
| 6.1 | Image | Envoyer un PNG/JPG | Vignette cliquable, visionneuse plein écran | P0 |
| 6.2 | Fichier | Envoyer un PDF | Carte fichier téléchargeable | P0 |
| 6.3 | Dossier | Envoyer un dossier | Reçu en `.zip` ouvrable | P1 |
| 6.4 | Dossier illisible | Dossier contenant un fichier sans droit de lecture | **Erreur remontée** — pas de ZIP silencieusement incomplet | P1 |
| 6.5 | Téléchargement | Télécharger un média reçu | Écrit dans Téléchargements, **sans geler la fenêtre** | P0 |
| 6.6 | Nom en double | Télécharger deux fois | Suffixe « (1) », rien d'écrasé | P1 |
| 6.7 | Nom hostile | Pair envoyant `../../evil.txt` | Écrit dans Téléchargements uniquement, nom réduit | P0 |
| 6.8 | **Sous le seuil** | Envoyer un fichier de ~10 Mo | Accepté **sans confirmation** | P0 |
| 6.9 | **Au-dessus du seuil** | Envoyer un fichier > 50 Mo | Bandeau d'acceptation chez le destinataire | P0 |
| 6.10 | Refus | Refuser l'offre | Annoté « refusé » chez l'émetteur, rien écrit | P0 |
| 6.11 | Interruption | Fermer l'émetteur en cours de transfert | Carte « interrompu », **aucun fichier partiel** dans `media/` | P0 |
| 6.12 | Envoi massif | Sélectionner 50 fichiers d'un coup | **Interface fluide**, envois séquencés. Régression : un thread OS par fichier | P1 |

> 6.8 et 6.9 encadrent le seuil abaissé de 1 Gio à 50 Mo.

# 7. GIF et emojis

| # | Test | Actions | Attendu | P |
|---|---|---|---|---|
| 7.1 | Sélecteur | Ouvrir GIF / Mèmes / Stickers | Trois onglets peuplés | P1 |
| 7.2 | Recherche | Chercher un terme | Résultats, pagination au défilement | P1 |
| 7.3 | **Envoi et rendu** | Envoyer un GIF | **S'anime chez le destinataire** | P0 |
| 7.4 | Sans clé API | Retirer `ABCOM_KLIPY_API_KEY` | Message clair, pas de crash | P2 |
| 7.5 | Emojis | Sélecteur + `:shortcode` | Insertion et complétion | P1 |

> **7.3 est critique.** Le filtre d'URL n'autorise que `https://*.klipy.com` ;
> le CDN observé est `static.klipy.com`, donc couvert. Si un GIF s'affiche en
> carte fichier au lieu de s'animer, c'est que Klipy a changé de domaine :
> l'ajouter à `ALLOWED_MEDIA_URL_HOSTS` dans `src/message/media.rs`.

# 8. Markdown et liens

| # | Test | Actions | Attendu | P |
|---|---|---|---|---|
| 8.1 | Formatage | `**gras**`, `_italique_`, `~~barré~~`, `` `code` `` | Rendus | P1 |
| 8.2 | Blocs | Bloc de code, citation, listes, tableau GFM | Rendus | P1 |
| 8.3 | Lien légitime | `[site](https://example.com)` | Cliquable | P1 |
| 8.4 | **Schéma hostile** | Recevoir `[Rapport](file:///etc/passwd)` puis `[doc](smb://serveur/partage)` | **Affichés en texte brut, non cliquables** | P0 |

# 9. Conversations et historique

| # | Test | Actions | Attendu | P |
|---|---|---|---|---|
| 9.1 | Non-lus | Recevoir sur une conversation fermée | Badge incrémenté, remis à zéro à l'ouverture | P0 |
| 9.2 | Épinglage | Épingler un pair et un salon | Remontent en tête, persistent après redémarrage | P2 |
| 9.3 | Sourdine | Mettre un salon en muet | Aucun son ni notification pour lui | P2 |
| 9.4 | Recherche | Rechercher un terme | Résultats, navigation vers le message | P1 |
| 9.5 | Pagination | Remonter dans un long historique | Charge par pages de 100 | P0 |
| 9.6 | **Pagination après débordement** | Générer > 600 messages dans une session (`scripts/seed-demo.py`), puis **remonter** | **Le chargement continue.** Régression : il s'arrêtait définitivement | P0 |
| 9.7 | Effacer l'historique | Effacer une conversation | Messages **et** leurs réactions/accusés partent immédiatement | P1 |
| 9.8 | Effacement ciblé | Après 9.7, vérifier une autre conversation | Intacte | P0 |
| 9.9 | Export | Exporter une conversation | Fichier `.txt` écrit, notification **après** écriture réelle | P1 |
| 9.10 | Export impossible | Exporter vers un dossier en lecture seule | **Erreur affichée**, pas de faux succès | P1 |

# 10. Sécurité

| # | Test | Actions | Attendu | P |
|---|---|---|---|---|
| 10.1 | Chiffrement | Observer le trafic (`tcpdump`, port 9000) | Aucun texte lisible | P0 |
| 10.2 | TOFU | Première connexion | Clé épinglée, empreinte visible dans Paramètres | P0 |
| 10.3 | **Changement de clé** | Supprimer `identity.key` de bob, relancer | Alice **refuse** et alerte sur l'usurpation possible | P0 |
| 10.4 | Ré-appairage | Accepter explicitement la nouvelle clé | Connexion rétablie | P1 |
| 10.5 | Passphrase | Lancer deux instances avec des passphrases différentes | **Aucune connexion** | P0 |
| 10.6 | Passphrase identique | Même passphrase des deux côtés | Connexion normale, démarrage < 1 s | P0 |
| 10.7 | Permissions de la clé | `ls -l identity.key` (Unix) | `-rw-------` (0600) | P0 |
| 10.8 | Permissions héritées | `chmod 644 identity.key`, relancer | **Resserré à 0600 au chargement** | P1 |
| 10.9 | Version de protocole | Un pair en version antérieure | Connexion refusée proprement | P0 |
| 10.10 | Usurpation de pseudo | Deux instances du même pseudo | Pas de mélange d'historique | P1 |

> 10.6 vérifie que l'étirement de la passphrase (~30 ms en release) ne se voit
> pas. En build debug il prend ~1 s : c'est normal, ne pas tester en debug.

# 11. Persistance et robustesse

| # | Test | Actions | Attendu | P |
|---|---|---|---|---|
| 11.1 | Redémarrage | Fermer et relancer | Historique, groupes, alias, avatars, accusés conservés | P0 |
| 11.2 | **Base non inscriptible** | `chmod 444 abcom.db`, écrire, quitter | Erreur journalisée `historique non sauvegardé` — pas de silence | P0 |
| 11.3 | File hors ligne | Bob éteint, alice envoie, bob revient | Le message est livré | P0 |
| 11.4 | **File durable** | Bob éteint, alice envoie, **tuer alice brutalement** (`kill -9`), relancer alice, rallumer bob | Le message part quand même. Régression : il était perdu | P0 |
| 11.5 | Compaction | Paramètres → compacter | Base réduite, aucune perte | P2 |
| 11.6 | Cache média | Laisser tourner > 15 min avec beaucoup de médias | Aucun fichier ne disparaît : plus aucune purge automatique | P1 |
| 11.7 | Purge manuelle | Paramètres → Stockage, saisir une durée, purger | L'aperçu annonce le volume, seuls les fichiers plus vieux partent, les images du fil sont épargnées si la case est décochée | P1 |
| 11.8 | Envoi sans doublon | Envoyer un fichier, regarder `media/` chez l'émetteur | Aucune copie créée ; l'original reste seul sur le disque | P2 |
| 11.7 | Instance hors plage | `ABCOM_INSTANCE=999999` | Démarre sur un port valide, **sans panique** | P2 |

## 11.8 — Migration depuis une version antérieure (P0)

À faire **une fois**, avec une vraie base d'avant la refonte des groupes.

1. Partir d'une installation antérieure contenant un salon, des messages, des
   réactions et des accusés.
2. Sauvegarder le répertoire de données.
3. Installer cette version, lancer.

**Attendu** : historique, réactions, accusés et repères de lecture intacts, le
salon garde son nom. Ensuite, appliquer 4.9 (renommage) : tout doit tenir.

> ⚠️ **Le protocole passe en version 3.** Un pair resté en version antérieure ne
> se connectera plus. Le déploiement doit se faire sur toutes les machines en
> même temps.

# 12. Interface et système

| # | Test | Actions | Attendu | P |
|---|---|---|---|---|
| 12.1 | Thèmes | Basculer clair / sombre / système | Tout reste lisible, aucun texte invisible | P1 |
| 12.2 | Langue | Basculer FR / EN | Tous les libellés suivent | P1 |
| 12.3 | Avatar | Choisir, changer, retirer | Propagé aux pairs | P1 |
| 12.4 | Alias | Renommer un contact | Local, persistant | P2 |
| 12.5 | Tray | Fermer la fenêtre | Repli dans le tray, l'app tourne | P0 |
| 12.6 | Restauration | Cliquer l'icône du tray | Fenêtre restaurée, état intact | P0 |
| 12.7 | Notification native | Message reçu fenêtre repliée | Notification système | P1 |
| 12.8 | Son | Message reçu hors conversation active | Son joué ; silence si conversation ouverte | P2 |
| 12.9 | Badge | Messages non lus | Badge sur l'icône du tray | P2 |
| 12.10 | Autostart | Activer, redémarrer la session | L'app démarre | P2 |
| 12.11 | Redimensionnement | Fenêtre très étroite / très large | Pas de chevauchement | P1 |

---

# 13. Spécificités par OS

Le cœur est identique partout ; ces points diffèrent réellement.

## Linux

| # | Test | Attendu |
|---|---|---|
| L1 | `scripts/abcom-install.sh` | Binaire, service systemd et **raccourci menu fonctionnel** |
| L2 | **Lancer depuis le menu** | L'app démarre. Régression : `%h` n'était pas développé, le raccourci ne lançait rien |
| L3 | Démarrage unique | Après installation puis reconnexion de session : **une seule** instance tourne. `systemctl --user status abcom` doit répondre que l'unité n'existe pas — le démarrage passe uniquement par `~/.config/autostart/` |
| L6 | Build minimal | `cargo build --release --no-default-features` sur une machine **sans ALSA** : doit compiler. L'app démarre sans tray (fermer la fenêtre quitte) et sans bip |
| L8 | Tray sans GTK | Sur une machine **sans `libgtk-3`, `libayatana-appindicator3` ni `libxdo`** : `ldd target/release/abcom` ne doit citer aucun des trois, et le tray doit fonctionner malgré tout (KDE/XFCE, ou GNOME + extension) |
| L7 | Alpine / musl | Installer les paquets de [docs/06](06-installation.md), compiler, lancer. Binaire lié dynamiquement à musl |
| L4 | Wayland et X11 | Tester les deux : tray, notifications, focus (test 3.3) |
| L5 | Tray selon l'environnement | GNOME demande une extension AppIndicator ; KDE/XFCE fonctionnent nativement |

> L3 vérifie la correction du double autostart : les installateurs ne posent
> plus de service systemd, le démarrage est géré par l'application seule
> (entrée XDG, réglable dans Paramètres → Général).

## macOS

| # | Test | Attendu |
|---|---|---|
| M1 | Repli | Disparaît du Dock, reste dans la barre de menus |
| M2 | Restauration | Revient dans le Dock |
| M3 | Découverte locale | Deux instances sur la même machine se voient (multicast loopback) |
| M4 | Permissions réseau | Le pare-feu demande l'autorisation au premier lancement |
| M5 | Focus (test 3.3) | Cliquer une fenêtre en arrière-plan déclenche l'accusé de lecture |
| M6 | Permissions de la clé | `stat -f "%Sp" identity.key` → `-rw-------` |

## Windows

| # | Test | Attendu |
|---|---|---|
| W1 | Repli | Sort de la barre des tâches, **le menu du tray reste actif** |
| W2 | Restauration | Fenêtre replacée à l'écran |
| W3 | ACL de la clé | `icacls identity.key` → accès limité à l'utilisateur courant |
| W4 | Pare-feu | Autoriser au premier lancement, puis vérifier la découverte |
| W5 | Chemins avec espaces | Envoyer un fichier depuis un dossier au nom espacé |
| W6 | Focus (test 3.3) | Idem, avec la fenêtre restaurée depuis le tray |

---

# 14. Passe minimale avant livraison

Si le temps manque, ces tests seuls :

1. **1.1** découverte — **2.1** message privé — **2.3** salon
2. **3.3** accusé de lecture au retour du focus
3. **4.9** renommage de salon *(le plus important)*
4. **6.1**, **6.8**, **6.9** médias et seuil
5. **7.3** rendu d'un GIF
6. **10.3** changement de clé — **10.5** passphrase
7. **11.1** redémarrage — **11.8** migration
8. **12.5 / 12.6** tray, sur chaque OS cible

# 15. Ce que la suite automatique couvre déjà

Inutile de le refaire à la main : hachage des messages, dérivation et migration
des identifiants de salon, filtrage d'URL de médias, schémas de liens markdown,
péremption des pairs, bornes des réactions, durabilité de la file d'envoi,
pagination après débordement, purge en cascade, framing et handshake Noise,
signature des annonces, path traversal.

```bash
make test          # 329 tests
cargo test --release   # à lancer avant livraison : le debug fausse les temps
```
