$ErrorActionPreference = 'Stop';
$all = @(Get-CimInstance Win32_Process -ErrorAction Stop | Where-Object { $_.Name -like '*RustDesk*' });
$appRoot = [IO.Path]::GetFullPath($env:MEGARUSTDESK_DIAG_ROOT);
$session=(Get-Process -Id $PID).SessionId;
$roots = @($all | Where-Object { $_.SessionId -eq $session -and $_.ExecutablePath -and $_.CommandLine -notmatch '\s--' -and (Split-Path -Leaf $_.ExecutablePath) -ieq 'RustDesk-Agent.exe' } | ForEach-Object { Split-Path -Parent $_.ExecutablePath } | Sort-Object -Unique | Where-Object { Test-Path -LiteralPath (Join-Path $_ 'manifest.json') });
if ($roots.Count -gt 1) { throw 'More than one portable main is running; cannot select a diagnostic target' };
if ($roots.Count -eq 1) {
    $appRoot = $roots[0];
} elseif (!(Test-Path -LiteralPath (Join-Path $appRoot 'manifest.json'))) {
    throw 'Open the application, or place this diagnostic beside RustDesk-Agent.exe and manifest.json';
};
$exe = Join-Path $appRoot 'RustDesk-Agent.exe';
$manifest = Get-Content -LiteralPath (Join-Path $appRoot 'manifest.json') -Encoding UTF8 -Raw | ConvertFrom-Json;
$exeItem = Get-Item -LiteralPath $exe;
$exeHasher = [Security.Cryptography.SHA256]::Create(); $exeStream=[IO.File]::OpenRead($exe);
try { $exeHash=[BitConverter]::ToString($exeHasher.ComputeHash($exeStream)).Replace('-','').ToLowerInvariant() } finally { $exeStream.Dispose(); $exeHasher.Dispose() };
$processes = @($all | ForEach-Object { [ordered]@{pid=$_.ProcessId; sameFolder=($_.ExecutablePath -ieq $exe); startedUtc=$_.CreationDate.ToUniversalTime().ToString('o'); startedBeforeExeWrite=($_.CreationDate -lt $exeItem.LastWriteTime); viewer=($_.CommandLine -match '--connect'); mcp=($_.CommandLine -match '--agent-mcp')} });
$network = @();
foreach ($endpoint in @(@('rs-ny.rustdesk.com',21116),@('ovh-fr2.rustdesk.com',21117),@('rs-ny.rustdesk.com',443),@('ovh-fr2.rustdesk.com',443))) {
    $hostName=$endpoint[0]; $port=[int]$endpoint[1]; $addresses=@(); $connected=$false; $errorType=$null; $client=[Net.Sockets.TcpClient]::new(); $timer=[Diagnostics.Stopwatch]::StartNew();
    try { $task=$client.ConnectAsync($hostName,$port); if (!$task.Wait(6000)) { throw [TimeoutException]::new('TCP timeout') }; $connected=$client.Connected; if($connected){$addresses=@($client.Client.RemoteEndPoint.Address.ToString())} } catch { $errorType=$_.Exception.GetType().Name } finally { $client.Dispose() };
    $network += [ordered]@{host=$hostName;port=$port;tcpConnected=$connected;remoteAddresses=$addresses;errorType=$errorType;milliseconds=$timer.ElapsedMilliseconds};
};
$sockets=@(); $socketsAvailable=$true;
try { $pids=@($all.ProcessId); $sockets=@(Get-NetTCPConnection -ErrorAction Stop | Where-Object { $_.OwningProcess -in $pids -and $_.RemotePort -in @(21116,21117,443) } | Select-Object OwningProcess,RemoteAddress,RemotePort,State) } catch { $socketsAvailable=$false };
$safeLines=@(); $logRoot=Join-Path $appRoot 'data\rustdesk\log';
if(Test-Path -LiteralPath $logRoot){
    $logFiles=@(Get-ChildItem -LiteralPath $logRoot -Filter '*.log' -File -Recurse | Sort-Object LastWriteTime -Descending | Select-Object -First 4);
    foreach($logFile in $logFiles){
        foreach($line in @(Get-Content -LiteralPath $logFile.FullName -Encoding UTF8 -Tail 500)){
            if($line -match '(?i)password|token|secret|signed|public.key|fingerprint'){continue};
            if($line -notmatch 'start tcp:|Connection secured|keep_alive:|rendezvous server:|create_relay requested|Failed to create relay connection|Failed to accept connection|Failed to start ipc|server not started:|Handshake failed|Connection opened|Connection closed|Force to update pk|relay requested from peer|used to establish Relay|Peer handshake|Relay transport|Relay pairing|Relay pre-login handshake timed out|Relay pre-login transport interrupted|Agent transport WSS connected'){continue};
            $safe=$line -replace '\b(?:\d{1,3}\.){3}\d{1,3}\b','[address]';
            $safe=$safe -replace '(?i)\[[0-9a-f:]+:[0-9a-f:.]+\]','[address]';
            $safe=[regex]::Replace($safe,'(?i)uuid[: ]+([^,\s]+)',{param($match) $hasher=[Security.Cryptography.SHA256]::Create();try{$hash=[BitConverter]::ToString($hasher.ComputeHash([Text.Encoding]::UTF8.GetBytes($match.Groups[1].Value.TrimEnd(':').ToLowerInvariant()))).Replace('-','').Substring(0,16).ToLowerInvariant();return 'uuidHash '+$hash}finally{$hasher.Dispose()}});
            $safeLines += [ordered]@{log=$logFile.Name;event=$safe};
        };
    };
};
$report=[ordered]@{diagnosticVersion=5;checkedUtc=[DateTime]::UtcNow.ToString('o');packageVersion=$manifest.version;packageIsRunning=($roots.Count -eq 1);sourceBuildId=$manifest.sourceBuildId;executableSha256=$exeHash;executableMatchesManifest=($exeHash -eq $manifest.files.'RustDesk-Agent.exe');processes=$processes;network=$network;socketsInspectionAvailable=$socketsAvailable;sockets=$sockets;logEvents=@($safeLines | Sort-Object event | Select-Object -Last 70);settingsChanged=$false;authenticationRead=$false};
$output=$env:MEGARUSTDESK_DIAG_OUTPUT;
if([string]::IsNullOrEmpty($output)){$output=Join-Path $appRoot 'MegaRustDesk-diagnostics.txt'};
[IO.File]::WriteAllText([IO.Path]::GetFullPath($output),($report|ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false));
Write-Host 'Report created: MegaRustDesk-diagnostics.txt';
