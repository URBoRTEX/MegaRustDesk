param([string]$BuildRoot='C:\CodexBuilds\MegaRustDesk')
$ErrorActionPreference='Stop'
$repoRoot=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$deps=Join-Path $BuildRoot 'target\release\deps'
$library=Get-ChildItem -LiteralPath $deps -Filter 'libwinapi-*.rlib' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
& rustc +1.75.0 --edition 2021 -C panic=abort -C lto=thin -C opt-level=1 (Join-Path $PSScriptRoot 'test_update_job.rs') -L ('dependency='+$deps) --extern ('winapi='+$library.FullName) -o (Join-Path $repoRoot '.agent-build\update-job-tests.exe')
if($LASTEXITCODE -ne 0){throw 'Job tests did not compile'}
Push-Location $repoRoot
try { & (Join-Path $repoRoot '.agent-build\update-job-tests.exe'); if($LASTEXITCODE -ne 0){throw 'Job lifecycle tests failed'} } finally {Pop-Location}
