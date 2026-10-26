# MegaRustDesk: текущее исполнение

Цель по уточнению владельца: внешний Codex CLI/MCP на основном ПК управляет
вторым ПК через выбранную RustDesk-сессию. Чат и Codex runtime внутри приложения не нужны.

- [x] Session gate и прямые кадры/сетевой ввод, без локального screen/input/clipboard.
- [x] Внешний stdio MCP: initialize, tools, discovery, immutable bind, controller ownership, DPAPI capability.
- [x] UI без чата/login/task textarea: существующие кнопки доступа и просмотр.
- [x] Исходящий TCP/relay, без входящих портов и изменения Windows Firewall.
- [x] Упаковка Windows x64 EXE + Sciter; внешний Codex не включён.
- [ ] Проверить новый ZIP: hashes, relocation, external MCP, сеть, нативную UI fixture.
- [ ] Подключить сервер к локальному Codex через штатный CLI.
- [ ] Реальный второй ПК и GUI-задача, чистая Windows, отсутствие локальных помех.
- [ ] Квитанция хранения и доступная очистка. Отказы policy не обходить.

Состояние проверок и ограничения — STATUS.md.
