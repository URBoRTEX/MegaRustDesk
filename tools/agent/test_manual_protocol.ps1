param([string]$BuildRoot='C:\CodexBuilds\MegaRustDesk')
$ErrorActionPreference='Stop'
$repoRoot=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$deps=Join-Path $BuildRoot 'target\release\deps'
$library=Get-ChildItem -LiteralPath $deps -Filter 'libhbb_common-*.rlib' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
$arguments=@('+1.75.0','--edition','2021','--cfg','feature=\"agent\"','-C','panic=abort','-C','lto=thin','-C','opt-level=1',(Join-Path $PSScriptRoot 'test_manual_protocol.rs'),'-L',('dependency='+$deps),'--extern',('hbb_common='+$library.FullName),'-o',(Join-Path $repoRoot '.agent-build\manual-protocol-tests.exe'))
foreach($directory in Get-ChildItem -Path (Join-Path $BuildRoot 'toolchain\cargo\registry\src\*\windows_x86_64_msvc-*\lib') -Directory){$arguments+=@('-L',('native='+$directory.FullName))}
& rustc @arguments
if($LASTEXITCODE -ne 0){throw 'Manual protocol tests did not compile'}
& (Join-Path $repoRoot '.agent-build\manual-protocol-tests.exe')
if($LASTEXITCODE -ne 0){throw 'Manual protocol tests failed'}
