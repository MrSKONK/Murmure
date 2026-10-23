@echo off
rem Construit dist\Murmure\Murmure.exe (a lancer une fois apres chaque mise a jour du code).
cd /d "%~dp0"
"venv\Scripts\python.exe" -m pip install -r requirements.txt "pyinstaller>=6.10" || goto :fail
"venv\Scripts\python.exe" -m PyInstaller --noconfirm --clean packaging\murmure.spec || goto :fail
set "ISCC=%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe"
if not exist "%ISCC%" goto :noinstaller
for /f %%v in ('"venv\Scripts\python.exe" -c "from src.version import __version__; print(__version__)"') do set "VER=%%v"
"%ISCC%" /DAppVersion=%VER% packaging\murmure.iss || goto :fail
echo Installateur : dist\Murmure-Setup-%VER%.exe
goto :built
:noinstaller
echo Inno Setup absent : installateur non construit (https://jrsoftware.org/isinfo.php).
:built
echo.
echo Termine : dist\Murmure\Murmure.exe
explorer "dist"
pause
exit /b 0
:fail
echo.
echo La construction a echoue : lisez le message ci-dessus.
pause
exit /b 1
