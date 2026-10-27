# Agent UI contract

Baseline: RustDesk 1.4.3 Sciter, `remote.html`, `remote.tis`, `common.css`.
Повторно используются `.button`, `.outline`, `color(bg/text/border/button/light-text)`.
Существующие подключение, разрешения и пароль остаются в штатном интерфейсе.

Уточнение владельца: встроенный чат и авторизация Codex не нужны.
Внешний Codex CLI/MCP управляет удалённой RustDesk-сессией.
Baseline: нативная диагностическая панель 0.1.1, проверена 2026-10-25 до изменения.
Acceptance delta: нет task textarea, истории диалога, login и model status;
есть целевой ПК, статус MCP, разрешение/пауза/стоп/ручное управление, hide/exit.
Используются те же кнопки и токены; панель 230px; окно минимум 850x500.
Просмотр использует оставшуюся область. Никаких новых UI runtime/dependencies.
До 0.3.3 manual input разрешён только после «Взять управление».
В 0.3.3 manual input включён по умолчанию; «Передать Codex» разрешает MCP-ввод.
Модель не может снять человеческую паузу.
Закрытие просмотра скрывает его; «Выйти» останавливает задачу и весь дочерний процессный job.

Acceptance: 1280x800 и 900x600, светлая/тёмная темы; disconnected/MCP idle/enabled/paused/error;
pause/resume/stop/manual, hide/show/exit; соседние подключение и password dialog.
Native Sciter и browser evidence записываются отдельно. Build не подтверждает UI.

Updater delta 2026-10-26: baseline main 0.2.3 — left-pane 200px, padding20px,
button/outline/lighter-text из index.css/common.css; отдельного updater нет.
Добавляется блок версии/кнопки под существующим блоком доступа, без новой панели/чата.
Матрица: idle/checking/current/available/downloading/verifying/ready/blocked/failed,
light/dark, 1280x800 и 900x600; keyboard/button action; соединение/пароли рядом.
Native accessibility/health, screenshots и browser gate фиксируются отдельно;
изображения с настоящими паролями в evidence не включаются. Browser gate пока не проверен.

Encoding baseline 2026-10-26: пользовательский screenshot 0.3.0 — direct HTML/TIS
русские строки искажены, native translated strings читаемы. Generated HTML содержит
корректный UTF-8, charset отсутствует. Native Sciter A/B: windows-1251 даёт те же
начальные codepoints 1056,160,1056,176; UTF-8 meta сохраняет HTML и TIS literal.
Acceptance: явный UTF-8 до scripts во всех inline documents; генератор не зависит
от Windows locale; native DOM и script строки совпадают. Компоненты/токены/геометрия
сохраняются. Реальный второй ПК и browser gate проверяются отдельно.

Singleton baseline: несколько portable main из разных папок разделяют RustDesk IPC;
новое окно может использовать старую принимающую часть. Acceptance delta: последний
обычный запуск заменяет прежние main и legacy portable --server/--tray того же
Windows user/session до инициализации data/Sciter/IPC. MCP, --connect, helper,
диагностика, неизвестные режимы и другие продукты не считаются копиями main.
При закрытии старого main его дочерние viewer могут завершиться вместе с Windows job.
Ошибки используют существующий native message box; layout/components/tokens не меняются.
Матрица: cold/repeat/cross-folder/concurrent launch, corrupt new package, worker modes,
same-user/session, parent-job отказ. Native lifecycle и browser gate фиксируются отдельно.

Viewer controls delta 2026-10-27 (request from owner): baseline 0.3.2 has a fixed
230px sidebar, overflow:auto, five overlapping ownership buttons and explanatory prose.
The connected viewer uses the remaining width; native accessibility baseline returned
no tree, so existing source/user screenshots are the recorded baseline.
Reuse .button/.outline, common.css colors, existing remote session and keyboard transport.
Acceptance: manual control by default; one ownership button reflects actual mode and
hands control to Codex or returns it to the human; no pause/resume/stop buttons/help prose.
Collapse/expand only resizes the sidebar and adapts video, never changes ownership.
A collapsed sidebar keeps an always-visible reclaim action while Codex owns input.
Only vertical sidebar scrolling; long status and translated button labels wrap.
Manual paste: explicit human button or Ctrl+V reads local text once and sends RustDesk
KeyEvent::Seq to the selected viewer; no clipboard synchronization, files or MCP access.
Read is denied before touching clipboard when disconnected, non-manual, view-only or
keyboard permission denied. Queued text is discarded after ownership/epoch changes.
Matrix: 1280x800, 900x600, minimum 850x500; light/dark; manual/running/paused/stopped/
disconnected/error; collapse/expand/reclaim; Unicode/newlines/empty/oversized paste;
adjacent toolbar, file transfer and password dialog. Native and browser fixture evidence
are recorded separately; live remote clipboard/input requires a separate human check.

Human clipboard delta 2026-10-27: restore standard Ctrl+C/Ctrl+V text/files in both
directions in MegaRustDesk (not nested Windows RDP). Reuse Windows Cliprdr, existing
toolbar/settings/permissions and explicit text-paste fallback. No geometry, tokens,
components or new MCP tools. Physical enter/key activates only a connected Manual
epoch. Focus loss preserves an authorized file operation for local Explorer paste;
handoff/pause/stop/disconnect revoke it. A pending Ctrl+V is cancelled by the ordered
focus/key release and never re-presses buffered keys afterward.

Acceptance: existing 1280x800/900x600/850x500 light/dark viewer regression; native OLE
fixture Unicode, directory and two files with exact bytes; initial copy before
activation, exact apply ACK before first V-down, old ACK/epoch/token rejection,
permission denial, timeout, bounded queue, clipboard busy and real bidirectional
copy on two PCs. Native fixture substitutes the OS clipboard write and does not
read user clipboard; it is separate from physical Windows Explorer acceptance.
