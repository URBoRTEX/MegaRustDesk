param([string]$BuildRoot='C:\CodexBuilds\MegaRustDesk')
$ErrorActionPreference='Stop'
$repoRoot=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$deps=Join-Path $BuildRoot 'target\release\deps'
$library=Get-ChildItem -LiteralPath $deps -Filter 'libsciter-*.rlib' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if(!$library){throw 'Missing compiled Sciter dependency'}
& rustc +1.75.0 --edition 2021 -C panic=abort -C lto=thin -C opt-level=1 (Join-Path $PSScriptRoot 'test_viewer_controls.rs') -L ('dependency='+$deps) --extern ('sciter='+$library.FullName) -o (Join-Path $repoRoot '.agent-build\viewer-controls-tests.exe')
if($LASTEXITCODE -ne 0){throw 'Viewer UI tests did not compile'}
Push-Location $repoRoot
try {
    & (Join-Path $repoRoot '.agent-build\viewer-controls-tests.exe') (Join-Path $BuildRoot 'toolchain\sciter.dll')
    if($LASTEXITCODE -ne 0){throw 'Viewer UI tests failed'}
} finally { Pop-Location }
