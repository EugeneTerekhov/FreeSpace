; FreeSpace 1.0 — Inno Setup script.
;
; Build first:
;   cargo build --workspace --release
; then compile this file with ISCC (Inno Setup 6).
;
; The installer copies both executables and the application-owned `data`
; directory. It never registers Explorer entries: the context menu is disabled
; by default and is controlled by the `context_menu_enabled` setting. Uninstall
; removes the data directory and both user and machine context-menu entries.

#define AppName "FreeSpace"
#define AppVersion "1.1.0"
#define AppPublisher "FreeSpace"
#define AppExeName "freespace-gui.exe"
#define CliExeName "freespace.exe"

[Setup]
AppId={{9D4B1B3E-6B5A-4B7E-9E27-5C2B0F0A1D10}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
DefaultDirName={sd}\FreeSpace
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
OutputDir=dist
OutputBaseFilename=freespace-setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
UninstallDisplayIcon={app}\{#AppExeName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Dirs]
Name: "{app}\data"; Permissions: users-modify

[Files]
Source: "..\target\release\freespace-gui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\freespace.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\docs\user-guide.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "..\en\user-guide.md"; DestDir: "{app}\docs\en"; Flags: ignoreversion
Source: "..\ru\user-guide.md"; DestDir: "{app}\docs\ru"; Flags: ignoreversion
Source: "..\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{group}\{#AppName} command line"; Filename: "{cmd}"; Parameters: "/k ""{app}\{#CliExeName}"" --help"
Name: "{group}\{cm:UninstallProgram,{#AppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Run]
; A fresh installation must contain no Explorer entries. This also removes
; leftovers from an older version that registered them by default.
Filename: "{app}\{#CliExeName}"; Parameters: "context-menu uninstall --all --yes"; Flags: runhidden waituntilterminated skipifdoesntexist
Filename: "{app}\{#AppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(AppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; Remove the entries before the executables disappear. Both scopes are cleaned
; idempotently; missing entries are ignored by the CLI.
Filename: "{app}\{#CliExeName}"; Parameters: "context-menu uninstall --yes"; Flags: runhidden waituntilterminated skipifdoesntexist
Filename: "{app}\{#CliExeName}"; Parameters: "context-menu uninstall --machine --yes"; Flags: runhidden waituntilterminated skipifdoesntexist

[UninstallDelete]
; The application writes only inside {app}\data. Removing it leaves no
; settings, journal, quarantine or hash cache behind.
Type: filesandordirs; Name: "{app}\data"

[Code]
function InitializeSetup(): Boolean;
begin
  Result := True;
end;
