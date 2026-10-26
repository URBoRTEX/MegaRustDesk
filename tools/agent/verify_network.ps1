param([Parameter(Mandatory=$true)][string]$Executable)
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$qaRoot = [IO.Path]::GetFullPath('C:\CodexBuilds\MegaRustDesk\qa-mcp\')
$exePath = (Resolve-Path -LiteralPath $Executable).Path
if (!$exePath.StartsWith($qaRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'Use the task-owned QA extraction; preserve release staging' }
if (Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -eq $exePath }) { throw 'QA application already running' }

function FirewallSnapshot {
    try {
        $rules = Get-NetFirewallRule -PolicyStore ActiveStore -ErrorAction Stop | Sort-Object Name | Select-Object Name,Enabled,Direction,Action,Profile
        $profiles = Get-NetFirewallProfile -ErrorAction Stop | Sort-Object Name | Select-Object Name,Enabled,NotifyOnListen,DefaultInboundAction,DefaultOutboundAction
        $bytes = [Text.Encoding]::UTF8.GetBytes((@{rules=$rules;profiles=$profiles} | ConvertTo-Json -Depth 5 -Compress))
        $sha = [Security.Cryptography.SHA256]::Create()
        try { return [Convert]::ToBase64String($sha.ComputeHash($bytes)) } finally { $sha.Dispose() }
    } catch { return $null }
}

$before = FirewallSnapshot
$info = [Diagnostics.ProcessStartInfo]::new()
$info.FileName = $exePath
$info.Arguments = '--agent-network-test'
$info.WorkingDirectory = Split-Path -Parent $exePath
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
$info.RedirectStandardOutput = $true
$info.RedirectStandardError = $true
$info.EnvironmentVariables['PATH'] = $env:SystemRoot + '\System32;' + $env:SystemRoot
foreach ($name in @('CODEX_HOME','OPENAI_API_KEY','CODEX_API_KEY','AZURE_OPENAI_API_KEY','ANTHROPIC_API_KEY','NODE_PATH','PYTHONPATH','CARGO_HOME','RUSTUP_HOME','VCPKG_ROOT','LIBCLANG_PATH')) { $info.EnvironmentVariables.Remove($name) }
$process = [Diagnostics.Process]::Start($info)
$stdout = $process.StandardOutput.ReadToEndAsync()
$stderr = $process.StandardError.ReadToEndAsync()
$samples = @()
try {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    do {
        # Query success is required: a permissions failure must not look like zero sockets.
        $tcp = @(Get-NetTCPConnection -ErrorAction Stop | Where-Object { $_.OwningProcess -eq $process.Id })
        $udp = @(Get-NetUDPEndpoint -ErrorAction Stop | Where-Object { $_.OwningProcess -eq $process.Id })
        $samples += [pscustomobject]@{
            seconds=[math]::Round($timer.Elapsed.TotalSeconds,1)
            tcpListeners=@($tcp | Where-Object { $_.State -eq 'Listen' }).Count
            udpEndpoints=$udp.Count
            establishedOutbound=@($tcp | Where-Object { $_.State -eq 'Established' }).Count
        }
        if ($timer.Elapsed.TotalSeconds -gt 60) { throw 'Network startup diagnostic timed out' }
        Start-Sleep -Milliseconds 750
        $process.Refresh()
    } while (!$process.HasExited)
    $output = $stdout.GetAwaiter().GetResult().Trim()
    $errorText = $stderr.GetAwaiter().GetResult()
    if ($process.ExitCode -ne 0) { throw ('Network diagnostic failed with exit ' + $process.ExitCode) }
    $policy = ($output -split '\r?\n')[-1] | ConvertFrom-Json
    $after = FirewallSnapshot
    $passed = $policy.outboundRelayOnly -eq $true -and @($samples | Where-Object { $_.tcpListeners -gt 0 -or $_.udpEndpoints -gt 0 }).Count -eq 0
    $report = [ordered]@{
        executableSha256=(Get-FileHash -LiteralPath $exePath -Algorithm SHA256).Hash.ToLowerInvariant()
        networkPolicy=$policy
        receiverStartupSocketCheckPassed=$passed
        outboundConnectionObserved=(@($samples | Where-Object { $_.establishedOutbound -gt 0 }).Count -gt 0)
        samples=$samples
        firewallSnapshotAvailable=($null -ne $before -and $null -ne $after)
        firewallConfigurationUnchanged=($null -ne $before -and $before -eq $after)
        freshWindowsFirewallPromptTested=$false
        realRemoteConnectionTested=$false
    }
    $json = $report | ConvertTo-Json -Depth 6
    [IO.File]::WriteAllText((Join-Path $repoRoot '.agent-build/network-check.json'), $json, [Text.UTF8Encoding]::new($false))
    $json
    if (!$passed) { throw 'Receiver created a listener/UDP socket or policy was not applied' }
} finally {
    if (!$process.HasExited) { $process.Kill(); $process.WaitForExit() }
    $process.Dispose()
}
