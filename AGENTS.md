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
- В 0.3.3 физический Ctrl+V/кнопка человека давали только text Seq. Уточнение владельца 2026-10-27 для 0.3.6: человеческий двусторонний clipboard текста/файлов включается действием в Manual+connected+keyboard allowed, с настройками и разрешениями сеанса. MCP не может активировать или читать локальный clipboard; handoff/pause/stop/disconnect отзывают native token. Кнопка «Вставить текст» остаётся явным text fallback.
- При focus loss/handoff известные удержанные клавиши отпускаются безусловным neutral release batch (ControlKey/Chr/Win2winHotkey, только up, без modifiers/Seq). Ручная вставка сохраняет физические модификаторы с учётом receiver fix_modifiers.
- Проверки viewer: rustc --test tools/agent/test_manual_input.rs; tools/agent/test_manual_protocol.ps1; tools/agent/test_viewer_controls.ps1; test_viewer_controls_browser.cjs (browser fixture, не полный Sciter viewer).
- С 0.3.3 опубликованные обновления доступны установленной кнопке: обычный release, prerelease=false, Latest. publish_release.py по умолчанию использует этот канал; --preview исключает сборку из кнопки только при явном запросе на preview. Подпись/inventory/source gates и ограничения QA сохраняются.
- Updater 0.3.4 проверяет child HANDLE именно относительно собственного job до выхода GUI; внешний job допускается и не изменяется. Recovery до создания своего job не требует breakaway. Native gate: tools/agent/test_update_job.ps1.
- Для сломанного updater 0.3.2/0.3.3 есть одноразовый repair-update.cmd: pinned SHA старого verifier, signed ZIP validation до выполнения candidate, offline parentPid=0 только после закрытия всех процессов этой папки. Не убивает процессы, не читает/заменяет data, не меняет внешние jobs или ExecutionPolicy.
- Диагностика 0.3.5 различает relay transport/pairing и peer handshake send/receive; это только контекст ошибок, без смены timeout/protocol/security. diagnose.ps1 v3 сохраняет этапы и канонический UUID hash (без завершающего ':'); regression tools/agent/test_diagnose.ps1. До нового physical runtime evidence причину intermittent pre-password timeout не считать установленной.
- В 0.3.7 только типизированный pre-login peer receive timeout допускает максимум три fresh connection attempts; login/input, security и неизвестные ошибки не повторяются. Close отменяет connect/backoff, очередь ввода не переносится. Watchdog установленного сеанса начинается после handshake. Проверка: tools/agent/test_connection_retry.ps1. Это восстановление после наблюдаемого сбоя; причина intermittent public relay pairing не доказана. Диагностика v4 выбирает работающий main текущего Windows session и явно помечает offline package.
- С 0.3.8 agent::network переводит публичный rs-ny:21115/21116 и relay *.rustdesk.com:21117 в certificate-verified WSS/443. Частные endpoints и настроенный proxy не перенаправляются; config/API не меняются; hbb добавляет is_tls для существующего WebSocket. secure_tcp отличает TLS от обычного WebSocket, peer E2E handshake сохранён. До первого peer message допускаются также typed EOF/reset и точный upstream WS reset; повреждённые сообщения, crypto/TLS failures и post-handshake send не повторяются. Проверки: test_public_wss.ps1 [-Live], test_connection_retry.ps1; native pair arrival gap — только fixture, не production delay. Для исключения проблемного raw TCP leg обновляются оба ПК. Диагностика v5 показывает WSS connections/443.
- Уточнение 0.3.9: постоянный start_tcp регистрации использует connect_registration и прежний signed TCP; короткие coordinator queries/responses и relay data остаются WSS. На public WSS-регистрации 0.3.8 зафиксированы ~18-секундные resets; не переносить её снова на WSS на основании только успешного relay-теста. Regression test_public_wss.ps1 -Live проверяет также pinned RS exchange + encrypted TCP query; проверка сохранения регистрации после обновления выполняется отдельно.
- Clipboard 0.3.6: docs/agent/CLIPBOARD.md. Windows Cliprdr/OLE переиспользуется; grant сохраняет завершение paste-out после focus loss, но не переживает смену Manual epoch/token. Native callback TLS, channel envelopes, delayed workers и OLE objects/waits проверяют поколение. Context suspension не уничтожает viewer OLE context; teardown отзывает access до ожидания native thread.
- Первый файловый Ctrl+V ждёт exact apply ACK после OleSetClipboard. Constant format marker carries monotonic operation ID; response uses existing msg_flags bits, low standard flags preserved. Оба клиента должны быть 0.3.6. Очередь input 256, timeout 5 секунд; revoke/focus release отменяет её без повторного ввода. Tests: test_clipboard_access.rs, test_clipboard_barrier.rs, test_clipboard_native.ps1; native fixture не читает пользовательский clipboard, OS OLE write подменён только в тесте. Physical two-PC acceptance проверяется отдельно.
