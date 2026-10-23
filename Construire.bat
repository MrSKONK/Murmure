@echo off
rem Construit dist\Murmure\Murmure.exe (a lancer une fois apres chaque mise a jour du code).
cd /d "%~dp0"
"venv\Scripts\python.exe" -m pip install -r requirements.txt "pyinstaller>=6.10" || goto :fail
"venv\Scripts\python.exe" -m PyInstaller --noconfirm --clean packaging\murmure.spec || goto :fail
echo.
echo Termine : dist\Murmure\Murmure.exe
explorer "dist\Murmure"
pause
exit /b 0
:fail
echo.
echo La construction a echoue : lisez le message ci-dessus.
pause
exit /b 1
