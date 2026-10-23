# Passation Claude Code — Murmure (dépôt Speech-To-Text)

Dernière mise à jour : 23 octobre 2026. Ce fichier sert à reprendre le travail d'une session à l'autre.

## Objectif

Une alternative légère à Typeless pour Windows : dictée push-to-talk entièrement locale (Faster-Whisper), gratuite,
sans API distante, avec une vraie interface (pas seulement un terminal), qui reste dans la zone de notification.
Fonctions attendues : dictionnaire personnel, raccourci configurable, réglages simples.

## Architecture actuelle

- Exécutable : `packaging/murmure.spec` (PyInstaller, dossier `dist/Murmure/`), `Construire.bat` en local,
  `.github/workflows/build-windows.yml` sur GitHub (tests, construction, autotest `Murmure.exe --check <rapport>`,
  artefact `Murmure-windows`). Réglages de l'exécutable dans `%APPDATA%\Murmure`.

- `Lancer.bat` → `pythonw main.py` (sans terminal) ; `main.py --background` est utilisé au démarrage de Windows.
- `src/app.py` : fenêtre pywebview (WebView2, pas d'Electron), icône pystray, une seule instance (port local 47653).
- `ui/index.html` : interface Accueil / Dictionnaire / Réglages ; `?mock` permet de l'essayer dans un navigateur.
- `src/engine.py` : états loading / idle / recording / processing / error, raccourci (maintenir ou basculer),
  pipeline micro → Whisper → dictionnaire → correction → collage. Une erreur ne bloque jamais l'essai suivant.
- `src/cleanup.py` : correction par règles (hésitations et répétitions, ponctuation dictée, adresses e-mail/web,
  messages courts sans point final, typographie française, raccord au texte avant le curseur).
- `src/caret.py` : lecture des caractères avant le curseur via UI Automation (comtypes), délai max 0,4 s.
  Ollama a été retiré : la correction ne dépend d'aucun modèle de langue.
- `src/history.py` : historique des dictées (`history.json` à côté des réglages, limite configurable),
  onglet Historique dans l'interface (recherche, copier, supprimer, tout effacer).
- `src/updater.py` + `packaging/murmure.iss` : installateur Inno Setup (par utilisateur) et mises à jour depuis les
  releases du dépôt public `MrSKONK/Murmure` (SHA-256 vérifié, installation silencieuse puis relance).
  `src/version.py` = version ; `CHANGELOG.md` = notes de release. Le dépôt de code (renommé `MrSKONK/Murmure`,
  public) héberge aussi les releases : publication avec la clé automatique d'Actions, aucun secret requis.
- `src/overlay.py` : pastille dessinée avec Pillow (onde en dégradé, minuteur, croix d'annulation), affichée par tkinter
  sans prise de focus. Annulation : `Engine.cancel()` (croix ou Échap).
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

## Feuille de route

### 1. Finir le MVP (version Python actuelle)
- [ ] CI verte avec l'installateur : construction Inno Setup, installation silencieuse, autotest de l'app installée.
- [x] Dépôt renommé `MrSKONK/Murmure` et rendu public : il sert aussi de dépôt de releases.
- [ ] Essais réels sur le PC Windows :
  - pastille : rendu, fondu, échelle selon le DPI ;
  - croix : le clic ne doit pas retirer le focus à l'application de saisie ;
  - touche Échap ;
  - raccord au texte avant le curseur dans Word, un navigateur, WhatsApp et Slack ;
  - historique ;
  - modèle `large-v3-turbo`.
- [ ] Fusion sur `main` → première release `v1.0.0` ; puis une `v1.0.1` pour tester la mise à jour de bout en bout.
- [x] Correcteur Grammalecte 2.3.0 (`src/grammar.py`) : téléchargé à l'activation (GPL, non distribué), corrections
      homophones à suggestion unique ; calibré avec `tools/grammalecte_probe.py` (workflow « Grammalecte probe ») et
      vérifié dans l'exécutable par `Murmure.exe --check-grammar`.

### 2. Version 2 : réécriture Tauri (Rust) avec whisper.cpp, pour la transcription en temps réel
Décidée par l'utilisateur, à commencer une fois le MVP terminé. Objectifs : temps réel, carte graphique
(GTX 1650), exécutable d'environ 20 Mo, démarrage rapide.
- **Moteur :** whisper.cpp via `whisper-rs`, avec accélération Vulkan (ou CUDA) pour la GTX 1650.
  Le temps réel découpe la parole par le VAD et transcrit pendant qu'on parle. La pastille affiche le texte provisoire,
  puis le texte définitif est collé au relâchement.
- **Interface :** reprendre `ui/index.html` presque tel quel (WebView2, comme aujourd'hui) ; les appels
  `window.pywebview.api.*` deviennent des commandes Tauri `invoke`.
- **Pastille :** fenêtre Tauri transparente, non activable, toujours au premier plan (l'onde peut passer en CSS ou canvas).
- **Corrections :** porter `src/cleanup.py` en Rust ; les tests de `tests/test_cleanup.py` servent de cahier des
  charges (les exporter en cas JSON partagés).
- **Compatibilité :** mêmes fichiers `%APPDATA%\Murmure\settings.json` et `history.json` ; mises à jour via
  `tauri-plugin-updater` sur le même dépôt public de releases ; même AppId d'installation si possible, pour que
  la v2 se pose par-dessus la v1.
