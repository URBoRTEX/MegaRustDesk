$ErrorActionPreference='Stop'
$repoRoot=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$fixture=Join-Path $repoRoot '.agent-build/diagnose-fixture'
if((Test-Path -LiteralPath $fixture) -and [IO.File]::ReadAllText((Join-Path $fixture 'RustDesk-Agent.exe')) -ne 'diagnostic fixture'){throw 'Unknown fixture contents'}
[IO.Directory]::CreateDirectory((Join-Path $fixture 'data/rustdesk/log')) | Out-Null
[IO.File]::WriteAllText((Join-Path $fixture 'RustDesk-Agent.exe'),'diagnostic fixture')
[IO.File]::WriteAllText((Join-Path $fixture 'manifest.json'),'{"version":"fixture","files":{}}')
$nonce='a30b8040-e1bd-4cf1-9b58-5c4b8bbd1a77'
$events=@(
    "create_relay requested from 192.0.2.1:1234, uuid: $nonce, secure: true",
    "Failed to create relay connection for 192.0.2.1:1234 with uuid ${nonce}: Peer handshake receive timed out: deadline has elapsed",
    'Peer handshake receive timed out: deadline has elapsed',
    'Relay pre-login handshake timed out; retrying attempt 2/3',
    'Relay pre-login transport interrupted; retrying attempt 2/3',
    'Agent transport WSS connected: wss://ovh-fr2.rustdesk.com/ws/relay',
    'Failed to create relay connection token=DO_NOT_EXPORT'
)
[IO.File]::WriteAllLines((Join-Path $fixture 'data/rustdesk/log/fixture.log'),$events)
$priorRoot=$env:MEGARUSTDESK_DIAG_ROOT
$priorOutput=$env:MEGARUSTDESK_DIAG_OUTPUT
try {
    $env:MEGARUSTDESK_DIAG_ROOT=$fixture
    $env:MEGARUSTDESK_DIAG_OUTPUT=Join-Path $fixture 'report.json'
    function Get-CimInstance { @() }
    function Get-NetTCPConnection { @() }
    & (Join-Path $PSScriptRoot 'diagnose.ps1')
    $raw=[IO.File]::ReadAllText($env:MEGARUSTDESK_DIAG_OUTPUT)
    $report=$raw | ConvertFrom-Json
    $hashes=@([regex]::Matches($raw,'uuidHash ([0-9a-f]{16})') | ForEach-Object {$_.Groups[1].Value})
    if($hashes.Count -ne 2 -or $hashes[0] -ne $hashes[1]){throw 'Request and failure must have the same canonical nonce hash'}
    if($report.logEvents.Count -ne 6){throw 'Handshake stage, retry count and WSS route must survive export'}
    if($raw -match '192\.0\.2\.1|DO_NOT_EXPORT|a30b8040-e1bd'){throw 'Sensitive event data escaped redaction'}
    if($report.settingsChanged -or $report.authenticationRead){throw 'Diagnostic must remain read-only'}
    $activeRoot=Join-Path $fixture 'active'
    [IO.Directory]::CreateDirectory($activeRoot) | Out-Null
    $activeExe=Join-Path $activeRoot 'RustDesk-Agent.exe'
    [IO.File]::WriteAllText($activeExe,'diagnostic fixture')
    [IO.File]::WriteAllText((Join-Path $activeRoot 'manifest.json'),'{"version":"running-fixture","files":{}}')
    $diagnosticProcesses=@([pscustomobject]@{Name='RustDesk-Agent.exe';ExecutablePath=$activeExe;CommandLine=('"'+$activeExe+'"');ProcessId=12345;CreationDate=[DateTime]::Now;SessionId=(Get-Process -Id $PID).SessionId})
    Set-Item Function:Get-CimInstance -Value ({ $diagnosticProcesses }.GetNewClosure())
    & (Join-Path $PSScriptRoot 'diagnose.ps1')
    $activeReport=Get-Content -LiteralPath $env:MEGARUSTDESK_DIAG_OUTPUT -Raw | ConvertFrom-Json
    if($activeReport.packageVersion -ne 'running-fixture' -or !$activeReport.processes[0].sameFolder -or !$activeReport.packageIsRunning){throw 'Diagnostic selected stale adjacent package instead of running main'}
    $foreign=[pscustomobject]@{Name='RustDesk-Agent.exe';ExecutablePath=(Join-Path $fixture 'RustDesk-Agent.exe');CommandLine='RustDesk-Agent.exe';ProcessId=12346;CreationDate=[DateTime]::Now;SessionId=((Get-Process -Id $PID).SessionId+100)}
    $diagnosticProcesses=@($diagnosticProcesses[0],$foreign)
    Set-Item Function:Get-CimInstance -Value ({ $diagnosticProcesses }.GetNewClosure())
    & (Join-Path $PSScriptRoot 'diagnose.ps1')
    $activeReport=Get-Content -LiteralPath $env:MEGARUSTDESK_DIAG_OUTPUT -Raw | ConvertFrom-Json
    if($activeReport.packageVersion -ne 'running-fixture'){throw 'Another Windows session changed diagnostic selection'}
    Write-Host 'PASS: nonce correlation, stage visibility, redaction, read-only report'
} finally {
    $env:MEGARUSTDESK_DIAG_ROOT=$priorRoot
    $env:MEGARUSTDESK_DIAG_OUTPUT=$priorOutput
    # Retain fixture on failure for investigation. Cleanup only after a successful run.
}
