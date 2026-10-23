# Passation Claude Code — Speech-To-Text

Date du point : 23 octobre 2026

## Objectif du projet

Terminer une application Windows de dictée vocale utilisable comme remplacement de Typeless, sans abonnement ni quota d’usage. La voie gratuite retenue est une transcription entièrement locale avec Faster-Whisper. Aucun appel à l’API Mistral ne doit être nécessaire pour dicter.

La transcription locale évite le coût par minute et les quotas d’une API, mais consomme les ressources du PC. L’audio reste sur la machine pour cette étape.

## État constaté

Le projet contient déjà le squelette du parcours de dictée :

1. `main.py` charge `config/settings.json` et lance l’application.
2. `src/tray_app.py` crée l’icône de zone de notification et gère le raccourci clavier maintenu.
3. `src/audio_capture.py` enregistre le microphone en mono, 16 kHz, puis renvoie les échantillons audio.
4. `src/transcriber.py` transcrit avec Faster-Whisper.
5. `src/llm_processor.py` peut envoyer le texte à Ollama pour une correction facultative.
6. `src/text_injector.py` colle le résultat dans l’application active avec le presse-papiers.

Configuration actuelle (`config/settings.json`) :

- raccourci : `²`
- modèle Whisper : `large-v3`
- langue : français (`fr`)
- calcul : CPU, `int8`
- correction Ollama activée, modèle `llama3.2:3b`, serveur local `http://localhost:11434`

Les dépendances incluent déjà Faster-Whisper, sounddevice, keyboard, pystray, pyperclip et PyAutoGUI. Aucune dépendance ni configuration Mistral n’a été trouvée.

## Blocage immédiat observé

L’environnement `venv` contient plusieurs paquets du projet, dont Faster-Whisper, mais son interpréteur ne démarre pas. Le fichier `venv/pyvenv.cfg` pointe vers un exécutable Python WindowsApps ; lancer `venv/Scripts/python.exe` échoue avec `Unable to create process`.

Avant de reconstruire l’environnement, inspecter la configuration et les paquets présents pour éviter d’écraser inutilement l’installation existante. Le dossier n’est pas un dépôt Git : aucun historique ou diff n’était disponible pendant ce point.

## Points à terminer en priorité

1. **Rendre l’environnement Python exécutable.** Identifier un Python installé et compatible, puis réparer ou recréer le venv de manière ciblée. Vérifier que les dépendances listées dans `requirements.txt` sont disponibles.
2. **Valider le parcours complet sous Windows.** L’application doit démarrer, charger le modèle, enregistrer une dictée avec le raccourci, transcrire en français et coller le texte dans le champ actif.
3. **Gérer les erreurs sans bloquer l’application.** Si le chargement du modèle, le microphone, la transcription ou le collage échoue, afficher ou journaliser une erreur compréhensible et permettre un nouvel essai. L’état ne doit pas rester bloqué sur « traitement ».
4. **Rendre Ollama réellement facultatif.** La dictée et la transcription doivent fonctionner lorsqu’Ollama est absent ou désactivé. Garder la correction locale seulement si elle apporte une valeur utile ; elle ne doit pas devenir une dépendance payante ou distante.
5. **Protéger le texte dicté.** `src/tray_app.py` écrit actuellement la transcription complète dans la console. Retirer ces journaux de contenu ou les rendre explicitement désactivables.
6. **Vérifier le comportement du presse-papiers.** `src/text_injector.py` ne restaure l’ancien contenu que s’il n’est pas vide. Vérifier que le collage fonctionne et que le contenu précédent est restauré dans les deux cas.

## Critères de fin

- L’application démarre depuis une installation Python documentée.
- Le raccourci push-to-talk fonctionne et l’icône reflète les états chargement, attente, enregistrement et traitement.
- Une dictée française est transcrite localement et collée dans plusieurs champs d’application Windows.
- La fonction de dictée reste opérationnelle avec Ollama arrêté et sans accès Internet après téléchargement du modèle.
- Une erreur de microphone ou de transcription est visible et ne condamne pas les dictées suivantes.
- Le texte dicté n’est pas affiché dans les journaux par défaut.
- La documentation explique l’installation, le premier téléchargement du modèle, le raccourci, les réglages et le dépannage.
- Les vérifications sont exécutées et leurs résultats sont rapportés sans prétendre à une validation utilisateur qui n’a pas eu lieu.

## Notes de portée

- Ne pas ajouter l’API Mistral pour répondre à l’objectif gratuit : elle est facturée selon la durée audio et nécessite une connexion.
- Le modèle `large-v3` sur CPU peut être lent selon la machine. Mesurer le délai de transcription avant de choisir un modèle plus petit ; tout changement de modèle peut réduire la précision.
- Le fichier `nvidia-smi.txt` indique une GTX 1650 avec 4 Go de mémoire, mais la configuration actuelle utilise le CPU. Ne basculer sur CUDA qu’après vérification de la compatibilité logicielle et un essai réel sur cette machine.
- Le dépôt contient un dossier FFmpeg, mais le code inspecté ne l’appelle pas directement. Confirmer s’il est nécessaire avant de l’inclure dans une procédure d’installation ou de le supprimer.
- Aucun test n’a été lancé pendant ce point d’état.

## Fichiers principaux

- `main.py`
- `config/settings.json`
- `requirements.txt`
- `src/tray_app.py`
- `src/audio_capture.py`
- `src/transcriber.py`
- `src/llm_processor.py`
- `src/text_injector.py`
- `src/config.py`
