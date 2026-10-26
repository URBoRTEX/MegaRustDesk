# Сборка MegaRustDesk с внешним MCP

Версия с updater — 0.3.0 preview. Встроенного чата, App Server и runtime Codex в поставке нет.
Codex CLI/другой MCP-клиент работает отдельно на основном ПК.
Основа: RustDesk 1.4.3 commit `6a0da9cf09b80bba6ca731d3029210c5d839a77b`,
ветка `codex/agent-portable`. Точные входы — tools/agent/inputs.json.
Подмодуль hbb_common содержит patch tools/agent/hbb-portable.patch.

Для разработки: Windows x64, MSVC C++ Build Tools + Windows SDK, Rust 1.75.0,
Python 3, libclang 16.0.6 и vcpkg commit из inputs.json.
Task build root: `C:\CodexBuilds\MegaRustDesk`; Cargo target — target,
VCPKG_ROOT — toolchain/vcpkg, LIBCLANG_PATH — toolchain/python/clang/native.
Static triplet x64-windows-static: aom/libvpx/libyuv/opus/libjpeg-turbo.
Sciter DLL и лицензия лежат в toolchain; SHA проверяется.
Конечному пользователю эти build tools не нужны.

Сборка: `powershell -File tools/agent/build.ps1 -UseTaskCargoTransport`.
Штатный inline Sciter генератор + cargo build --locked --release --features agent --bin rustdesk.
Упаковщик отказывается создавать заглушку без настоящего EXE.
Результат: release/MegaRustDesk-0.3.0-windows-x64.zip и SHA256SUMS.txt.
Новый staging .agent-build/mcp-package не принимает runtime или пользовательские data.
Пользовательские данные распакованного приложения не перезаписываются.

В этой среде Schannel не работает (SEC_E_ALGORITHM_MISMATCH).
Task Cargo home toolchain/cargo использует tools/agent/cargo_proxy.py:
loopback транспорт к официальным crates.io через проверенный Python HTTPS;
Cargo дополнительно проверяет Cargo.lock hashes. Глобальные конфиги Git/Cargo не меняются.
Это build-only средство; в portable-поставку оно не входит.

MCP entrypoint: EXE --agent-mcp, stdio JSON-RPC. Initialize/tools/list доступны без GUI.
list_sessions получает живые viewer-сессии из data/mcp-sessions. Capability защищена DPAPI
текущего пользователя, TCP listener только 127.0.0.1. bind_session неизменно привязывает
процесс клиента к одному nonce; после disconnect другая цель автоматически не выбирается.
Человек нажимает «Разрешить Codex»; модель не может снять pause/stop или manual ownership.
Перед отправкой каждого сетевого input проверяются epoch, геометрия и свежесть кадра.
Закрытие MCP stdio освобождает контроллер; GUI проверяет PID на случай аварии клиента.
Внешние диалоги и авторизация хранятся самим Codex, приложение их не читает/не копирует.

Сеть: только исходящие TCP/relay; входящий direct server, LAN discovery,
UDP/IPv6 punch отключены. Системные firewall rules и уведомления не меняются.
Прямой автономный LAN между двумя экземплярами сборки не поддерживается.

Проверки:
- Gate unit tests: rustc +1.75.0 --test --edition 2021 src/agent/gate.rs.
- MCP framing/retry tests: rustc +1.75.0 --test --edition 2021 src/agent/transport.rs.
- Temporary live transport helper: python -m unittest discover -s tools/agent -p test_live_transport.py -v.
- Windows identity DPAPI: build.ps1 -UseTaskCargoTransport -TestSecrets.
- EXE --agent-self-test: реальный stdio MCP, discovery, bind, запрет смены цели и отключённого ввода; без модели.
- python tools/agent/verify_package.py: чистая распаковка вне repo, hashes, пути с пробелами/кириллицей и перенос, без dev PATH.
- verify_network.ps1 -Executable <QA-exe>: реальный receiver startup, наблюдение TCP/UDP и read-only firewall snapshot.
- EXE --agent-ui-test: нативная UI fixture без удалённого подключения/ввода.

Эти команды не заменяют реальный второй ПК и чистую Windows.
Фактические результаты — STATUS.md и .agent-build receipts; полная приёмка пока UNVERIFIED.
Перед сборкой/очисткой действует C:/CodexMaintenance/BUILD_RETENTION.md.

## Подписанный выпуск

Single-instance проверки: `powershell -File tools/agent/build.ps1 -UseTaskCargoTransport -TestSingleInstance`.
Они запускают собственные fixture-процессы в отдельном временном scope и не закрывают
пользовательские GUI/MCP. Проверяются разные/одинаковые папки, параллельный запуск,
защита worker-ролей и data, неверный hash и отказ дочернему запуску из старого host.

Первый producer работает на локальной Windows с указанным выше toolchain.
GitHub хранит public source/assets; готового GitHub-hosted build workflow пока нет.
Существующему reqwest включён Rustls TLS для updater; остальные сетевые пути сохранены.

`tools/agent/release_config.json` задаёт версию, репозиторий и публичный Ed25519 ключ.
`python tools/agent/release.py init` проверяет существующий DPAPI private key;
не удаляйте ключ и не генерируйте новый без отдельного плана ротации доверия клиентов.
Ключ не передаётся в CI, исходники или архив. Новый разработчик может собирать fork,
но выпуск, которому доверяют существующие клиенты, требует исходного signing key.

1. Выполнить `powershell -File tools/agent/build.ps1 -UseTaskCargoTransport -TestUpdates` и product build.
2. Проверить payload и commit всех release-исходников. Реальный diff hbb_common должен совпадать с hbb-portable.patch.
3. После commit повторить package.py с тем же build-root. runtime_snapshot отклонит изменения runtime после компиляции.
4. `python tools/agent/release.py source` создаёт complete corresponding source ZIP с файлами подмодулей.
5. `python tools/agent/release.py prepare --archive release/MegaRustDesk-0.3.0-windows-x64.zip`.
6. `python tools/agent/release.py verify-prepared` повторно проверяет подпись, binary/source SHA, inventory и commit.
7. Создать и отправить tag `agent-v0.3.0` на точный release commit в свой public fork.
8. `python tools/agent/publish_release.py --notes-file docs/agent/RELEASE-0.3.0.md --publish`.

Publisher создаёт preview draft, загружает binary/source/manifest/signature/checksums,
скачивает assets и сверяет SHA до публикации. Перезапись опубликованного релиза запрещена.
Без `--publish` остаётся проверенный draft. Авторизация GitHub нужна producer, клиентам не нужна.
Stable discovery пропускает preview. Первая версия требует однократного bootstrap на каждом ПК.
