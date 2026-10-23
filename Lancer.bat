@echo off
rem Lance Murmure sans fenetre de terminal (fonctionne quelle que soit la lettre du disque).
cd /d "%~dp0"
start "" "venv\Scripts\pythonw.exe" "main.py"
