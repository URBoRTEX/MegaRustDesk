param([string]$BuildRoot='C:\CodexBuilds\MegaRustDesk')
$ErrorActionPreference='Stop'
$repoRoot=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$deps=Join-Path $BuildRoot 'target\release\deps'
$arguments=@('+1.75.0','--edition','2021','--cfg','test','--cfg','singleton_standalone','-C','panic=abort','-C','lto=thin','-C','opt-level=1',(Join-Path $repoRoot 'src\agent\single_instance.rs'),'-L',('dependency='+$deps),'-o',(Join-Path $repoRoot '.agent-build\singleton-tests.exe'))
foreach($crate in @('serde_json','sha2','sysinfo','winapi')) {
    $library=Get-ChildItem -LiteralPath $deps -Filter ('lib'+$crate+'-*.rlib') | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if(!$library){throw ('Missing compiled dependency: '+$crate)}
    $arguments+=@('--extern',($crate+'='+$library.FullName))
}
foreach($directory in Get-ChildItem -Path (Join-Path $BuildRoot 'toolchain\cargo\registry\src\*\windows_x86_64_msvc-*\lib') -Directory){$arguments+=@('-L',('native='+$directory.FullName))}
& rustc @arguments
if($LASTEXITCODE -ne 0){throw 'Singleton tests did not compile'}
& (Join-Path $repoRoot '.agent-build\singleton-tests.exe')
if($LASTEXITCODE -ne 0){throw 'Singleton tests failed'}
