@echo off
setlocal
set "CODEX_EXE=codex.exe"
where codex.exe >nul 2>nul
if errorlevel 1 (
  set "CODEX_EXE=%LOCALAPPDATA%\Programs\OpenAI\Codex\bin\codex.exe"
)
rem Register only this server; Codex preserves its other MCP connections/settings.
"%CODEX_EXE%" mcp add megarustdesk -- "%~dp0RustDesk-Agent.exe" --agent-mcp
if errorlevel 1 (
  echo MCP registration failed. Install or update Codex CLI, then retry.
) else (
  echo MCP registered. Restart Codex and use the megarustdesk tools.
)
pause
