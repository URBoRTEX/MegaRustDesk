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
Manual input разрешён только после «Взять управление».
Кнопка «Разрешить Codex» разрешает MCP-ввод; модель не может снять человеческую паузу.
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
