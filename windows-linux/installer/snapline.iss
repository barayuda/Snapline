; Snapline for Windows: per-user installer (no admin). Built on CI with
;   ISCC.exe /DVersion=<version> /DSource=<path to snapline.exe> installer\snapline.iss
; Never change AppId: upgrades find the installed Snapline by it.

#ifndef Version
  #error Pass /DVersion=<version>
#endif
#ifndef Source
  #error Pass /DSource=<path to snapline.exe>
#endif

[Setup]
AppId={{CC116E4B-B1A0-4EF8-B87C-4E9EB12E2ED1}
AppName=Snapline
AppVersion={#Version}
AppVerName=Snapline {#Version}
AppPublisher=Barayuda
AppPublisherURL=https://github.com/barayuda/Snapline
AppSupportURL=https://github.com/barayuda/Snapline/issues
VersionInfoVersion={#Version}
PrivilegesRequired=lowest
DefaultDirName={localappdata}\Programs\Snapline
DisableDirPage=auto
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
LicenseFile=..\..\LICENSE.md
OutputBaseFilename=Snapline-Setup-{#Version}
UninstallDisplayIcon={app}\snapline.exe
UninstallDisplayName=Snapline
; The app's single-instance mutex: a running Snapline is closed before files are replaced.
AppMutex=Local\dev.barayuda.snapline
CloseApplications=yes
RestartApplications=no
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Tasks]
Name: "startup"; Description: "Launch Snapline when I sign in to Windows"; Flags: unchecked

[Files]
Source: "{#Source}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Snapline"; Filename: "{app}\snapline.exe"

[Registry]
; The same value the General tab writes ("<path>\snapline.exe" in quotes), so both agree.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Snapline"; ValueData: """{app}\snapline.exe"""; Flags: uninsdeletevalue; Tasks: startup
; Clears "Disabled" set in Task Manager when the user asks to launch at sign-in.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"; ValueType: none; ValueName: "Snapline"; Flags: deletevalue; Tasks: startup
; Uninstall always removes both, also when "Launch at sign-in" was turned on later in Snapline's settings.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "Snapline"; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"; ValueType: none; ValueName: "Snapline"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\snapline.exe"; Description: "Start Snapline"; Flags: nowait postinstall skipifsilent
