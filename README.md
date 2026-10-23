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

```powershell
.\venv\Scripts\python.exe main.py
```

L'icône apparaît dans la zone de notification :

| Couleur | État |
|---|---|
| Gris | chargement du modèle |
| Bleu | en attente |
| Rouge | enregistrement |
| Orange | transcription en cours |
| Rouge sombre | le modèle n'a pas pu se charger (voir la console) |

**Premier lancement :** le modèle `large-v3` (~3 Go) est téléchargé une fois dans `~\.cache\huggingface`.
Ensuite, tout fonctionne hors ligne.

## Réglages (`config/settings.json`)

| Clé | Rôle |
|---|---|
| `hotkey` | touche maintenue pour dicter (défaut `²`) |
| `whisper_model` | `large-v3` (précis, lent sur CPU), `medium`, `small`… |
| `whisper_language` | langue de dictée (`fr`) |
| `device` / `compute_type` | `cpu` / `int8` ; `cuda` seulement après essai réel sur votre machine |
| `remove_fillers` | retire « euh », « heu », « hum »… (défaut `true`) |
| `spoken_punctuation` | convertit la ponctuation dictée en signes (défaut `true`, voir ci-dessous) |
| `french_nbsp` | en français, espaces insécables avant `; : ! ?` et dans les guillemets (défaut `true`) |
| `log_transcripts` | `false` par défaut : le texte dicté n'est pas écrit dans la console |

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

- **Rien ne se passe au raccourci** : attendez l'icône bleue ; lancez en administrateur si la touche est interceptée par une application élevée.
- **Notification « Microphone indisponible »** : vérifiez le périphérique d'entrée par défaut de Windows. Un nouvel essai est possible aussitôt.
- **Transcription trop lente** : mesuré sur ce PC avec `large-v3` en CPU/int8 : ~600 s de chargement (premier démarrage, disque froid) et ~25 s pour 4,6 s d'audio. Essayez `whisper_model` = `medium` ou `small` (précision réduite).
- **Espaces étranges avant « ? » dans une application** : désactivez « Espaces insécables » dans les réglages.
- Le dossier `ffmpeg-*` n'est pas utilisé par le code (Faster-Whisper décode via PyAV).
