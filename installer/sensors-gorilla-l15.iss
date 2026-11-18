#define AppName "sensors.gorilla (ThinkPad L15 Gen 3)"
#define AppVersion "3.0.0"
#define AppExe "sensors-gorilla-l15.exe"
#define AppPublisher "Gorilla System Tools"
#define AppURL "https://github.com/gorillanobakaa-dot/sensors-gorilla"

[Setup]
AppId={{A73D9E24-4A7A-4A9E-8F7E-2D5D9B1F1102}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
DefaultDirName={autopf}\Gorilla Sensors L15
DefaultGroupName=Gorilla Sensors
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
MinVersion=10.0.17763
SetupIconFile=..\assets\icons\sensors-gorilla.ico
UninstallDisplayIcon={app}\sensors-gorilla.ico
OutputDir=output
OutputBaseFilename=sensors-gorilla-{#AppVersion}-win64-thinkpad-l15-installer
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ChangesEnvironment=yes

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"

[Files]
Source: "..\target\x86_64-pc-windows-msvc\release\sensors-gorilla-l15.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\icons\sensors-gorilla.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\pawnio\IntelMSR.bin"; DestDir: "{app}\pawnio"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\pawnio\IntelPCHThermal.bin"; DestDir: "{app}\pawnio"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\pawnio\LpcThinkpadEC.bin"; DestDir: "{app}\pawnio"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\pawnio\COPYING"; DestDir: "{app}\pawnio"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\sensors.gorilla ThinkPad L15 Gen 3"; Filename: "wt.exe"; Parameters: """{app}\{#AppExe}"""; IconFilename: "{app}\sensors-gorilla.ico"
Name: "{group}\sensors.gorilla L15 (Command Prompt)"; Filename: "{app}\{#AppExe}"; IconFilename: "{app}\sensors-gorilla.ico"
Name: "{autodesktop}\sensors.gorilla (ThinkPad L15)"; Filename: "wt.exe"; Parameters: """{app}\{#AppExe}"""; IconFilename: "{app}\sensors-gorilla.ico"; Tasks: desktopicon
Name: "{group}\Uninstall sensors.gorilla L15"; Filename: "{uninstallexe}"

[Registry]
Root: HKLM; Subkey: "SYSTEM\CurrentControlSet\Control\Session Manager\Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Check: NeedsAddPath('{app}')

[Code]
function NeedsAddPath(Param: string): boolean;
var OrigPath: string;
begin
  if not RegQueryStringValue(HKEY_LOCAL_MACHINE,
    'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', OrigPath) then
  begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + Uppercase(Param) + ';', ';' + Uppercase(OrigPath) + ';') = 0;
end;
