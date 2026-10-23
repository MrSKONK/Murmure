# Murmure

Dictée vocale push-to-talk pour Windows, 100 % locale (Faster-Whisper), sans abonnement ni API distante.
Maintenez le raccourci, parlez, relâchez : le texte est collé dans l'application active.

## Installation (recommandé)

Téléchargez `Murmure-Setup-x.y.z.exe` depuis la page des releases (dépôt public `MrSKONK/Murmure`) et lancez-le :
aucun droit administrateur n'est demandé, Murmure s'installe dans `%LOCALAPPDATA%\Programs\Murmure`, apparaît dans
le menu Démarrer et se désinstalle depuis les Paramètres de Windows.

**Mises à jour :** Murmure vérifie au démarrage puis toutes les 12 heures s'il existe une nouvelle version. Si oui,
une notification et un bandeau sur l'Accueil le signalent ; un clic sur « Mettre à jour » télécharge l'installateur,
vérifie son empreinte SHA-256, ferme Murmure, installe et relance. Réglages et historique sont conservés.
La vérification automatique se désactive dans Réglages → À propos et mises à jour.

## Publier une version

1. Augmentez `__version__` dans `src/version.py` et ajoutez une section `## x.y.z` dans `CHANGELOG.md`.
2. Fusionnez sur `main` : le workflow « Build Windows » construit `Murmure.exe` et l'installateur, les teste
   (installation silencieuse puis autotest), et publie la release `vx.y.z` dans `MrSKONK/Murmure`.

Mise en place, une seule fois : le code reste privé, les releases sont publiées dans un dépôt **public** dédié pour
que Murmure puisse les lire sans clé. Créez le dépôt public `MrSKONK/Murmure` avec un README (un dépôt vide ne peut
pas recevoir de release), puis un jeton GitHub *fine-grained* limité à ce dépôt avec la permission
« Contents : Read and write », et enregistrez-le comme secret `RELEASES_TOKEN` dans ce dépôt-ci
(Settings → Secrets and variables → Actions). Sans ce secret, la construction fonctionne mais rien n'est publié.

## Exécutable Windows (sans installateur)

Murmure peut tourner comme un vrai logiciel : `Murmure.exe`, sans Python ni ligne de commande.

**Le récupérer tout fait :** sur GitHub, onglet *Actions* → dernier « Build Windows » réussi → artefact
**Murmure-windows** (zip). Dézippez le dossier où vous voulez (par exemple `C:\Programmes perso\Murmure`)
et lancez `Murmure.exe`. Clic droit → « Épingler à la barre des tâches » ou « Épingler à l'écran de démarrage ».

**Ou le construire soi-même** (installation Python ci-dessous déjà faite) : double-cliquez sur `Construire.bat`.
Le résultat est dans `dist\Murmure\`. Gardez tout le dossier : `Murmure.exe` a besoin des fichiers à côté de lui.

Avec l'exécutable, les réglages et le dictionnaire sont dans `%APPDATA%\Murmure\settings.json`
(ils survivent aux mises à jour) ; le modèle Whisper reste dans `~\.cache\huggingface`.
Windows SmartScreen peut avertir au premier lancement, car l'exécutable n'est pas signé :
« Informations complémentaires » → « Exécuter quand même ».

## Installation (pour le développement)

Prérequis : Python 3.12 (testé avec 3.12.10).

```powershell
py -3.12 -m venv venv
.\venv\Scripts\python.exe -m pip install -r requirements.txt
```

Si le dossier du projet a été déplacé et que `venv\Scripts\python.exe` répond « Unable to create process »,
relancez simplement `py -3.12 -m venv venv` (les paquets sont conservés), puis la commande `pip install` ci-dessus.

## Lancement

Double-cliquez sur **`Lancer.bat`** : l'application démarre sans fenêtre de terminal et ouvre sa fenêtre
(Accueil, Dictionnaire, Réglages). Fermer la fenêtre ne quitte pas l'application : elle reste dans la zone
de notification, près de l'horloge (flèche ^ si l'icône est masquée). Un clic sur l'icône rouvre la fenêtre ;
clic droit puis « Quitter » pour l'arrêter. Relancer `Lancer.bat` alors qu'elle tourne réaffiche simplement la fenêtre.

Pour voir les messages de diagnostic, lancez plutôt depuis PowerShell, dans le dossier du projet :

```powershell
.\venv\Scripts\python.exe main.py
```

L'icône de la zone de notification indique l'état :

| Couleur | État |
|---|---|
| Gris | chargement du modèle |
| Violet | prêt à dicter |
| Rouge | enregistrement |
| Orange | transcription en cours |
| Rouge sombre | erreur (détail sur l'écran Accueil) |

Pendant l'écoute et la transcription, une pastille s'affiche en bas de l'écran ; elle ne prend jamais le focus.

**Premier lancement :** le modèle choisi (`medium` par défaut, ~1,5 Go) est téléchargé une fois dans
`~\.cache\huggingface`. Ensuite, tout fonctionne hors ligne.

**Interface :** fenêtre native Windows (pywebview + WebView2, déjà présent sur Windows 10/11), pas d'Electron.
L'interface elle-même pèse quelques dizaines de Mo ; l'essentiel de la mémoire va au modèle Whisper,
qu'on peut décharger après inactivité (Réglages).

## Réglages

Tout se règle depuis la fenêtre et s'enregistre aussitôt dans `config/settings.json` (`%APPDATA%\Murmure\settings.json` avec l'exécutable) :

| Réglage (clé) | Rôle |
|---|---|
| Touche de dictée (`hotkey`) | touche ou combinaison, capturée en cliquant puis en l'appuyant (défaut `²`) |
| Mode (`hotkey_mode`) | `hold` : maintenir pour parler ; `toggle` : un appui démarre, un second arrête |
| Langue (`whisper_language`) | `fr`, `en`, `es`, `de`, `it`, `pt`, `nl` ou `auto` |
| Modèle (`whisper_model`) | `small` (rapide), `medium` (équilibré), `large-v3-turbo` (précis et plus rapide que large-v3, non mesuré ici), `large-v3` (le plus précis, lent sur CPU) |
| Charge processeur (`cpu_threads`) | `0` = tous les cœurs ; `4` ou `2` laissent de la marge au reste du PC |
| Libérer la mémoire (`unload_after_min`) | décharge le modèle après N minutes sans dictée (`0` = jamais) |
| Entrée audio (`mic`) | micro choisi ; vide = micro par défaut de Windows |
| Pastille à l'écran (`show_overlay`) | indicateur en bas de l'écran |
| Lancer avec Windows (`autostart`) | démarre en arrière-plan à l'ouverture de session |
| Dictionnaire (`dictionary`) | mots soufflés à Whisper, avec les variantes « entendu comme » remplacées automatiquement |
| Correction (`remove_fillers`, `spoken_punctuation`, `spoken_addresses`, `short_no_period`, `fit_to_context`, `grammar_enabled`, `french_nbsp`) | voir ci-dessous |
| Garder l'historique (`history_enabled`) | enregistre chaque dictée dans `history.json`, à côté des réglages (défaut `true`) |
| Dictées conservées (`history_limit`) | 100, 500, 1 000 (défaut), 5 000 ou `0` = toutes |

Réglages sans interface, à modifier dans le fichier : `device` / `compute_type` (`cpu` / `int8` ; `cuda`
seulement après essai réel sur votre machine) et `log_transcripts` (`false` : le texte dicté n'est jamais écrit
dans la console).

## Historique

L'onglet **Historique** liste toutes les dictées, groupées par jour, avec recherche, bouton « Copier » et suppression
(à l'unité ou tout effacer). Elles sont enregistrées en clair dans `history.json`, sur ce PC uniquement :
`config/history.json` en développement (exclu de Git), `%APPDATA%\Murmure\history.json` avec l'exécutable.
Désactivez « Garder l'historique » dans les réglages pour ne plus rien enregistrer.

## Correction du texte

La correction est faite localement par des règles (`src/cleanup.py`), sans modèle de langue ni service externe :

- **Hésitations** : « euh », « heu », « hum » sont retirés avec la virgule qui les suit, ainsi que les mots
  répétés en hésitant (« le le chat »), sauf les répétitions correctes (« nous nous », « vous vous », « si si »).
- **Adresses** : « nathan arobase gmail point com » → `nathan@gmail.com`, « www point exemple point fr » →
  `www.exemple.fr` (« tiret », « tiret bas » acceptés). « le point com » reste du texte.
- **Messages courts** : une seule phrase de moins de 20 mots est collée sans point final.
- **Raccord au texte existant** : Murmure lit, via UI Automation de Windows, les quelques caractères avant le
  curseur (jamais dans un champ de mot de passe, rien n'est gardé). En milieu de phrase, la dictée commence par une
  minuscule (sauf mots du dictionnaire et sigles) et une espace est ajoutée si besoin. Si l'application n'expose pas
  ce texte, la dictée est collée telle quelle.
- **Ponctuation dictée** : « virgule », « point-virgule », « deux-points », « point d'interrogation »,
  « point d'exclamation », « points de suspension », « point final », « point à la ligne », « à la ligne »,
  « nouveau paragraphe », « ouvrez / fermez les guillemets », « ouvrez / fermez la parenthèse ».
  Le mot « point » seul n'est pas converti, car il est trop courant en français.
- **Typographie française** (langue `fr`, imposée ou détectée) : espace avant `; : ! ?` et dans les guillemets,
  sans toucher aux heures (`14:30`) ni aux adresses (`http://`).
- **Majuscules** : après une fin de phrase dictée et en début de texte.

- **Correcteur grammatical (facultatif)** : [Grammalecte](https://grammalecte.net) 2.3.0, logiciel libre (GPL v3)
  non fourni avec Murmure. Il est téléchargé une fois (6 Mo, empreinte SHA-256 vérifiée) dans le dossier des
  réglages quand on active l'option, puis fonctionne hors ligne (moins d'une seconde de chargement, quelques
  millisecondes par dictée). Seules les corrections adaptées à la dictée sont appliquées : accords, conjugaisons et
  confusions (« Ils mange » → « Ils mangent », « a » → « à », « Ces un » → « C'est un »), avec une seule suggestion,
  et seulement si elle se prononce comme ce qui a été dit. Les corrections sur lesquelles Grammalecte se contredit
  sont ignorées.

La ponctuation et les majuscules ordinaires viennent de Whisper lui-même ; les mots mal reconnus se corrigent avec le dictionnaire.

## Choix du modèle (mesures sur ce PC, CPU/int8)

Banc d'essai : `bench\bench.py` (4 phrases françaises de 5 à 9 s, synthèse vocale Windows). Résultats bruts : `bench\resultats.txt`.

| Modèle | Calcul pour ~8 s d'audio | Qualité sur ce corpus |
|---|---|---|
| `small` | 4 à 6 s | 1 erreur (« où » → « ou ») |
| `medium` | 15 à 18 s | sans faute de mots |
| `large-v3` | 20 à 32 s | sans faute de mots |

Ces phrases synthétiques sont propres : l'écart réel sur une voix au micro, avec bruit et accent, sera plus grand. À confirmer par un essai à la voix.

## Dépannage

- **`Lancer.bat` ne fait rien** : lancez `.\venv\Scripts\python.exe main.py` dans PowerShell pour lire l'erreur. Si le disque externe a changé de lettre ou si le dossier a été déplacé, réparez le venv avec `py -3.12 -m venv venv`. Python 3.12 doit être installé sur le PC lui-même : le venv n'est pas autonome.
- **La fenêtre reste blanche** : installez le runtime Microsoft Edge WebView2 (déjà présent sur Windows 11).
- **Rien ne se passe au raccourci** : attendez l'icône violette ; lancez en administrateur si la touche est interceptée par une application élevée.
- **Notification « Microphone indisponible »** : vérifiez le périphérique d'entrée par défaut de Windows. Un nouvel essai est possible aussitôt.
- **Transcription trop lente** : mesuré sur ce PC avec `large-v3` en CPU/int8 : ~600 s de chargement (premier démarrage, disque froid) et ~25 s pour 4,6 s d'audio. Essayez `whisper_model` = `medium` ou `small` (précision réduite).
- **Espaces étranges avant « ? » dans une application** : désactivez « Espaces insécables » dans les réglages.
- Le dossier `ffmpeg-*` n'est pas utilisé par le code (Faster-Whisper décode via PyAV).

## Tests

```powershell
.\venv\Scripts\python.exe -m unittest discover tests
```

L'interface peut être essayée dans un navigateur, sans le moteur : ouvrez `ui/index.html?mock`.
