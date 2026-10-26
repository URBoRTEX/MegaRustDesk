param(
    [string]$BuildRoot = 'C:\CodexBuilds\MegaRustDesk',
    [switch]$UseTaskCargoTransport,
    [switch]$TestSecrets,
    [switch]$TestUpdates,
    [switch]$TestSingleInstance
)
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$policyPath = 'C:\CodexMaintenance\BUILD_RETENTION.md'
if (!(Test-Path -LiteralPath $policyPath)) { throw 'Build retention policy is required' }
Get-Content -LiteralPath $policyPath | Out-Null
$freeBefore = (Get-PSDrive -Name ([IO.Path]::GetPathRoot($BuildRoot).Substring(0,1))).Free
if ($freeBefore -lt 20GB) { throw 'Reserve: at least 20 GiB required before building' }
$env:PYTHONUTF8 = '1'
$vswhere = 'C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe'
$vsPath = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (!$vsPath) { throw 'MSVC C++ build environment is missing' }
$devCommand = Join-Path $vsPath 'Common7\Tools\VsDevCmd.bat'
$devEnvironment = & $env:ComSpec /d /c ('"' + $devCommand + '" -no_logo -arch=x64 -host_arch=x64 >nul && set')
if ($LASTEXITCODE -ne 0) { throw 'MSVC environment initialization failed' }
foreach ($entry in $devEnvironment) {
    if ($entry -match '^([^=]+)=(.*)$') { [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], 'Process') }
}
$env:VCPKG_ROOT = Join-Path $BuildRoot 'toolchain\vcpkg'
$env:LIBCLANG_PATH = Join-Path $BuildRoot 'toolchain\python\clang\native'
$env:CARGO_TARGET_DIR = Join-Path $BuildRoot 'target'
$env:GIT_CONFIG_COUNT = '1'
$env:GIT_CONFIG_KEY_0 = 'http.sslBackend'
$env:GIT_CONFIG_VALUE_0 = 'openssl'
$env:CARGO_NET_GIT_FETCH_WITH_CLI = 'true'
$env:BINDGEN_EXTRA_CLANG_ARGS = (($env:INCLUDE -split ';' | Where-Object { $_ } | ForEach-Object { '-I"' + $_ + '"' }) -join ' ')
$nasmTool = Get-ChildItem -LiteralPath (Join-Path $env:VCPKG_ROOT 'downloads\tools\nasm') -Filter nasm.exe -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
if ($nasmTool) { $env:PATH = $nasmTool.DirectoryName + ';' + $env:PATH }
if ($UseTaskCargoTransport) { $env:CARGO_HOME = Join-Path $BuildRoot 'toolchain\cargo' }
Push-Location $repoRoot
try {
    if ($TestSingleInstance) {
        & (Join-Path $PSScriptRoot 'test_single_instance.ps1') -BuildRoot $BuildRoot
        if ($LASTEXITCODE -ne 0) { throw 'Singleton tests failed' }
        return
    }
    if ($TestUpdates) {
        & (Join-Path $PSScriptRoot 'test_update.ps1') -BuildRoot $BuildRoot
        if ($LASTEXITCODE -ne 0) { throw 'Updater tests failed' }
        return
    }
    if ($TestSecrets) {
        cargo +1.75.0 test -p hbb_common agent_key_tests --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'Windows secret protection test failed' }
        return
    }
    if (!(Select-String -LiteralPath 'libs\hbb_common\src\config.rs' -SimpleMatch 'RUSTDESK_AGENT_DATA' -Quiet)) {
        git -C libs/hbb_common apply (Join-Path $repoRoot 'tools\agent\hbb-portable.patch')
        if ($LASTEXITCODE -ne 0) { throw 'Portable configuration patch failed' }
    }
    python res/inline-sciter.py
    if ($LASTEXITCODE -ne 0) { throw 'Sciter resource generation failed' }
    python tools/agent/runtime_snapshot.py capture
    if ($LASTEXITCODE -ne 0) { throw 'Runtime snapshot failed' }
    cargo +1.75.0 build --locked --release --features agent --bin rustdesk
    if ($LASTEXITCODE -ne 0) { throw 'RustDesk build failed; no package produced' }
    python tools/agent/runtime_snapshot.py verify
    if ($LASTEXITCODE -ne 0) { throw 'Runtime source changed while building' }
    python tools/agent/package.py --build-root $BuildRoot
    if ($LASTEXITCODE -ne 0) { throw 'Packaging failed' }
} finally { Pop-Location }
