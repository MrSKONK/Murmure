# Passation Claude Code — Speech-To-Text

Dernière mise à jour : 23 octobre 2026. Ce fichier sert à reprendre le travail d'une session à l'autre.

## Objectif

Une alternative légère à Typeless pour Windows : dictée push-to-talk entièrement locale (Faster-Whisper), gratuite,
sans API distante, avec une vraie interface (pas seulement un terminal), qui reste dans la zone de notification.
Fonctions attendues : dictionnaire personnel, raccourci configurable, réglages simples.

## Architecture actuelle

- `Lancer.bat` → `pythonw main.py` (sans terminal) ; `main.py --background` est utilisé au démarrage de Windows.
- `src/app.py` : fenêtre pywebview (WebView2, pas d'Electron), icône pystray, une seule instance (port local 47653).
- `ui/index.html` : interface Accueil / Dictionnaire / Réglages ; `?mock` permet de l'essayer dans un navigateur.
- `src/engine.py` : états loading / idle / recording / processing / error, raccourci (maintenir ou basculer),
  pipeline micro → Whisper → dictionnaire → correction → collage. Une erreur ne bloque jamais l'essai suivant.
- `src/cleanup.py` : correction par règles (hésitations, ponctuation dictée, typographie française).
  Ollama a été retiré : la correction ne dépend d'aucun modèle de langue.
- `src/overlay.py` : pastille tkinter en bas de l'écran, sans prise de focus.
- `src/autostart.py` : clé Run de l'utilisateur.

## Vérifié

- Tests unitaires de `src/cleanup.py` (`python -m unittest discover tests`).
- Interface en mode `?mock` dans Chromium, thèmes clair et sombre : navigation, dictionnaire (ajout,
  modification, annulation), capture du raccourci, interrupteurs ; aucune erreur JavaScript.

## Reste à vérifier sur le PC Windows

1. Démarrage par `Lancer.bat`, chargement du modèle, icône et pastille.
2. Dictée réelle au micro : raccourci `²` puis une combinaison, modes maintenir et basculer, collage dans plusieurs applications.
3. Ponctuation dictée et suppression des hésitations sur une vraie voix (Whisper peut écrire les commandes autrement).
4. « Lancer avec Windows » et relance alors que l'application tourne déjà.

## Limites connues

- Un raccourci combiné (ex. Ctrl+Alt+D) n'est pas bloqué : la dernière touche parvient aussi à l'application active.
  Une touche seule (`²`) est bloquée.
- Le texte collé passe par le presse-papiers ; l'ancien contenu est restauré 0,2 s après, ce qui peut être court
  pour une application lente, et l'historique Windows (Win+V) garde le texte dicté.
- Python 3.12 doit être installé sur le PC ; le venv n'est pas portable d'une machine à l'autre.
