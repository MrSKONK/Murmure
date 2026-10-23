; Installateur de Murmure (Inno Setup 6) : installation par utilisateur, sans droits administrateur.
; Construction : ISCC /DAppVersion=1.2.3 packaging\murmure.iss  (après PyInstaller -> dist\Murmure)
; Mise à jour silencieuse (lancée par Murmure) : Murmure-Setup-x.y.z.exe /SILENT /SUPPRESSMSGBOXES /CLOSEAPPLICATIONS

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{6F1C9A52-3B7E-4D8A-9E21-5C4B7A0D2F83}
AppName=Murmure
AppVersion={#AppVersion}
AppVerName=Murmure {#AppVersion}
AppPublisher=MrSKONK
DefaultDirName={localappdata}\Programs\Murmure
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64Mode=x64compatible
OutputDir=..\dist
OutputBaseFilename=Murmure-Setup-{#AppVersion}
SetupIconFile=..\build\murmure.ico
UninstallDisplayIcon={app}\Murmure.exe
UninstallDisplayName=Murmure
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=force
RestartApplications=no

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[InstallDelete]
; les bibliothèques de la version précédente sont remplacées en entier
Type: filesandordirs; Name: "{app}\_internal"

[Files]
Source: "..\dist\Murmure\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Murmure"; Filename: "{app}\Murmure.exe"
Name: "{autodesktop}\Murmure"; Filename: "{app}\Murmure.exe"; Tasks: desktopicon

[Registry]
; retire le lancement avec Windows à la désinstallation (la valeur est créée par Murmure lui-même)
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "Murmure"; ValueType: none; Flags: uninsdeletevalue dontcreatekey

[Run]
Filename: "{app}\Murmure.exe"; Description: "{cm:LaunchProgram,Murmure}"; Flags: nowait postinstall; Check: ShouldLaunch

[Code]
{ /NOLAUNCH : ne pas démarrer Murmure après l'installation (utilisé par les tests automatiques) }
function ShouldLaunch: Boolean;
begin
  Result := Pos('/NOLAUNCH', UpperCase(GetCmdTail)) = 0;
end;
