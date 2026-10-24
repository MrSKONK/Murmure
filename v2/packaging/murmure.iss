; Installateur de Murmure 2 (Tauri + whisper.cpp), Inno Setup 6. Même AppId que la version 1 : il s'installe
; par-dessus, en gardant réglages, dictionnaire et historique (%APPDATA%\Murmure).
; Construction : ISCC /DAppVersion=2.0.0 v2\packaging\murmure.iss  (après cargo build --release)
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
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist
OutputBaseFilename=Murmure-Setup-{#AppVersion}
SetupIconFile=..\app\icons\icon.ico
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
; fichiers de la version 1 (Python), remplacés par un seul exécutable
Type: filesandordirs; Name: "{app}\_internal"

[Files]
Source: "..\target\release\murmure.exe"; DestDir: "{app}"; DestName: "Murmure.exe"; Flags: ignoreversion
; chargeur Vulkan : sans pilote graphique compatible, Murmure calcule sur le processeur au lieu de refuser de démarrer
Source: "..\dist\runtime\vulkan-1.dll"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist

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
