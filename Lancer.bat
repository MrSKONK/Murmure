@echo off
rem Lance Speech-To-Text sans fenetre de terminal (fonctionne quelle que soit la lettre du disque).
cd /d "%~dp0"
start "" "venv\Scripts\pythonw.exe" "main.py"
