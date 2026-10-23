# Speech-To-Text

Dictée vocale push-to-talk pour Windows, 100 % locale (Faster-Whisper), sans abonnement ni API distante.
Maintenez le raccourci, parlez, relâchez : le texte est collé dans l'application active.

## Installation

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

Tout se règle depuis la fenêtre et s'enregistre aussitôt dans `config/settings.json` :

| Réglage (clé) | Rôle |
|---|---|
| Touche de dictée (`hotkey`) | touche ou combinaison, capturée en cliquant puis en l'appuyant (défaut `²`) |
| Mode (`hotkey_mode`) | `hold` : maintenir pour parler ; `toggle` : un appui démarre, un second arrête |
| Langue (`whisper_language`) | `fr`, `en`, `es`, `de`, `it`, `pt`, `nl` ou `auto` |
| Modèle (`whisper_model`) | `small` (rapide), `medium` (équilibré), `large-v3` (précis, lent sur CPU) |
| Charge processeur (`cpu_threads`) | `0` = tous les cœurs ; `4` ou `2` laissent de la marge au reste du PC |
| Libérer la mémoire (`unload_after_min`) | décharge le modèle après N minutes sans dictée (`0` = jamais) |
| Entrée audio (`mic`) | micro choisi ; vide = micro par défaut de Windows |
| Pastille à l'écran (`show_overlay`) | indicateur en bas de l'écran |
| Lancer avec Windows (`autostart`) | démarre en arrière-plan à l'ouverture de session |
| Dictionnaire (`dictionary`) | mots soufflés à Whisper, avec les variantes « entendu comme » remplacées automatiquement |
| Correction (`remove_fillers`, `spoken_punctuation`, `french_nbsp`) | voir ci-dessous |

Réglages sans interface, à modifier dans le fichier : `device` / `compute_type` (`cpu` / `int8` ; `cuda`
seulement après essai réel sur votre machine) et `log_transcripts` (`false` : le texte dicté n'est jamais écrit
dans la console).

## Correction du texte

La correction est faite localement par des règles (`src/cleanup.py`), sans modèle de langue ni service externe :

- **Hésitations** : « euh », « heu », « hum » sont retirés avec la virgule qui les suit.
- **Ponctuation dictée** : « virgule », « point-virgule », « deux-points », « point d'interrogation »,
  « point d'exclamation », « points de suspension », « point final », « point à la ligne », « à la ligne »,
  « nouveau paragraphe », « ouvrez / fermez les guillemets », « ouvrez / fermez la parenthèse ».
  Le mot « point » seul n'est pas converti, car il est trop courant en français.
- **Typographie française** (langue `fr`, imposée ou détectée) : espace avant `; : ! ?` et dans les guillemets,
  sans toucher aux heures (`14:30`) ni aux adresses (`http://`).
- **Majuscules** : après une fin de phrase dictée et en début de texte.

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
