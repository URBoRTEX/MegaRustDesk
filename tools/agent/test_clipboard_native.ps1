param([string]$BuildRoot='C:\CodexBuilds\MegaRustDesk')
$ErrorActionPreference='Stop'
$repoRoot=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$vswhere='C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe'
$vsPath=& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$devCommand=Join-Path $vsPath 'Common7/Tools/VsDevCmd.bat'
$entries=& $env:ComSpec /d /c ('"'+$devCommand+'" -no_logo -arch=x64 -host_arch=x64 >nul && set')
if($LASTEXITCODE -ne 0){throw 'MSVC environment failed'}
foreach($entry in $entries){if($entry -match '^([^=]+)=(.*)$'){[Environment]::SetEnvironmentVariable($Matches[1],$Matches[2],'Process')}}
$output=Join-Path $repoRoot '.agent-build/clipboard-native-tests.exe'
$object=Join-Path $repoRoot '.agent-build/clipboard-native-tests.obj'
Push-Location $repoRoot
try {
    & cl.exe /nologo /MT /TC /DWIN32 /W3 /Fe:$output /Fo:$object tools/agent/test_clipboard_native.c /link user32.lib ole32.lib shell32.lib shlwapi.lib uuid.lib gdi32.lib advapi32.lib
    if($LASTEXITCODE -ne 0){throw 'Native clipboard test compile failed'}
    & $output
    if($LASTEXITCODE -ne 0){throw 'Native clipboard tests failed'}
} finally {Pop-Location}
