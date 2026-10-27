param([string]$BuildRoot='C:\CodexBuilds\MegaRustDesk')
$ErrorActionPreference='Stop'
if($PSVersionTable.PSVersion.Major -ge 7){$PSNativeCommandArgumentPassing='Legacy'}
$repoRoot=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$deps=Join-Path $BuildRoot 'target\release\deps'
# Check-only dependencies have rmeta; host/build-script rlibs may lack LTO.
# Resolve a bin-rustdesk dependency that has a code-bearing target rlib.
$fingerprints=Join-Path $BuildRoot 'target/release/.fingerprint'
$linked=@(Get-ChildItem -LiteralPath $fingerprints -Directory -Filter 'rustdesk-*' | Where-Object {
    Test-Path -LiteralPath (Join-Path $_.FullName 'bin-rustdesk.json')
} | Sort-Object { (Get-Item -LiteralPath (Join-Path $_.FullName 'bin-rustdesk.json')).LastWriteTime } -Descending)
$library=$null
foreach($binary in $linked){
    $raw=[IO.File]::ReadAllText((Join-Path $binary.FullName 'bin-rustdesk.json'))
    $match=[regex]::Match($raw,'\[\d+,\s*"hbb_common",\s*(?:true|false),\s*(\d+)\]')
    if(!$match.Success){continue}
    $expected=[UInt64]::Parse($match.Groups[1].Value)
    foreach($folder in Get-ChildItem -LiteralPath $fingerprints -Directory -Filter 'hbb_common-*'){
        $stamp=Join-Path $folder.FullName 'lib-hbb_common'
        $candidate=Join-Path $deps ('lib'+$folder.Name+'.rlib')
        if(!(Test-Path -LiteralPath $stamp) -or !(Test-Path -LiteralPath $candidate)){continue}
        $hex=[IO.File]::ReadAllText($stamp).Trim()
        $bytes=[byte[]]@(for($i=0;$i -lt 16;$i+=2){[Convert]::ToByte($hex.Substring($i,2),16)})
        if([BitConverter]::ToUInt64($bytes,0) -eq $expected){$library=Get-Item -LiteralPath $candidate;break}
    }
    if($library){break}
}
if(!$library){throw 'Build a release binary with its target dependency rlib first'}
$arguments=@('+1.75.0','--edition','2021','--cfg','feature=\"agent\"','-C','panic=abort','-C','lto=thin','-C','opt-level=1',(Join-Path $repoRoot 'tools/agent/test_connection_retry.rs'),'-L',('dependency='+$deps),'--extern',('hbb_common='+$library.FullName),'-o',(Join-Path $repoRoot '.agent-build\connection-retry-tests.exe'))
foreach($directory in Get-ChildItem -Path (Join-Path $BuildRoot 'toolchain\cargo\registry\src\*\windows_x86_64_msvc-*\lib') -Directory){$arguments+=@('-L',('native='+$directory.FullName))}
& rustc @arguments
if($LASTEXITCODE -ne 0){throw 'Connection retry tests did not compile'}
& (Join-Path $repoRoot '.agent-build\connection-retry-tests.exe')
if($LASTEXITCODE -ne 0){throw 'Connection retry tests failed'}
