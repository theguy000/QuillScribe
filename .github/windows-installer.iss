#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceExe
  #define SourceExe "..\target\release\quillscribe.exe"
#endif
#ifndef OutputDir
  #define OutputDir ".."
#endif
#ifndef OutputBaseFilename
  #define OutputBaseFilename "QuillScribe-Setup"
#endif

[Setup]
AppId={{B05A386A-228C-4A19-B29A-18A13347F963}
AppName=QuillScribe
AppVersion={#AppVersion}
AppVerName=QuillScribe {#AppVersion}
AppPublisher=QuillScribe Team
AppPublisherURL=https://github.com/theguy000/QuillScribe
AppSupportURL=https://github.com/theguy000/QuillScribe/issues
AppUpdatesURL=https://github.com/theguy000/QuillScribe/releases/latest
DefaultDirName={localappdata}\Programs\QuillScribe
DefaultGroupName=QuillScribe
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupIconFile=..\icons\icon.ico
UninstallDisplayIcon={app}\QuillScribe.ico
OutputDir={#OutputDir}
OutputBaseFilename={#OutputBaseFilename}
CloseApplications=yes
RestartApplications=no
VersionInfoVersion={#AppVersion}
VersionInfoProductName=QuillScribe
VersionInfoProductVersion={#AppVersion}

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "QuillScribe.exe"; Flags: ignoreversion
Source: "..\icons\icon.ico"; DestDir: "{app}"; DestName: "QuillScribe.ico"; Flags: ignoreversion

[Icons]
Name: "{group}\QuillScribe"; Filename: "{app}\QuillScribe.exe"; WorkingDir: "{app}"; IconFilename: "{app}\QuillScribe.ico"
Name: "{userdesktop}\QuillScribe"; Filename: "{app}\QuillScribe.exe"; WorkingDir: "{app}"; IconFilename: "{app}\QuillScribe.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\QuillScribe.exe"; Description: "Launch QuillScribe"; Flags: nowait postinstall skipifsilent
