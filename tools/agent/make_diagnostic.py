"""Build a small standalone read-only network/receiver diagnostic, without user data."""
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
script=' '.join(line.strip() for line in (ROOT/'tools/agent/diagnose.ps1').read_text(encoding='utf-8').splitlines())
assert '"' not in script and '%' not in script and '\n' not in script
lines=['@echo off','setlocal','set "MEGARUSTDESK_DIAG_ROOT=%~dp0"','if not "%~1"=="" set "MEGARUSTDESK_DIAG_ROOT=%~1"','set "MEGARUSTDESK_DIAG_OUTPUT=%~dp0MegaRustDesk-diagnostics.txt"','if not "%~2"=="" set "MEGARUSTDESK_DIAG_OUTPUT=%~2"','powershell.exe -NoLogo -NoProfile -Command "'+script+'"','set "DIAG_EXIT=%errorlevel%"','if not "%DIAG_EXIT%"=="0" echo Diagnostic failed. Keep this window open.','if "%~2"=="" pause','endlocal & exit /b %DIAG_EXIT%','']
assert max(map(len,lines))<7900,'Windows CMD command length limit'
output=ROOT/'release/Проверить соединение.cmd';output.parent.mkdir(exist_ok=True)
output.write_bytes('\r\n'.join(lines).encode('ascii'))
print('Standalone diagnostic created; program files, settings and release ZIPs unchanged')
