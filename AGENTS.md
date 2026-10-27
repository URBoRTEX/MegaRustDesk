# RustDesk Agent Portable

Продолжать официальный RustDesk 1.4.3, upstream commit `6a0da9cf09b80bba6ca731d3029210c5d839a77b`.
Ветка: `codex/agent-portable`. Пользовательский запрос: `docs/agent/REQUEST.md`.

- Цель — Windows x64 portable, без установки службы/драйверов/автозапуска.
- Агент получает только декодированные кадры конкретной RustDesk-сессии.
- Ввод только через RustDesk messages. Запрещены локальные SendInput, clipboard и screen capture для агента.
- Настройки RustDesk/Codex отделены от пользовательских глобальных настроек.
- Не включать реальные auth, пароли, историю и data в распространяемый архив.
- Feature `agent` использует существующий Sciter UI (`src/ui`), токены common.css.
- Уточнение владельца: встроенный чат/App Server/runtime Codex удалены из продукта.
- Codex CLI/другой клиент MCP работает отдельно на основном ПК; stdio entrypoint `--agent-mcp`.
- Доступ к кадрам/вводу только выбранной bind_session; разрешение/пауза/стоп — в RustDesk.
- Сеть 0.2: исходящие TCP/relay; входящие порты, UDP/IPv6 punch и LAN discovery отключены.
- Проверки безопасности gate можно запускать отдельно: `rustc --test src/agent/gate.rs`.
- Build: `tools/agent/build.ps1`; результаты и фактические ограничения — `docs/agent/STATUS.md`.
- Перед сборкой/очисткой читать `C:/CodexMaintenance/BUILD_RETENTION.md`.
- Не считать исходники/compile/fixture полноценным portable-релизом.
- Обновления 0.3: public URBoRTEX/MegaRustDesk; signed exact manifest bytes, pinned Ed25519 key из tools/agent/release_config.json.
- Private signing key хранится только локально DPAPI в ignored .agent-build/signing; не публиковать и не выводить.
- Публикация: tools/agent/release.py source/prepare/verify-prepared, затем publish_release.py; первый bootstrap — preview.
- Подпись разрешена только после commit чистых release-исходников и проверки runtime_snapshot; изменённый hbb_common поставляется целиком в source ZIP плюс patch.
- Updater заменяет только подписанный inventory, сохраняет data, блокируется при чужом MCP/активном сеансе; запрещено обходить блокировку завершением чужих процессов.
- Singleton 0.3.2: обычный запуск заменяет только подтверждённые portable main/legacy --server/--tray того же Windows user/session, включая другие папки. MCP/viewer/helper/неизвестные роли не закрываются напрямую. При завершении прежнего main его job может закрыть его viewer.
- Новый пакет проверяется до takeover; candidate проверяется по manifest+SHA256 и удержанному HANDLE/birth/path. Неподтверждённый EXE или родительский host требуют ручного закрытия. Проверки: build.ps1 -TestSingleInstance.
- Viewer 0.3.3: manual по умолчанию; одна кнопка передачи Codex/возврата, сворачиваемая правая панель с доступным reclaim. API pause/stop и запрет автоматического resume сохраняются.
- Только физический Ctrl+V/кнопка человека могут прочитать local text clipboard в Manual+connected+keyboard allowed; это не инструмент агента/MCP и не clipboard sync. ManualText привязан к epoch; разрешения проверяются повторно в IO. Файлы/изображения через буфер не включены.
- При focus loss/handoff известные удержанные клавиши отпускаются безусловным neutral release batch (ControlKey/Chr/Win2winHotkey, только up, без modifiers/Seq). Ручная вставка сохраняет физические модификаторы с учётом receiver fix_modifiers.
- Проверки viewer: rustc --test tools/agent/test_manual_input.rs; tools/agent/test_manual_protocol.ps1; tools/agent/test_viewer_controls.ps1; test_viewer_controls_browser.cjs (browser fixture, не полный Sciter viewer).
