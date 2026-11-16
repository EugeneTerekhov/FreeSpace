; FreeSpace 1.0 — Inno Setup script.
;
; Build first:
;   cargo build --workspace --release
; then compile this file with ISCC (Inno Setup 6).
;
; The installer copies both executables, registers the optional Explorer
; integration through the CLI (a separate task) and never starts a network
; connection.

#define AppName "FreeSpace"
#define AppVersion "1.0.0"
#define AppPublisher "FreeSpace"
#define AppExeName "freespace-gui.exe"

[Setup]
AppId={{9D4B1B3E-6B5A-4B7E-9E27-5C2B0F0A1D10}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
DefaultDirName={autopf}\FreeSpace
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
OutputDir=dist
OutputBaseFilename=freespace-setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
UninstallDisplayIcon={app}\{#AppExeName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"

[Tasks]
Name: "contextmenu"; Description: "Add FreeSpace to the Explorer context menu (current user)"; GroupDescription: "Integration:"; Flags: unchecked
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\release\freespace-gui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\freespace.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\docs\user-guide.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "..\en\user-guide.md"; DestDir: "{app}\docs\en"; Flags: ignoreversion
Source: "..\ru\user-guide.md"; DestDir: "{app}\docs\ru"; Flags: ignoreversion
Source: "..\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{group}\{#AppName} command line"; Filename: "{cmd}"; Parameters: "/k ""{app}\freespace.exe"" --help"
Name: "{group}\{cm:UninstallProgram,{#AppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\freespace.exe"; Parameters: "context-menu install"; Tasks: contextmenu; Flags: runhidden waituntilterminated
Filename: "{app}\{#AppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(AppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{app}\freespace.exe"; Parameters: "context-menu uninstall"; Flags: runhidden waituntilterminated

[Code]
function InitializeSetup(): Boolean;
begin
  Result := True;
end;
