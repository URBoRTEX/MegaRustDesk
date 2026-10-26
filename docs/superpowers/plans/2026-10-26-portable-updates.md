# Portable Updates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Выпускать проверенные подписанные релизы в публичный URBoRTEX/MegaRustDesk и обновлять portable-клиент кнопкой без замены data.

**Architecture:** Отдельные модули проверки manifest/ZIP, транзакции замены и UI/network coordinator. Отдельная копия updater выполняется вне GUI job. Producer/signing/publish работает локально; GitHub хранит исходники и versioned assets.

**Tech Stack:** Rust 1.75, Sciter, существующие reqwest/sodiumoxide/sha2/zip/winapi; Python stdlib/PyNaCl только для developer tooling.

**Spec:** docs/superpowers/specs/2026-10-26-portable-updates-design.md, согласована владельцем 2026-10-26.

## Global Constraints

- Windows x64 portable, без службы/драйвера/автозапуска/firewall изменений.
- data и внешний Codex не изменяются; чужие MCP-процессы не завершаются.
- APP_NAME RustDeskAgent и стабильный RustDesk-Agent.exe --agent-mcp сохраняются.
- Public repo URBoRTEX/MegaRustDesk; подпись Ed25519 с pinned public key.
- Новых production-зависимостей нет. Первый updater-клиент — 0.3.0.
- Native UI и browser evidence раздельны; отсутствие browser gate не скрывается.

## Review Focus

- Чужая data/старый runtime в ZIP: reject exact inventory до извлечения и публикации.
- Подписанный manifest с неподходящими version/repo/paths: fail closed до изменения файлов.
- Locked EXE/внешний MCP/активный сеанс: текущий клиент остаётся рабочим, широкого kill нет.
- Helper в Windows job или неверный PID: не закрывать GUI без готовности helper.
- Ошибка нового запуска/падение в середине замены: journal + восстановление предыдущего payload.

### Task 1: Проверка релиза, версия и developer producer

**Files:** tools/agent/release_config.json, release.py, test_release.py, package.py, verify_package.py; src/agent/update_manifest.rs.
**Interfaces:** config version/repository/public_key; verify_manifest(bytes, signature, key, current) -> Manifest; validate/extract_archive(path, manifest, destination).

- [ ] Добавить отрицательные fixture tests для signature/hash/repo/version, contaminated ZIP, traversal/case/link/limits.
- [ ] Проверить RED; реализовать общий strict inventory и подпись точных manifest bytes.
- [ ] Генерировать DPAPI signing key только в ignored local signing path; публичный ключ вне секретов в config.
- [ ] Единая версия UI/package/CLI; source identity включает exact HEAD/submodule/patch/locks.
- [ ] Проверить GREEN, пакет/исходники и отсутствие пользовательских файлов в publisher allowlist.

### Task 2: Portable apply/rollback и Windows lifecycle

**Files:** src/agent/update_install.rs, updater.rs, portable.rs, mod.rs; src/main.rs; минимальный incoming-connection counter.
**Interfaces:** check()->background status; download()->stage; prepare_apply()->helper handshake; early CLI apply(plan)->exit status.

- [ ] Тесты apply/locked file/journal/rollback/data preservation/host version health.
- [ ] Helper dispatch до Sciter и GUI job; отдельный runner path, own-job breakaway и handshake.
- [ ] Проверять exact root/PID; блокировать активные сеансы и другие MCP/UI процессы, не убивать их.
- [ ] Заменять только подписанные files; проверять новый self-test/version, иначе rollback.
- [ ] Тесты проходят; реальная QA-транзакция старый 0.2.3 → новый 0.3.0 без изменения synthetic data.

### Task 3: Кнопка и статусы

**Files:** src/ui.rs, src/ui/index.tis, docs/UI_CONTRACT.md, updater.rs.
**Interfaces:** agent_update_status/check/download/apply; JSON state/version/message/progress/action.

- [ ] Зафиксировать reuse/baseline и acceptance matrix.
- [ ] Использовать существующие button/outline/tokens; не менять соединение/пароли/agent gate.
- [ ] Показать check/current/available/download/ready/blocked/error/version; фоновые запросы не блокируют UI.
- [ ] Native states/viewports/interactions и соседняя regression; browser gate отдельно отмечен.

### Task 4: Сборка, GitHub и выпуск

**Files:** docs/agent/BUILD.md, QUICKSTART.md, STATUS.md; tools/agent/publish_release.ps1; README fork; release receipts.

- [ ] Релевантные unit tests, cargo release и portable hashes/self-test, network startup.
- [ ] Независимый review diff и безопасность updater; исправить findings.
- [ ] Создать public fork URBoRTEX/MegaRustDesk, сохранить upstream origin.
- [ ] Secret/ignored/generated preflight перед explicit git add/commit/push feature branch; patch подмодуля опубликован.
- [ ] Подписать versioned binary/source assets, создать draft, повторно проверить перед upload/publish.
- [ ] Первый bootstrap пометить preview, пока полная UI/portable приёмка не завершена.
- [ ] Проверить public release download/signature; client stable check и QA apply/rollback.
- [ ] Retention current/previous/private quarantine, buildVerified/cleanupCompleted и открытые ограничения.

## Execution

Выполняет основной агент в текущей сессии; подагенты используются только для независимого read-only review, согласно AGENTS владельца. Дополнительное согласование метода не требуется: владелец подтвердил реализацию спеки. Публикация public repo/release разрешена выбором владельца; секреты CI не создаются.
