param([string]$BuildRoot='C:\CodexBuilds\MegaRustDesk')
$ErrorActionPreference='Stop'
$repoRoot=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$deps=Join-Path $BuildRoot 'target\release\deps'
$arguments=@('+1.75.0','--edition','2021','--cfg','test','--cfg','update_standalone','-C','panic=abort','-C','lto=thin','-C','opt-level=1',(Join-Path $repoRoot 'src\agent\update_manifest.rs'),'-L',('dependency='+$deps),'-o',(Join-Path $repoRoot '.agent-build\update-tests.exe'))
foreach($crate in @('serde_json','sha2','zip','sodiumoxide')) {
    $library=Get-ChildItem -LiteralPath $deps -Filter ('lib'+$crate+'-*.rlib') | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if(!$library){throw ('Missing compiled dependency: '+$crate)}
    $arguments+=@('--extern',($crate+'='+$library.FullName))
}
$native=Get-ChildItem -LiteralPath (Join-Path $BuildRoot 'target\release\build') -Filter '*.lib' -Recurse | Select-Object -ExpandProperty DirectoryName -Unique
foreach($directory in $native){$arguments+=@('-L',('native='+$directory))}
& rustc @arguments
if($LASTEXITCODE -ne 0){throw 'Updater test compilation failed'}
& (Join-Path $repoRoot '.agent-build\update-tests.exe')
if($LASTEXITCODE -ne 0){throw 'Updater tests failed'}
