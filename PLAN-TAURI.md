# Plan d'evaluation automatisee : egui face a Tauri/Svelte

> Plan de travail pour determiner, par une implementation et des mesures
> reproductibles, si Abcom doit conserver son interface egui ou migrer vers
> Tauri 2, Svelte 5, TypeScript et Vite.
>
> Redige le 5 aout 2026. Ce document ne constitue pas encore une decision de
> migration. L'interface egui reste la reference fonctionnelle jusqu'au rapport
> comparatif final.

## 1. Objectif et principe de decision

Abcom fonctionne aujourd'hui, mais l'interface egui devient couteuse a faire
evoluer pour une messagerie riche : GIF animes, medias, Markdown, selection de
texte, composeur, virtualisation et stabilite du scroll.

Le but n'est pas de choisir Tauri sur une impression. Il faut :

1. mesurer automatiquement la stack egui actuelle ;
2. construire un prototype equivalent avec Tauri/Svelte/TypeScript/Vite ;
3. rejouer exactement les memes scenarios sur les deux stacks ;
4. produire un rapport chiffre et prendre une decision selon des seuils fixes.

La sobriete en arriere-plan est une contrainte bloquante. Une meilleure
interface visible ne justifie pas une application qui gene l'ordinateur quand
elle est repliee dans le tray.

## 2. Regles d'execution

- Tout le travail de ce plan est realise exclusivement dans la branche
  `dev-tauri`, creee une seule fois depuis `dev` a jour. Le harnais et les
  scripts d'execution refusent de modifier le projet si la branche active n'est
  pas `dev-tauri`.
- Aucun commit de l'experimentation Tauri ne doit etre applique directement sur
  `dev` ou `main`. La branche `dev` reste la reference egui intacte jusqu'a la
  decision finale.
- L'ensemble du protocole doit etre lance par une commande unique, sans clic,
  saisie, preparation de base ou relevé manuel demande a l'utilisateur.
- Les donnees de benchmark vivent dans un repertoire temporaire. La base, les
  medias, l'identite et les preferences de l'utilisateur ne sont jamais lus ni
  modifies.
- Les deux interfaces utilisent le meme coeur Rust, la meme base SQLite, les
  memes donnees, la meme taille de fenetre et les memes scenarios.
- Les builds mesures sont des builds `release`, jamais les serveurs de
  developpement Vite ou les builds Rust debug.
- Les medias du benchmark sont locaux afin d'eliminer le CDN, le debit Internet
  et le cache reseau des mesures.
- Chaque scenario est chauffe une fois, puis execute au moins cinq fois. Le
  rapport retient la mediane, le p95 et le pic plutot qu'une mesure unique.
- Le harnais refuse une mesure si la charge de fond de la machine depasse un
  seuil defini. Il attend et recommence automatiquement au lieu de conserver un
  resultat pollue.
- Les processus enfants du WebView sont inclus dans la RAM, le CPU et le nombre
  de threads de Tauri.
- Aucun `sudo` ne doit etre requis. Une mesure indisponible sans privilege est
  marquee `non disponible` et n'est pas remplacee par une estimation.
- Aucun push, merge ou remplacement de l'interface actuelle n'est effectue sans
  decision explicite apres le rapport final.
- L'interface existante est la specification de reference. Tauri ne doit
  introduire aucun redesign, simplification visuelle ou changement
  d'interaction pendant cette evaluation.

## 3. Livrables prevus

L'implementation du plan devra aboutir a cette organisation ou a un equivalent
justifie :

```text
benchmarks/
  fixtures/             donnees et medias deterministes
  scenarios/            description commune des parcours mesures
  scripts/              orchestration et echantillonnage par OS
  results/              sorties JSON ignorees par Git
  schema/               format stable des resultats
web/                     frontend Svelte 5 + TypeScript + Vite
src-tauri/               adaptateur et shell Tauri 2
scripts/benchmark-ui     point d'entree unique
BENCHMARK-REPORT.md      rapport comparatif genere
```

Les chemins definitifs pourront etre ajustes au moment de l'implementation,
mais une seule source de scenarios et un seul format de resultats doivent etre
conserves.

## Partie A - Etablir la reference egui automatiquement

### A1. Geler l'etat de reference

Avant toute modification structurelle :

1. enregistrer le commit, l'OS, l'architecture, le nombre de coeurs, la RAM et
   les versions Rust dans les metadonnees du benchmark ;
2. executer `cargo fmt --all --check` ;
3. executer `cargo clippy --all-targets -- -D warnings` ;
4. executer `cargo test` ;
5. executer `cargo build --release` ;
6. conserver le nombre de tests, la duree du build et la taille du binaire.

La campagne s'arrete immediatement si cette barriere n'est pas verte. Le
prototype Tauri ne doit jamais etre compare a une reference deja cassee.

### A2. Isoler les donnees de test

Ajouter une surcharge de configuration `ABCOM_DATA_DIR`, prioritaire uniquement
quand elle est explicitement fournie. Le comportement de production de
`config::data_dir()` reste inchange sans cette variable.

Le harnais cree un repertoire temporaire par execution et y genere :

- une base SQLite initialisee par le code de stockage Abcom ;
- 10 000 messages deterministes repartis entre public, prive et groupes ;
- du texte court, long, multiligne et Unicode ;
- du Markdown, des citations, des reactions et des accusés ;
- des images de dimensions connues ;
- des WebP ou GIF animes locaux de petite, moyenne et grande taille ;
- des transferts simules avec progression ;
- des compteurs non lus et des pairs en ligne/hors ligne.

Les fixtures ne doivent pas dependre de `scripts/seed-demo.py`, qui modifie les
instances de demonstration existantes. Un generateur dedie produit toujours le
meme contenu a partir d'une graine fixe.

### A3. Creer un mode benchmark non interactif

L'application egui doit pouvoir executer un scenario sans controle externe de
la souris. Ce mode est reserve aux builds de benchmark et ne change pas le
comportement normal.

Il doit :

- ouvrir une conversation cible ;
- faire defiler le fil selon une trajectoire et une vitesse fixes ;
- injecter un lot de messages via les memes chemins applicatifs que le reseau ;
- ouvrir puis fermer le picker GIF ;
- afficher 1, 5 puis 20 animations dans la fenetre visible ;
- masquer l'application dans le tray ;
- attendre la destruction ou purge attendue des ressources ;
- simuler des messages recus pendant le repli ;
- rouvrir l'interface ;
- ecrire des jalons horodates dans un fichier JSONL ;
- quitter proprement et vider le stockage.

Le mode benchmark ne doit pas court-circuiter le rendu, la pagination, les
caches ou la logique de cycle de vie que l'on cherche a mesurer.

### A4. Ajouter un echantillonneur externe

Un processus independant echantillonne l'application et ses descendants. Des
adaptateurs macOS, Linux et Windows exposent le meme schema JSON.

Mesures obligatoires :

| Mesure | Valeur conservee |
|---|---|
| CPU | moyenne, mediane, p95 et maximum |
| Memoire residente | mediane, p95, maximum et valeur apres repli |
| Processus et threads | minimum, maximum, details des descendants |
| Demarrage | processus lance vers premiere interface utilisable |
| Reouverture | clic tray simule vers interface utilisable |
| Temps de frame | mediane, p95, p99 et frames superieures a 16/33 ms |
| Activite de rendu cachee | nombre de frames/repaints apres stabilisation |
| Disque | octets lus/ecrits pendant le scenario si l'OS l'expose |
| Taille livrable | binaire et package installable |

La mesure GPU n'est obligatoire que si elle est disponible automatiquement
sans privilege. Dans tous les cas, le mode cache doit rapporter zero frame
produite par l'interface apres sa periode de grace.

### A5. Scenarios de reference

| ID | Scenario | Duree minimale | Signal principal |
|---|---|---:|---|
| E0 | Demarrage a froid | jusqu'a `ui-ready` | latence, RSS de depart |
| E1 | Fenetre visible au repos | 60 s | CPU, frames, wakeups |
| E2 | Scroll de 10 000 messages | parcours fixe | fluidite, p95 frame |
| E3 | Reception de 100 messages | rafale puis repos | latence, stabilite |
| E4 | 1, 5 et 20 GIF visibles | 60 s par niveau | CPU, RAM, frames |
| E5 | Picker GIF ouvert puis ferme | 3 pages locales | liberation memoire |
| E6 | Repli dans le tray | 180 s apres grace | CPU/RAM cachees |
| E7 | Messages pendant le repli | inclus dans E6 | coeur toujours actif |
| E8 | Vingt reouvertures | cycle masque/reouvre | mediane et p95 |
| E9 | Pagination vers le passe | 20 pages | scroll stable, I/O |

Le benchmark complet peut prendre plusieurs minutes. Une variante `--quick`
execute une seule repetition pour le developpement, mais seul le mode complet
est recevable pour la decision.

### A6. Sortie de la partie A

La commande unique doit produire :

```text
benchmarks/results/<date>-<commit>/egui.raw.json
benchmarks/results/<date>-<commit>/egui.summary.json
benchmarks/results/<date>-<commit>/environment.json
```

La partie A est terminee lorsque deux campagnes consecutives donnent des
medians coherentes a plus ou moins 10 %. Si ce n'est pas le cas, le harnais est
considere instable et doit etre corrige avant de commencer Tauri.

## Partie B - Implementer le prototype Tauri/Svelte

### B1. Perimetre du prototype

Le prototype doit reproduire l'interface actuelle aussi strictement que le
permet la difference entre egui et un WebView. Tout le travail deja investi dans
l'UI est conserve : structure, densite, dimensions, couleurs, typographie,
espacements, icones, hierarchie, libelles, etats, animations et interactions.
Il ne s'agit pas de profiter de Tauri pour redessiner l'application.

La parite couvre au minimum :

- demarrage et restauration de l'etat ;
- sidebar avec pairs, groupes et non-lus ;
- fil public, prive et groupe ;
- pagination des anciens messages ;
- texte, Unicode, Markdown, citations et reactions ;
- images et GIF/WebP animes ;
- composeur texte avec envoi ;
- reception de messages et progression de transfert ;
- tray, repli, notification et reouverture ;
- themes clair et sombre ;
- parametres, profil et avatar ;
- picker emoji et picker GIF/memes/stickers ;
- creation et gestion des groupes ;
- modales, confirmations et menus contextuels ;
- visionneuse media, pieces jointes et etats de transfert ;
- versions francaise et anglaise ;
- raccourcis clavier, focus, selection et comportement du scroll.

Aucun ecran secondaire ne peut etre supprime sous pretexte qu'il n'intervient
pas dans le benchmark de performance. Une premiere tranche peut etre mesuree
pendant le developpement, mais la comparaison finale de la partie C ne commence
qu'une fois la matrice de parite complete.

### B1 bis. Verrouiller la parite visuelle et comportementale

Avant d'implementer les composants Svelte, le harnais capture automatiquement
l'interface egui de reference avec les fixtures deterministes.

Captures minimales :

- fil public, prive et groupe ;
- messages courts, longs, Markdown, citation, reactions, image et GIF ;
- sidebar vide, peuplee, avec non-lus et pair hors ligne ;
- composeur vide, multilignes, avec selection et piece jointe ;
- picker emoji et les trois onglets Klipy ;
- parametres, profil, themes clair/sombre et langues FR/EN ;
- creation et gestion d'un groupe ;
- visionneuse media et progression de transfert ;
- toutes les modales de confirmation et tous les etats d'erreur visibles.

Chaque scene est capturee au minimum dans la taille historique `860x600`, puis
dans une taille plus large et dans la taille minimale supportee. Les contenus
variables comme l'heure, le curseur clignotant et la frame courante d'un GIF
sont figes ou masques de maniere identique dans les deux stacks.

La verification combine :

- comparaison perceptuelle des captures egui/Tauri ;
- assertions de geometrie sur les zones principales ;
- verification des couleurs, polices, tailles et espacements issus de tokens
  communs documentes ;
- scenarios automatiques de clic, clavier, scroll, hover, focus et fermeture ;
- inventaire de tous les libelles francais et anglais ;
- matrice des etats visibles et de leur transition.

La cible est la parite pixel et comportementale. Une tolerance technique est
admise uniquement pour l'anticrenelage du texte et les differences de rendu
natives entre egui et le WebView. Toute difference intentionnelle, meme jugee
meilleure, doit etre exclue de cette branche ou approuvee explicitement avant
d'etre ajoutee.

### B2. Conserver un coeur Rust unique

Extraire ce qui est necessaire du binaire actuel vers une bibliotheque Rust
reutilisable, sans dupliquer le protocole ni le stockage.

La frontiere cible est :

```text
abcom-core
  etat, stockage, reseau, Noise, medias, controleur applicatif
        |                                  |
adaptateur egui                     adaptateur Tauri
        |                                  |
interface egui                     interface Svelte
```

La logique metier actuellement dans `src/ui/events.rs`, notamment les ACK,
recus de lecture, groupes, notifications et transferts, doit rejoindre un
controleur independant de l'interface. Les deux interfaces consomment ensuite
les memes actions et evenements.

Barriere obligatoire apres chaque extraction :

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### B3. Initialiser la stack frontend

Creer une application locale, sans SSR ni serveur applicatif :

- Tauri 2 pour la fenetre, le tray et le cycle de vie ;
- Svelte 5 pour les composants ;
- TypeScript en mode strict ;
- Vite pour le build ;
- `svelte-check` pour les controles statiques ;
- Vitest pour les tests unitaires ;
- Playwright pour les tests navigateur du frontend ;
- une bibliotheque de virtualisation Svelte/TanStack seulement si le prototype
  prouve qu'elle est necessaire.

Ne pas ajouter SvelteKit, SSR, Tailwind ou une bibliotheque globale d'etat au
demarrage. Le prototype doit rester petit, inspectable et rapide a charger.

### B4. Definir un pont Tauri type

Rust reste la source de verite. Le frontend ne doit ni ouvrir SQLite ni acceder
librement au systeme de fichiers.

Commandes minimales :

```text
bootstrap
select_conversation
load_older_messages
send_message
send_reaction
mark_conversation_read
hide_window
```

Evenements minimaux :

```text
message-received
message-updated
peer-updated
group-updated
reaction-updated
transfer-progress
unread-updated
```

Les DTO Rust sont serialisables, versionnes et verifies par des tests de contrat
TypeScript. Aucun type egui ou Tauri ne doit entrer dans `abcom-core`.

### B5. Construire l'interface representative

Ordre d'implementation :

1. squelette de fenetre et bootstrap depuis Rust ;
2. sidebar et selection de conversation ;
3. fil virtualise et pagination de 100 messages ;
4. rendu Markdown assaini avant insertion dans le DOM ;
5. images avec dimensions reservees pour eviter les sauts de layout ;
6. GIF/WebP via les balises natives du WebView ;
7. composeur et envoi ;
8. reactions, citations et mises a jour incrementales ;
9. transfert et notifications ;
10. cycle de vie tray et restauration ;
11. parametres, profil, groupes, visionneuse et modales restantes ;
12. passage complet de la matrice de parite visuelle et comportementale.

Le frontend ne recoit jamais les 10 000 messages en une fois. Il recoit une
fenetre recente puis les pages demandees. L'arrivee d'un message ne doit mettre
a jour que la conversation et les compteurs concernes.

### B6. Traiter explicitement le mode resident

Le comportement cible est le suivant :

1. la croix masque immediatement la fenetre ;
2. une periode de grace configurable, initialement 20 secondes, conserve le
   WebView pour une reouverture instantanee ;
3. apres la grace, le WebView est detruit ;
4. le coeur Rust, le reseau, SQLite, le tray et les notifications restent actifs ;
5. un clic sur le tray recree le WebView ;
6. le frontend rappelle `bootstrap` et retrouve la conversation, les brouillons
   et les non-lus necessaires ;
7. aucun evenement n'est emis vers une fenetre absente.

Les GIF, timers, observers et listeners frontend doivent tous disparaitre avec
le WebView. Aucun polling JavaScript n'est autorise en arriere-plan.

### B7. Securite minimale du prototype

- CSP restrictive, avec uniquement les origines Klipy necessaires en production.
- Medias du disque servis par un protocole ou une portee Tauri limitee au
  repertoire media Abcom.
- Markdown assaini ; HTML brut interdit par defaut.
- Aucune permission shell.
- Aucune permission filesystem globale exposee au frontend.
- Validation Rust de chaque argument recu par une commande.
- Pas de secret, cle Noise ou chemin arbitraire envoye au frontend.

### B8. Tests automatiques de la stack Tauri

Barriere frontend :

```bash
npm ci
npm run format:check
npm run lint
npm run check
npm run test
npm run build
```

Barriere Rust/Tauri :

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo tauri build
```

Couverture attendue :

- tests unitaires Svelte pour messages, Markdown, reactions et compteurs ;
- tests de virtualisation et pagination ;
- tests du pont TypeScript avec l'API Tauri simulee ;
- tests Rust des commandes et du controleur sans WebView ;
- tests Playwright contre le build Vite pour le scroll, le composeur et les GIF ;
- test du processus Tauri reel pour bootstrap, repli, destruction et recreation ;
- tests de regression visuelle sur toutes les scenes de B1 bis ;
- tests de geometrie et d'interaction garantissant que l'UI ne derive pas de la
  reference egui ;
- `tauri-driver` sur les plateformes ou le WebDriver Tauri est disponible ;
- aucun clic manuel requis sur macOS si le WebDriver natif ne pilote pas le
  WebView : le mode benchmark commande alors le cycle de vie depuis Rust et
  verifie ses jalons.

## Partie C - Rejouer et comparer

### C1. Rejouer exactement les scenarios

La partie C est bloquee tant que l'inventaire fonctionnel et la matrice de
parite de B1/B1 bis ne sont pas entierement verts. Les performances ne doivent
pas etre ameliorees artificiellement en comparant une interface Tauri incomplete
a l'application egui complete.

Le meme orchestrateur lance alternativement egui et Tauri pour limiter les
effets de temperature et de charge de la machine :

```text
egui -> Tauri -> Tauri -> egui -> egui -> Tauri ...
```

Pour chaque stack :

- meme executable coeur et meme revision du protocole ;
- copie neuve de la meme fixture ;
- meme resolution et meme theme ;
- meme temps de grace avant destruction des ressources ;
- meme duree de scenario ;
- meme echantillonneur externe ;
- aucune requete Internet.

Si l'une des stacks echoue fonctionnellement, ses mesures de performance sont
invalidees. Une interface rapide mais incorrecte ne peut pas gagner.

### C2. Seuils bloquants

Tauri est rejete ou retravaille avant toute migration si un seul de ces seuils
est manque de maniere reproductible :

| Critere | Seuil Tauri |
|---|---|
| CPU cachee apres destruction | <= 0,5 % et au plus +0,2 point face a egui |
| Frames frontend cachees | 0 apres la periode de grace |
| RSS cachee agregee | <= max(180 Mo, egui + 20 %) |
| Reouverture mediane | <= 500 ms |
| Reouverture p95 | <= 1 000 ms |
| Reception fenetre detruite | 100 % des messages stockes et notifies |
| Perte ou corruption de donnees | 0 |
| Tests bloquants | 100 % verts |
| Parite visuelle et comportementale | 100 % des scenes et scenarios valides |

Ces valeurs sont des budgets initiaux. Le rapport affiche aussi les valeurs
absolues egui afin qu'un seuil trop permissif ne masque pas une regression nette.

### C3. Criteres notes

Une fois les seuils bloquants franchis, la decision utilise cette ponderation :

| Axe | Poids | Signaux |
|---|---:|---|
| Sobriete en arriere-plan | 35 % | CPU, RSS, threads, absence de rendu |
| Contenu riche | 25 % | GIF, Markdown, medias, stabilite du layout |
| Reactivite visible | 15 % | frame p95, scroll, rafales, pagination |
| Fiabilite et testabilite | 15 % | tests, erreurs, contrats, reproductibilite |
| Demarrage et distribution | 10 % | lancement, reouverture, taille package |

La quantite de code n'est pas une mesure de performance, mais le rapport doit
indiquer les lignes specifiques a l'UI, le nombre de dependances directes et les
contournements necessaires dans chaque version.

### C4. Rapport genere

La commande finale genere `BENCHMARK-REPORT.md` avec :

- environnement et commits mesures ;
- resultat des barrieres de tests ;
- tableau egui/Tauri pour chaque scenario ;
- graphiques CPU/RSS dans le temps ;
- temps de demarrage et reouverture ;
- comportement avec 1, 5 et 20 GIF ;
- comportement pendant trois minutes sans fenetre ;
- regressions fonctionnelles eventuelles ;
- limites ou mesures indisponibles ;
- verdict automatique selon les seuils ;
- recommandation technique finale separee du verdict automatique.

Les donnees brutes restent disponibles pour verifier le rapport. Le generateur
ne doit permettre aucune correction manuelle des resultats.

## Partie D - Decision apres les mesures

### D1. Si Tauri est retenu

1. garder egui fonctionnel pendant la recherche de parite ;
2. porter groupes, reglages, fichiers, avatars et pickers restants ;
3. reutiliser tous les tests du coeur Rust ;
4. ajouter une matrice CI macOS, Windows et Linux ;
5. valider les packages et signatures ;
6. migrer sans changer le chemin de `abcom.db` ;
7. supprimer egui uniquement apres parite et nouvelle campagne verte ;
8. conserver le benchmark comme test de non-regression de performance.

### D2. Si Tauri est rejete

1. conserver le coeur extrait et le harnais de benchmark ;
2. retirer le prototype dans un diff distinct, sans toucher aux mesures ;
3. utiliser le rapport pour cibler les problemes egui les plus couteux ;
4. evaluer ensuite `wgpu`, Slint ou Qt uniquement avec le meme protocole ;
5. ne pas relancer une autre migration sans hypothese mesurable nouvelle.

## 4. Ordre d'execution et barrieres

```text
A1-A2  reference verte et donnees isolees
   |
A3-A6  benchmark egui stable
   |
B1-B4  coeur partage et squelette Tauri
   |
B5-B8  tranche verticale et tests verts
   |
C1-C4  campagne croisee et rapport
   |
D1/D2  decision humaine sur resultats automatiques
```

Il est interdit de commencer la partie B si la reference egui n'est pas stable.
Il est interdit de prendre une decision sur Tauri si les tests ou le harnais
d'une des deux stacks sont rouges. Toutes les phases A a C sont executees depuis
`dev-tauri`; `dev` sert uniquement de reference et ne recoit aucune modification.

## 5. Definition de termine

Le plan est execute lorsque :

- une commande unique construit, teste et mesure les deux stacks ;
- toutes les modifications ont ete realisees dans `dev-tauri`, creee depuis
  `dev`, sans modification directe de `dev` ou `main` ;
- aucune action manuelle ni donnee utilisateur n'est necessaire ;
- le coeur Rust est commun aux deux interfaces ;
- tous les scenarios E0 a E9 produisent des resultats comparables ;
- les processus WebView enfants sont inclus dans les mesures ;
- l'interface Tauri est visuellement et comportementalement equivalente a
  l'interface egui, sans redesign implicite ;
- la reception continue lorsque le WebView est detruit ;
- `BENCHMARK-REPORT.md` est genere automatiquement ;
- les seuils produisent un verdict non ambigu ;
- egui n'a pas ete supprime avant la decision finale.
