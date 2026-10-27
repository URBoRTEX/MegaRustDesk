@echo off
setlocal
set "MEGARUSTDESK_REPAIR_FILE=%~f0"
powershell.exe -NoLogo -NoProfile -Command "$ErrorActionPreference='Stop'; try { $text=[IO.File]::ReadAllText($env:MEGARUSTDESK_REPAIR_FILE); $body=[regex]::Split($text,'(?m)^# MEGARUSTDESK_REPAIR_PAYLOAD\r?$')[1]; if(!$body){throw 'Missing repair payload'}; & ([ScriptBlock]::Create($body)) } catch { Write-Host $_.Exception.Message -ForegroundColor Red; exit 1 }"
set "MEGARUSTDESK_REPAIR_EXIT=%errorlevel%"
if not "%MEGARUSTDESK_REPAIR_UNATTENDED%"=="1" pause
exit /b %MEGARUSTDESK_REPAIR_EXIT%
# MEGARUSTDESK_REPAIR_PAYLOAD
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
function RepairHash([string]$Path) {
    $hash=[Security.Cryptography.SHA256]::Create();$stream=[IO.File]::OpenRead($Path)
    try {[BitConverter]::ToString($hash.ComputeHash($stream)).Replace('-','').ToLowerInvariant()}
    finally {$stream.Dispose();$hash.Dispose()}
}
function RepairPlainPath([string]$Path) {
    $item=[IO.Path]::GetFullPath($Path)
    while($item) {
        if((Test-Path -LiteralPath $item) -and ((Get-Item -LiteralPath $item -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {throw 'Обновление через ссылку/reparse point запрещено'}
        $item=[IO.Path]::GetDirectoryName($item)
    }
}
function RepairDownload([string]$Url,[string]$Path,[long]$Limit) {
    $uri=[Uri]$Url
    for($redirect=0;$redirect -le 5;$redirect++) {
        if($uri.Scheme -ne 'https' -or $uri.Host -notin @('github.com','api.github.com','objects.githubusercontent.com','release-assets.githubusercontent.com','github-releases.githubusercontent.com')) {throw 'Недопустимый адрес обновления'}
        $request=[Net.HttpWebRequest]::Create($uri);$request.UserAgent='MegaRustDesk-repair/0.3.4';$request.AllowAutoRedirect=$false;$request.Timeout=45000;$request.ReadWriteTimeout=45000
        $response=$request.GetResponse()
        if([int]$response.StatusCode -in @(301,302,303,307,308)) {
            $next=[Uri]::new($uri,$response.Headers['Location']);$response.Dispose();$uri=$next;continue
        }
        $source=$null;$destination=$null
        try {
            if([int]$response.StatusCode -ne 200 -or $response.ContentLength -gt $Limit){throw 'Ответ сервера превышает лимит или недоступен'}
            $source=$response.GetResponseStream();$destination=[IO.File]::Open($Path,[IO.FileMode]::CreateNew)
            $buffer=New-Object byte[] 65536;$total=0
            while(($count=$source.Read($buffer,0,$buffer.Length)) -gt 0){$total+=$count;if($total -gt $Limit){throw 'Файл обновления превышает лимит'};$destination.Write($buffer,0,$count)}
            return
        } finally {if($source){$source.Dispose()};if($destination){$destination.Dispose()};$response.Dispose()}
    }
    throw 'Слишком много перенаправлений GitHub'
}
function RepairRun([string]$Exe,[string[]]$Arguments,[bool]$Capture=$true) {
    foreach($argument in $Arguments){if($argument.Contains('"')){throw 'Недопустимый аргумент'}}
    $start=[Diagnostics.ProcessStartInfo]::new();$start.FileName=$Exe;$start.Arguments=($Arguments|ForEach-Object{'"'+$_+'"'}) -join ' ';$start.UseShellExecute=$false;$start.CreateNoWindow=$true;$start.RedirectStandardOutput=$Capture;$start.RedirectStandardError=$Capture
    $process=[Diagnostics.Process]::Start($start);$output='';$errorText='';if($Capture){$output=$process.StandardOutput.ReadToEnd();$errorText=$process.StandardError.ReadToEnd()};$process.WaitForExit()
    try {if($process.ExitCode -ne 0){throw ('Ошибка проверенного updater: '+$errorText)};return $output} finally {$process.Dispose()}
}
$repairRoot=$env:MEGARUSTDESK_REPAIR_ROOT
if(!$repairRoot){$repairRoot=Split-Path -Parent $env:MEGARUSTDESK_REPAIR_FILE}
if(!(Test-Path -LiteralPath (Join-Path $repairRoot 'manifest.json'))) {
    $candidates=@(Get-CimInstance Win32_Process | Where-Object {$_.Name -like 'RustDesk-Agent*.exe' -and $_.ExecutablePath -and $_.CommandLine -notmatch '--agent-mcp'} | ForEach-Object {Split-Path -Parent $_.ExecutablePath} | Sort-Object -Unique | Where-Object {Test-Path -LiteralPath (Join-Path $_ 'manifest.json')})
    if($candidates.Count -ne 1){throw 'Положите repair-update.cmd рядом с RustDesk-Agent.exe и запустите снова'}
    $repairRoot=$candidates[0]
}
$repairRoot=[IO.Path]::GetFullPath($repairRoot);RepairPlainPath $repairRoot
$installed=Get-Content -LiteralPath (Join-Path $repairRoot 'manifest.json') -Encoding UTF8 -Raw | ConvertFrom-Json
if($installed.product -ne 'RustDesk Agent Portable' -or $installed.platform -ne 'windows-x64'){throw 'Это не папка MegaRustDesk Windows x64'}
$trusted=@{'0.3.2'='d005694706461da2ca5244a433098d48fdd78071f1090224aba282e557d42ce5';'0.3.3'='b3cdeb64344335b9941413cb0526d2dfe2e2f2958107e7908aaea2b4da5ebeca'}
if(!$trusted.ContainsKey($installed.version)){throw 'Восстановитель предназначен для 0.3.2/0.3.3. В новых версиях используйте кнопку обновления'}
$verifier=Join-Path $repairRoot 'RustDesk-Agent.exe'
if((RepairHash $verifier) -ne $trusted[$installed.version] -or $installed.files.'RustDesk-Agent.exe' -ne $trusted[$installed.version]){throw 'Файлы установленной версии не подтверждены; обновление не начато'}
$updates=Join-Path $repairRoot '.updates';RepairPlainPath $updates;[IO.Directory]::CreateDirectory($updates)|Out-Null
$repairWork=Join-Path $updates ('incoming-repair-'+[Guid]::NewGuid().ToString());[IO.Directory]::CreateDirectory($repairWork)|Out-Null
$manifestPath=Join-Path $repairWork 'update-manifest.json';$signaturePath=Join-Path $repairWork 'update-manifest.sig';$archivePath=Join-Path $repairWork 'download.zip'
Write-Host 'Получаю подписанное обновление...'
$offline=$env:MEGARUSTDESK_REPAIR_PACKAGE_DIRECTORY
if($offline){Copy-Item -LiteralPath (Join-Path $offline 'update-manifest.json') -Destination $manifestPath;Copy-Item -LiteralPath (Join-Path $offline 'update-manifest.sig') -Destination $signaturePath}
else {
    [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12
    $latestPath=Join-Path $repairWork 'release.json';RepairDownload 'https://api.github.com/repos/URBoRTEX/MegaRustDesk/releases/latest' $latestPath 1048576
    $latest=Get-Content -LiteralPath $latestPath -Encoding UTF8 -Raw|ConvertFrom-Json
    if($latest.draft -ne $false -or $latest.prerelease -ne $false -or $latest.tag_name -notmatch '^agent-v\d+\.\d+\.\d+$'){throw 'Нет опубликованного обновления'}
    Remove-Item -LiteralPath $latestPath
    $base='https://github.com/URBoRTEX/MegaRustDesk/releases/download/'+$latest.tag_name
    RepairDownload ($base+'/update-manifest.json') $manifestPath 262144;RepairDownload ($base+'/update-manifest.sig') $signaturePath 4096
}
$manifest=Get-Content -LiteralPath $manifestPath -Encoding UTF8 -Raw|ConvertFrom-Json
if($manifest.version -notmatch '^\d+\.\d+\.\d+$' -or [Version]$manifest.version -lt [Version]'0.3.4'){throw 'Исправленная версия пока не опубликована'}
$expected='MegaRustDesk-'+$manifest.version+'-windows-x64.zip';$expectedUrl='https://github.com/URBoRTEX/MegaRustDesk/releases/download/agent-v'+$manifest.version+'/'+$expected
if($manifest.asset.name -ne $expected -or $manifest.asset.url -ne $expectedUrl -or $manifest.asset.size -gt 134217728){throw 'Неверные метаданные обновления'}
if($offline){Copy-Item -LiteralPath (Join-Path $offline $expected) -Destination $archivePath}else{RepairDownload $expectedUrl $archivePath 134217728}
Write-Host 'Проверяю подпись установленным доверенным клиентом...'
$verified=RepairRun $verifier @('--agent-verify-update',$manifestPath,$signaturePath,$archivePath,$installed.version) | ConvertFrom-Json
if($verified.signatureVerified -ne $true -or $verified.inventoryVerified -ne $true -or $verified.version -ne $manifest.version){throw 'Подпись или содержимое архива не подтверждены'}
Add-Type -AssemblyName System.IO.Compression.FileSystem
$payload=Join-Path $repairWork 'payload';[IO.Directory]::CreateDirectory($payload)|Out-Null
$zip=[IO.Compression.ZipFile]::OpenRead($archivePath)
try {
    foreach($entry in $zip.Entries){$prefix='RustDesk-Agent-Portable-win-x64/';if(!$entry.FullName.StartsWith($prefix,[StringComparison]::Ordinal)){throw 'Неверный корень архива'}
        $relative=$entry.FullName.Substring($prefix.Length);$target=[IO.Path]::GetFullPath((Join-Path $payload $relative))
        if(!$target.StartsWith($payload+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Путь выходит за staging'}
        [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($target))|Out-Null
        [IO.Compression.ZipFileExtensions]::ExtractToFile($entry,$target,$false)
    }
} finally {$zip.Dispose()}
if($env:MEGARUSTDESK_REPAIR_UNATTENDED -ne '1'){Write-Host 'Полностью закройте MegaRustDesk на этом ПК, включая его окна подключения и MCP из этой папки. Данные сохраняются.';Read-Host 'После закрытия нажмите Enter'|Out-Null}
$remaining=@(Get-CimInstance Win32_Process|Where-Object {$_.ExecutablePath -and (Split-Path -Parent $_.ExecutablePath) -ieq $repairRoot})
if($remaining.Count -gt 0){throw 'Программа или MCP ещё работают из этой папки. Закройте их и запустите восстановитель снова'}
RepairPlainPath $repairWork
$runner=Join-Path $repairWork 'runner.exe';Copy-Item -LiteralPath (Join-Path $payload 'RustDesk-Agent.exe') -Destination $runner
if((RepairHash $runner) -ne $manifest.files.'RustDesk-Agent.exe'){throw 'Проверенный runner изменился'}
# Offline operator path: no old application/process/job remains to wait for.
$plan=[ordered]@{root=$repairRoot;work=$repairWork;parentPid=0;parentBirth=0;nonce=[Guid]::NewGuid().ToString();runnerSha256=(RepairHash $runner);mode='apply'}
$planPath=Join-Path $repairWork 'plan.json';[IO.File]::WriteAllText($planPath,($plan|ConvertTo-Json),[Text.UTF8Encoding]::new($false))
Write-Host ('Устанавливаю '+$manifest.version+' с сохранением data...')
RepairRun $runner @('--agent-apply-update',$planPath) $false|Out-Null
$result=Get-Content -LiteralPath (Join-Path $updates 'result.json') -Encoding UTF8 -Raw|ConvertFrom-Json
if($result.state -ne 'updated' -or $result.version -ne $manifest.version){throw 'Установка не подтверждена. Прежние файлы и данные сохранены'}
Write-Host ('Готово: '+$result.version+'. Следующие обновления доступны кнопкой.') -ForegroundColor Green
