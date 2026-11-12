; OpenCrosshair installer script for Inno Setup 6.
; Built by installer\build-installer.ps1 (or CI), which passes the version:
;   ISCC.exe /DMyAppVersion=0.1.0 installer\OpenCrosshair.iss
; Relative paths below are relative to this file's folder.

#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif

#define MyAppName "OpenCrosshair"
#define MyAppPublisher "OpenCrosshair contributors"
#define MyAppExeName "opencrosshair.exe"
#define MyRunValueName "OpenCrosshair"

[Setup]
; Fixed GUID identifying this application. Do not change it between releases.
AppId={{9BE87C66-F610-43DA-BCC9-CEA250FF8812}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
VersionInfoVersion={#MyAppVersion}
VersionInfoProductName={#MyAppName}
UninstallDisplayName={#MyAppName}
UninstallDisplayIcon={app}\{#MyAppExeName}

; Per-user install by default, no administrator rights needed.
; The dialog lets the user choose an all-users install, which asks for elevation.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes

; x64 only, Windows 10 or newer.
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0

; Close a running instance before replacing files.
CloseApplications=yes
RestartApplications=no

LicenseFile=..\LICENSE
SetupIconFile=..\assets\icons\icon.ico
OutputDir=..\dist
OutputBaseFilename=OpenCrosshair-{#MyAppVersion}-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked
Name: "startupentry"; Description: "Launch OpenCrosshair when I sign in to Windows (starts minimized to the tray)"; GroupDescription: "Startup:"; Flags: unchecked

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
; Autostart is optional and off by default. The value is removed on uninstall.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "{#MyRunValueName}"; ValueData: """{app}\{#MyAppExeName}"" --minimized"; Flags: uninsdeletevalue; Tasks: startupentry

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent

[Code]
// Asks whether to delete the user data folder. Default answer is No.
// Silent uninstalls never delete data.
function ShouldDeleteUserData(): Boolean;
begin
  if UninstallSilent() then
    Result := False
  else
    Result := MsgBox(
      'Also delete your OpenCrosshair settings, presets, game profiles and log files?' + #13#10 + #13#10 +
      ExpandConstant('{userappdata}\OpenCrosshair') + #13#10 +
      ExpandConstant('{localappdata}\OpenCrosshair') + #13#10 + #13#10 +
      'Choose No to keep them for a future installation.',
      mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DataDir: String;
begin
  if CurUninstallStep = usUninstall then
  begin
    // Make sure the autostart value is gone even if it was added after installation.
    RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', '{#MyRunValueName}');
  end;

  if CurUninstallStep = usPostUninstall then
  begin
    if ShouldDeleteUserData() then
    begin
      DataDir := ExpandConstant('{userappdata}\OpenCrosshair');
      if DirExists(DataDir) then
        DelTree(DataDir, True, True, True);
      DataDir := ExpandConstant('{localappdata}\OpenCrosshair');
      if DirExists(DataDir) then
        DelTree(DataDir, True, True, True);
    end;
  end;
end;
