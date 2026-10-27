# MegaRustDesk: фактический статус

## 0.3.6 — человеческий двусторонний clipboard, 2026-10-27

Причина отказа файлового Ctrl+V в MegaRustDesk: agent запрещал clipboard/files,
а физический Ctrl+V поглощался текстовым Seq. Восстановлен Windows Cliprdr/OLE
в обоих направлениях, только для активированного человеком Manual epoch.
MCP не получает clipboard/file API и не может выдать grant. Handoff/pause/stop/
disconnect отзывают native token; focus loss сохраняет завершение paste-out.
Настройки мигрируют один раз на peer и затем сохраняются.

Первый Ctrl+V читает актуальный человеческий snapshot. Receiver text update
сериализован перед следующей клавишей. Файлы ждут exact operation ACK после
OleSetClipboard: constant marker/существующий Cliprdr response, без нового proto.
Оба клиента должны быть 0.3.6. Старый/отсутствующий/failed ACK не вставляет прежний
буфер. Input queue: 256 событий/5 секунд; отмена — neutral releases без press/Seq.

Native callback TLS и envelopes сохраняют generation; worker повторно проверяет
epoch/token/permissions. OLE objects и waits не возобновляются после revoke/regrant.
Viewer context при handoff не уничтожается; CM/teardown отзывает доступ до join.
Реальные auth/data пользователя не используются тестами и не входят в пакет.

Проверено до release build: cargo check; access/ACK codec 5; barrier 3; manual
gate/text regression 17; neutral/manual protocol; native OLE synthetic paste x2
(Unicode, каталог, два файла, 2 100 242 байта в каждом прогоне), deny/stale OLE/
wait, sequence marker/posted apply ACK. В native fixture подменён только OS OLE
write, пользовательский clipboard не читается. Browser viewer fixture 6/6:
1280x800/900x600/850x500, light/dark, ownership/collapse/reclaim/paste/overflow.
Compile, эти fixture и browser panel не подтверждают physical Explorer transfer.
Полный двухкомпьютерный gate и multi-peer CM/global clipboard остаются UNVERIFIED.

Release compile завершён exit 0 за 6m31s, runtime snapshot
`e80a1912b152229e3878c2ff3c692bb457bf154f12c891d864ed2899c5bc7329`.
Первый compile 7m55s не был упакован: snapshot gate обнаружил добавленное во время
компиляции сообщение ошибки; после этого выполнена полная повторная сборка.
Ошибки вставки используют существующий custom-dialog, который не закрывает viewer;
намеренный revoke предупреждения не показывает. Native Rust/C ABI: 216 байт,
access callback offset 208. Полная физическая/UI приемка остаётся UNVERIFIED.


## 0.3.5 — уточнение диагностики соединения, 2026-10-27

Свежий отчёт второго физического ПК подтверждает 0.3.4, matching EXE и один main.
Запросы create_relay доходят, coordinator handshake завершается, затем возникает
общий deadline до пароля. TCP relay доступен с обоих ПК; synthetic pairing двух
сокетов основного ПК проходит. Handshake-only probe собственного receiver получает
первый кадр; probe второго ПК его не получает. Авторизация и ввод не отправлялись.
Эти проверки не доказывают исправность межсетевого пути или причину отказа.

В client/server добавлен контекст ошибок отдельно для relay transport, pairing,
handshake send/receive и timeout/I/O; server сохраняет полную цепочку причины.
Таймауты, протокол, маршрутизация и проверки ключей не меняются. Исправление самого
соединения НЕ заявлено: для установления причины нужен новый runtime-отказ с этапом.
Diagnostic v3 сохраняет этапы и одинаково хеширует UUID с завершающим двоеточием
и без него. Реальный regression прошёл RED → GREEN; секреты и адреса скрываются.
Новый CMD не включается в inventory: прежние клиенты разрешают только известные
root-файлы. Пакет сохраняет совместимость без расширения allowlist. Уже отправленный
CMD v2 тоже экспортирует receiver-ошибку с новым этапом. v3 остаётся отдельным
инструментом и использует manifest, EXE hash, процессы, сокеты и отфильтрованные логи.

## 0.3.4 — исправление Windows job в updater, 2026-10-27

Скриншот второго ПК 0.3.2 показал отказ «Updater остался внутри Windows job».
Нативный regression воспроизвёл причину: breakaway helper вышел из собственного
KILL_ON_CLOSE job и пережил завершение parent, но сохранил внешний non-breakaway job.
Прежний IsProcessInJob(NULL) запрещал этот безопасный сценарий.
Теперь parent проверяет точный child HANDLE относительно своего job после spawn и
перед выходом GUI. Handle не наследуется; обычный child по-прежнему завершается вместе
с app. Внешние job-политики не меняются. Recovery до создания app job не требует breakaway.
Parent identity, ready/nonce, signed inventory, журнал, health и rollback сохранены.
Read-only review job-fix не обнаружил P1/P2; ревьюер runtime не запускал.

Старый 0.3.2/0.3.3 копирует свой EXE в runner и не получает это исправление из ZIP до
установки. repair-update.cmd выполняет однократный offline переход: pinned старый
verifier проверяет signed ZIP до extraction/execute; пользователь закрывает процессы
именно этой папки; новый verified runner использует существующий operator parentPid=0.
Никаких kill, изменения ExecutionPolicy или обращения к data. Capture stdout отключён
у постоянного runner/GUI; verifier остаётся коротким процессом с JSON capture.
Проверен отказ пакету ниже 0.3.4: реальные installed EXE/manifest сохранены, GUI не закрывался.
CMD требует CRLF; это закреплено в .gitattributes. Старые cleanup-отказы не повторялись.

Release compile: exit 0, 6m30s, runtime snapshot
`e9ff4b3bccd15046895071a83c464d96729022aeaf296ad87d9ab93ee6ee5834`.
Portable self-test: 2/2 путей, 13 MCP tools, неизменные глобальные PATH/Codex metadata.
Producer 6/6; publisher 3/3 (включая отдельный repair asset). UI/сеть/ввод не менялись.
Фактический результат repair перехода сохраняется отдельно в .agent-build receipts.
На физическом втором ПК проверка ещё требует запуска пользователем; clean-Windows
и полный native/browser gate остаются UNVERIFIED, buildVerified=false.

## Доступность обновления 0.3.3 по кнопке, 2026-10-27

Причина отсутствия обновления: publisher всегда выставлял prerelease=true, а клиенты
0.3.x читают /releases/latest и исключают prerelease. До исправления endpoint давал 404.
У опубликованной 0.3.3 изменены только metadata release: prerelease=false, Latest.
Все пять asset IDs, подписанный manifest и исходные архивы сохранены.
Анонимный запрос с User-Agent MegaRustDesk/0.3.2 возвращает HTTP 200 и agent-v0.3.3.
Сам EXE 0.3.2 успешно проверил подпись и inventory 803 файлов 0.3.3.
Установка на работающий основной/второй ПК в этом действии не выполнялась.

Publisher теперь по умолчанию выпускает обычный Latest release; preview доступен
явным --preview. Signing/source/inventory/download verification остаются обязательными.
Regression default channel прошёл RED→GREEN; preview opt-in проверен отдельно.
Новых binaries/архивов не создавалось; 0.3.3 остаётся привязана к tag 4745029.
Полная native/clean-Windows/physical paste приёмка остаётся UNVERIFIED.

## 0.3.3 preview — управление и компактная панель, 2026-10-27

По запросу владельца manual включён по умолчанию. Одна кнопка передачи Codex/возврата
заменяет start/pause/resume/stop/manual. API pause/stop и обязательное человеческое
разрешение агента сохраняются. Правая панель сворачивается 256→38px, увеличивая видео;
в свёрнутом виде доступен возврат контроля. Убраны help-текст и горизонтальный scroll.

Кнопка/Ctrl+V человека читает только local text clipboard и отправляет RustDesk Seq.
До чтения проверяются Manual+connected+keyboard/view-only. ManualText связан с epoch;
в IO разрешения проверяются перед каждым сообщением. Физические модификаторы после
вставки восстанавливаются с учётом receiver fix_modifiers/press_modifiers.
На handoff/focus loss известные удержанные клавиши отпускаются отдельным нейтральным
batch, включая raw Map и Win2winHotkey Translate; текст через этот batch невозможен.
В MCP нет clipboard-метода, автоматический clipboard sync не включён.

Gate/pre-read 17/17; protobuf/Unicode/256 сочетаний модификаторов/нейтральные release
прошли. Sciter fixture с настоящими HTML/CSS/TIS и BUTTON_CLICK: 6/6; геометрия и
состояния Manual/Running/Paused/disconnected/error, возврат/collapse/hide/show проверены.
Browser fixture: 6/6, 1280×800/900×600/850×500, light/dark, keyboard Enter, длинный текст;
scrollWidth==clientWidth, JS errors 0. Для browser fixture Sciter flow заменён на flex:
это не полный browser gate native viewer, который остаётся UNVERIFIED.
Read-only review не обнаружил оставшихся P1/P2; runtime ревьюером не запускался.

Release build: exit 0, 7m05s, runtime snapshot
`a27a8f025f66c37639262c425336ccd205b3a3330aece0717f05e7ed9a70b4e3`.
Первый build остановлен для исправления review; следующая попытка обнаружила missing
Interface trait, исправленный fully-qualified вызовом. Проверки не отключались.
Portable self-test 2/2 в путях с пробелами/кириллицей без developer PATH; 13 MCP tools,
binding/target-switch guard прошли. Метаданные глобального Codex и PATH не изменились.
Producer tests 6/6 и encoding 2/2 прошли. Архив содержит только allowlist без data/runtime.

На 0.3.2 пользователь подтвердил подключение к 113203914; 3 последовательных чтения
его статуса показали connected=true. Причина прежних таймаутов не установлена.
В этой задаче сетевой код не менялся; рабочая GUI-сессия не перезапускалась.
На физическом втором ПК вставка/новый UI ещё не проверены; clean-Windows и полный
native/browser acceptance не завершены. buildVerified=false; cleanupCompleted=false
из-за ранее заблокированных generated путей, перечисленных в release/retention.json.
Ротация: current 0.3.3 preview, единственный rollback 0.3.2; receipts находятся
в .agent-build/package-check-0.3.3.json и release/retention.json.

## 0.3.2 candidate — один главный экземпляр

Обычный запуск/--noinstall выполняет проверку пакета, затем заменяет более ранние
подтверждённые main/legacy --server/--tray того же Windows пользователя и сеанса.
Проверяются product/platform manifest, настоящий SHA256 EXE, args, SID/session,
путь и FILETIME через удержанный HANDLE. Новый EXE не закрывает более новый запуск.
Короткий named mutex сериализует admission и освобождается до normal core startup.
MCP/--connect/--cm/helper/диагностика/неизвестные режимы не являются main-кандидатами.
Дочерний запуск старого host отклоняется; закрытие старого main может закрыть его viewer job.
При несовпадении hash takeover останавливается до закрытия любого экземпляра.

Пять standalone/native проверок прошли: whitelist ролей; mismatch сохраняет старый
процесс; дочерний запуск сохраняет parent и другие host; разные и одинаковая папки
сохраняют MCP/viewer/other product и synthetic data; параллельный запуск оставляет один main.
RED→GREEN сохранены для whitelist и настоящего случая mismatched EXE. Независимое
review после исправления сравнения SHA не нашло остаточных P1/P2.
Release build прошёл за 6m25s; runtime snapshot совпал после компиляции.
Portable self-test прошёл в двух путях, включая кириллицу, без developer PATH;
13 MCP tools/binding и запрет смены цели подтверждены. Producer 6 tests и encoding
2 tests прошли. Подпись и inventory 803 файлов проверяются перед публикацией.
Native process fixtures и packaged CLI отделены от Sciter UI/browser/clean-Windows
приёмки. Итоги реального запуска и установки сохраняются в ignored .agent-build receipts.
Связь со вторым физическим ПК ещё не подтверждена; общий buildVerified остаётся false.

## 0.3.1 candidate — кодировка и неполное обновление

Generated UTF-8 строки не имели явного charset; Sciter с Windows-1251 воспроизводит
искажение HTML и TIS literal. Во все inline documents добавлен UTF-8 meta до scripts;
все чтения генератора используют UTF-8 независимо от locale Windows.
Regression: генератор с принудительным CP1251 default ранее терял русские строки,
после исправления сохраняет их. Native Sciter A/B проверяет DOM и TIS literal.

До запуска GUI/принимающей стороны проверяются compiled version, manifest и hashes
EXE/Sciter. Смешанные после распаковки версии показывают явную ошибку до загрузки DLL
и data. Тест реального случая old EXE + new manifest прошёл RED → GREEN.

При текущей диагностике на основном ПК manifest обновлён до 0.3.0, но настоящий EXE
по SHA совпадает с 0.2.3. Использующие его внешние MCP-процессы не завершались.
Проверенный отдельный viewer 0.3.0 получает RelayResponse и открывает relay TCP,
затем ждёт ответ peer handshake 18 секунд до timeout. Ошибка соединения не объявляется
исправленной этим текстовым патчем. Перезапуск принимающего ПК и receiver evidence ожидаются.
Release build 0.3.1 прошёл за 6m28s, runtime snapshot совпал. 8 security + 8 installer,
6 producer и 2 encoding regression tests прошли. Portable self-test в двух путях прошёл,
включая кириллицу, без developer PATH; глобальные PATH/Codex metadata не изменились.
0.3.1 остаётся локальным candidate: новое соединение со вторым ПК ещё не подтверждено.

## 0.3.0 preview — подписанные portable-обновления

Спецификация согласована владельцем; реализация добавлена в update_manifest.rs,
update_install.rs, update_process.rs, updater.rs и главное Sciter окно.
Клиент проверяет public GitHub stable Releases; preview bootstrap скачивается один раз вручную.
Ed25519 подпись exact bytes, pinned public key, inventory/ZIP/hash, Windows paths и лимиты
проверяются до установки. Отдельный helper сохраняет data, ведёт журнал и восстанавливает
payload при неуспешном запуске. Чужие MCP/просмотры и входящие сеансы блокируют установку.

Локальные проверки ядра: 7 security + 8 installer tests прошли, включая реальные Windows
file locks, прерванный журнал/backup, откат и снятие lock перед повторным запуском.
Producer: 6 тестов contaminated ZIP, inventory/hash/Windows paths и source ZIP прошли.
Независимое review исправило recovery в mapped EXE, crash journal temp, partial backup
и restart до unlock. Повторное review process diff не обнаружило P1/P2.

Product release build прошёл на Rust 1.75 за 5m54s; snapshot исходников после компиляции
совпал. Portable self-test прошёл в двух путях, включая кириллицу, без developer PATH;
13 MCP tools, binding и запрет смены цели подтверждены. Глобальные PATH/Codex metadata
не изменились. Реальный EXE подтвердил version 0.3.0 и подпись/inventory 803 файлов.

QA helper действительно установил 0.2.3 → 0.3.0, exit0; новая GUI подтвердила nonce health,
показала «Обновление установлено», synthetic data сохранилась. Runtime MCP при активном
update lock и незавершённом recovery отклонён до старта без вывода JSON/данных.
Первый QA harness получил TimeoutExpired из-за унаследованных stdout pipes GUI;
повторный запуск с null stdout/stderr прошёл. Это ошибка harness, не признак зависания helper.

Ограничения: native accessibility видит версию/кнопку/результат, но click по index
отвергнут Computer Use как unavailable; interaction/viewports/themes не подтверждены.
Пользователь остановил управление интерфейсом Esc; GUI recovery после искусственного
прерывания не запускался, соответствующий QA snapshot сохранён. Unit crash recovery прошёл.
Network startup diagnostic exit -1: уже работающий RustDesk держит общий IPC query pipe
(os error5). Чужие процессы не завершались; изолированный network gate 0.3.0 не подтверждён.

Точные SHA и QA receipts хранятся локально в ignored .agent-build. `buildVerified=false`: browser не
исполняет Sciter/TIS, browser gate UNVERIFIED; clean Windows VM и полный сценарий
на втором ПК не подтверждены. GitHub-hosted CI не настроен: producer пока локальная Windows.
Публичный fork URBoRTEX/MegaRustDesk создан; 0.3.0 предназначен для preview-публикации
с binary/source/manifest/signature/checksums. Реальный ID второго ПК неизвестен;
его отсутствие окна и соединение этим updater-патчем не объявляются исправленными.

Ниже сохранена история предыдущей проверенной 0.2.3 и диагностики поставки.

По последнему уточнению владельца продукт работает через внешний Codex CLI/MCP.
Чат, поле задачи, вход в Codex и встроенный App Server удалены из работающего приложения.
Runtime Codex не включается в новый ZIP. Задачи и авторизация остаются во внешнем Codex.
Прежний REQUEST.md сохранён как история исходных требований; это уточнение имеет приоритет.

Схема: внешний Codex на первом ПК → stdio MCP этого EXE → конкретная сессия RustDesk
→ штатное принимающее ПО на втором ПК. Codex на втором ПК не требуется.
Человек подключает второй ПК и разрешает Codex; MCP не может снять человеческий stop/pause.
Один MCP-процесс имеет неизменную целевую сессию, кадр и ввод проверяются перед отправкой.
Закрытие MCP stdio и авария процесса снимают управление; PID проверяется локально.
Ключ loopback API защищён Windows DPAPI для текущего пользователя.

Изменены src/agent/mod.rs, mcp.rs, portable.rs; src/common.rs, client.rs,
rendezvous_mediator.rs, lan.rs; src/ui/remote.html, remote.css, remote.rs, agent.tis;
src/main.rs; package.py, verify_package.py, verify_network.ps1, connect-codex.cmd,
inputs.json; инструкции AGENTS.md, UI_CONTRACT.md, QUICKSTART.md, BUILD.md, PLAN.md.
Неиспользуемые исходники прежнего App Server не компилируются и не входят в продукт.

## Проверки

0.2.0: реальная сборка, hashes архива и self-test в путях с пробелами/кириллицей прошли.
MCP: handshake, 13 tools, discovery/bind, запрет смены цели и отключённого ввода прошли.
Receiver startup 25 секунд: исходящий TCP наблюдался, TCP listeners/UDP endpoints — ноль,
firewall rules/profiles до/после совпали. Это не проверка соединения с удалённым ПК.
Нативная UI fixture показала панель без чата. Полная UI/browser матрица не выполнена.
0.2.1: release build завершён за 8м56с, 31 предупреждение существующего кода.
Архив SHA256: `48f8d2f3977d67429ab3e397315c9525dbab616054ab233a3e3316c20d7109db`.
sourceBuildId: `e056be91af50e6ef8f4545008a607c8525c4ab866279e67c88e5bf3513311dea`.
Hashes прошли. В первом portable прогоне ASCII-путь дал `Session discovery failed`,
кириллический прошёл. Обёртка чтения лога первоначально скрыла exit code; при отдельном
чтении receipt ошибка обнаружена. Повтор с явной проверкой exit code прошёл 2/2:
MCP handshake/discovery/bind и ограничения прошли. Причина первого сбоя не установлена,
нестабильность поиска MCP остаётся открытой; повтор не отменяет первого неуспешного теста.
Развёрнутая папка обновлена по allowlist; hashes совпали, метаданные data не изменились.
Повтор реального подключения с инициатором 0.2.1 и receiver 0.2.0 дал тот же timeout.
Аутентификация не достигнута, кадры не получены, удалённый ввод не отправлялся.
Обновление receiver и повторная реальная проверка ожидаются от пользователя.

Network startup test 0.2.1: при параллельно открытом приложении первый запуск завершился -1;
причина отдельно не доказана. После закрытия принадлежащих задаче GUI-процессов отдельный
25-секундный запуск прошёл: исходящий TCP, ноль listeners/UDP, firewall snapshot неизменен.
Первый неуспешный запуск не скрывается и не считается пройденным тестом.
Ранее gate unit tests прошли 11/11, identity DPAPI synthetic test — 1/1.
Исторические 8/8 history tests относятся к удалённой из продукта встроенной истории чата.
Read-only review внешнего MCP проверил ownership, cleanup и исключение runtime;
после исправлений конкретных блокеров не обнаружено. Это не runtime evidence.

Сетевая политика сохранена: только исходящие TCP/relay, без входящего direct-server,
UDP/IPv6 punch и LAN discovery. Брандмауэр и его уведомления не меняются.
На кандидате 0.1.1 реально наблюдались исходящее соединение, ноль TCP listeners,
ноль UDP endpoints; firewall rules/profiles не изменились. Новый кандидат проверяется отдельно.

## Не проверено

Клики, ввод кириллицы и сохранение файла на реальном втором ПК;
модель во внешнем Codex с новым MCP; отсутствие помех на первом ПК во время этой GUI-задачи;
pause/stop/manual/reconnect и выполнение со скрытым просмотром на реальной сессии;
чистая Windows без старых firewall rules; полная матрица нативного UI и browser gate.
Пользователь предоставил ID второго ПК для проверки; пароль не предоставлен и не запрашивался.

## Ошибка соединения и исправление 0.2.1

На 0.2.0 воспроизведён реальный timeout согласования до аутентификации:
`Failed to connect via rendezvous server: Please try later(0)`.
Пользователь подтвердил 0.2.0 (архив 17,6 МБ) на втором ПК.
Read-only OnlineResponse сервера показал целевой ПК онлайн.
Protocol probe: сначала plaintext TestNatRequest на coordinator TCP 21116 не ответил
за 4 секунды, а защищённый запрос получил TestNatResponse. Повтор дал ответ обоим
режимам (0,39/0,56 с). Следовательно, обязательность шифрования сервером не доказана,
и ранний timeout не подтверждает причину ошибки. Повтор PunchHoleRequest также дал
timeout, включая разовый timeout самого TCP connect; удалённая 0.2.0 ещё не обновлена.

В agent-режиме три коротких coordinator TCP-пути теперь используют существующий secure_tcp:
исходящий PunchHoleRequest, fallback RequestRelay и встречный RelayResponse принимающей стороны.
Ранее первые два зависели от account token, а третий отправлялся без шифрования.
Регистрационный TCP уже использовал secure_tcp. Relay pairing 21117 и E2E не меняются.
Это согласование защиты транспорта с регистрационной сессией, а не доказанный fix timeout.
Для проверки кандидата требуется обновить оба ПК. До реальной проверки ошибка не устранена.
Private/OSS hbbs без KeyExchange не проверен: secure_tcp может ждать handshake timeout.

## Повтор после обновления receiver и исправление 0.2.2

Пользователь подтвердил установку и запуск 0.2.1 на втором ПК.
Реальный initiator 0.2.1 получил RelayResponse и установил relay TCP через ovh-fr2.rustdesk.com;
подробный журнал показал 3,76 с согласования и 324 мс relay connect. Ранний timeout пройден.
Следующий барьер — `Handshake failed: invalid message type`, до аутентификации и кадров.

Причина найдена в локальном portable patch hbb_common: Config::get_key_pair читал raw load_,
обходя DPAPI-расшифровку Config::load. После перезапуска cached secret содержал RDAP1 blob,
а server::create_tcp_connection требует 64-байтный secret для отправки SignedId. При другой
длине SignedId пропускался, соединение переходило к Hash, который клиент не ожидал.
Regression reloaded_portable_identity_can_sign до исправления реально FAILED с ошибкой длины.
Тест создаёт только синтетическую пару в новой временной папке; реальные data/auth не читаются.

0.2.2 использует общий agent_unlock_identity в Config::load и get_key_pair до кэширования,
проверяет длины secret/public 64/32, без рекурсивных CONFIG/KEY_PAIR locks. При повреждении
сбрасываются обе части пары, пароль и подтверждения регистрации; async store сохраняет reset.
Проверяются сохранение подписи после reload и fail-closed reset повреждённого synthetic blob.
Read-only review не обнаружил блокирующих замечаний; runtime подтверждается отдельно.
Green regression прошёл 2/2 за 2м28с компиляции: защита DPAPI и подпись после reload,
включая сброс повреждённого synthetic blob. Build 0.2.2 завершён за 11м02с.
SHA256 ZIP: `0567c5568de66df8e512353b72ca1e7f9becff5d645bc68b054e394601e6da2d`.
sourceBuildId: `cdfa0ddcb2443f74f9fe58ae9f694e39a5714870f0955699e43fa8e593c39182`.
Portable self-test 0.2.2 прошёл с первого прогона 2/2 (пробелы/кириллица, перенос, без dev PATH),
archive hashes совпали. Изолированный network startup прошёл: исходящий TCP, ноль listeners/UDP,
firewall rules/profiles до/после совпали. Это не замена аутентификации и удалённым кадрам.
Основной ПК обновлён по hashes/allowlist; метаданные пользовательской data не изменились.
Пользователь подтвердил обновление receiver на 0.2.2; результаты реального повтора — ниже.

## Реальная проверка соединения и MCP на 0.2.2

Оба ПК на 0.2.2 по подтверждению пользователя и проверенному локальному manifest.
Первая попытка установила relay TCP, затем дала `deadline has elapsed` через 18 секунд;
причина разового таймаута не установлена. Чистый повтор без изменения кода подключился:
RelayResponse примерно за 650 мс, relay TCP за 320 мс, затем получены peer info и video.
Пароли не вводились агентом; аутентификационные диалоги не автоматизировались.

Реальный stdio MCP `megarustdesk`: list_sessions нашёл ровно целевую сессию,
bind_session привязал её к предоставленному пользователем ID. connected=true, mcpReady=true.
Пользователь вручную нажал «Разрешить Codex». screenshot вернул настоящий PNG удалённого
экрана 1920×1080; изображение проверено. Локальный screen capture для получения кадра не использовался.
Клики, ввод текста и изменения удалённых документов не выполнялись.
После закрытия проверочного MCP-клиента реальная сессия осталась connected=true,
режим стал Paused: отключение владельца корректно сняло разрешение на агентный ввод.
При cleanup контроллера могут отправляться служебные отпускания клавиш/кнопок.
Регистрация megarustdesk в установленном Codex CLI проверена; путь и --agent-mcp совпадают.
Это проверка реального MCP-транспорта и кадра, не model turn во внешнем Codex.

Обновлены .agent-build/connection-check.json, mcp-remote-check.json, mcp-after-check.json
и release/retention.json. Подготовлен отдельный release/Проверить соединение.cmd для журналов
принимающей стороны; он не понадобился после успешного повтора и в ZIP не добавлялся.
Проверен на локальной QA-копии и synthetic fixture: hashes/процессы читаются, пароль/token
строки исключаются, IP/UUID скрываются. Настройки и firewall не меняет, auth/config не читает.
ZIP и EXE 0.2.2 при этой проверке не изменялись.

Поставка unsigned. Полная приёмка UNVERIFIED, buildVerified=false.
Очистка старого generated staging была отклонена автоматической проверкой: blocked by policy.
Его не удаляют другим способом. Старые QA и данные сохраняются; полный cleanup не завершён.
Квитанция и пути сохраняемых комплектов — release/retention.json.

## Стабильность MCP: 0.2.3, 2026-10-25

При connected=true/Running реальные screenshot иногда возвращали EOF/усечённый JSON,
а также Windows 10053/10054. Временная same-session loopback-трасса подтвердила
7 неуспешных ответов из 20 при корректных запросах. Сам RDP-сеанс оставался подключён.
Точная причина Windows abort отдельно не доказана; старый транспорт отправлял JSON
множеством отдельных socket writes и скрывал ошибки отправки.

src/agent/transport.rs и mcp.rs теперь сериализуют JSON целиком до socket write,
включают TCP_NODELAY, различают пустой/усечённый/слишком большой ответ и фиксируют
ошибки отправки без содержимого кадра или capability. Только screenshot/session_status
имеют максимум три попытки на том же endpoint. Ввод и управление не повторяются.
Человеческие pause/stop/manual, ownership, epoch, свежесть кадра и binding сохранены.

Для уже открытого GUI 0.2.2 запущен tools/agent/live_transport.py: временный локальный
loopback-мост к той же сессии с тем же capability. Не расшифровывает credential,
не использует desktop/clipboard API, не меняет разрешения, не выбирает другую цель.
При выходе viewer завершается; при штатном завершении восстанавливает исходный endpoint,
если его не успел заменить viewer. Служба и автозапуск не устанавливались.
Неверные типы JSON, чужая сессия и чрезмерная вложенность отклоняются без падения listener.

Regression RED: пять framing/retry тестов не проходили до исправления; JSON-depth
integration воспроизвёл остановку helper и прошёл после guard. GREEN: transport 10/10,
gate 11/11, helper 6/6, включая truncated reply, запрет повтора ввода, invalid client
и восстановление исходного endpoint. Build: exit 0, 6м23с, 31 предупреждение.
Portable self-test/hashes прошли 2/2 (пробелы/кириллица, без dev PATH).
Source snapshot и обновлённые payload hashes совпали, метаданные data не изменились.

На реальном старом viewer через мост пройдены серии кадров 20/20 и 12/12; после
последнего guard — 10/10, четыре чтения восстановлены, ошибок в этом финальном прогоне нет.
Отдельные ответы на ввод старого viewer всё ещё обрывались и не повторялись автоматически;
результат проверялся новым screenshot. Длинный одноразовый type_text во вложенном RDP
исказился; URL удалось ввести короткими частями. Ссылка открыта, Google требует вход.
Данные таблицы не менялись. Это не полная проверка удалённого ввода.

0.2.3 подготовлена в release/RustDesk-Agent-Portable-win-x64 без закрытия старого viewer.
Активный mapped EXE 0.2.2 сохранён рядом как RustDesk-Agent.running-0.2.2.exe.
Для постоянного исправления нужен следующий запуск viewer и MCP-клиента с новым EXE;
в новой сессии человек снова разрешает Codex. Принимающий ПК обновлять для этого fix не нужно.
Постоянный transport нового GUI на реальной сессии ещё не проверен; чистая Windows,
полная portable-приёмка и сетевой startup 0.2.3 отдельно не выполнялись.

ZIP SHA256: afb51d0dbf0d678d9f9457cdacf0202d4cd107ad6380113a7f949ab3e4235087.
sourceBuildId: b26355f36342ee75a766c89e009bdc02489c2808c780e0bcd4bef68d5fc51c5b.
Текущий кандидат — 0.2.3; откат — release/rollback/mcp-0.2.2.zip (исходный SHA проверен).
buildVerified=false: полная приёмка и активация постоянного fix не завершены.
cleanupCompleted=false: автоматическая проверка отклонила exact-allowlist удаление
.agent-build/mcp-package, transport-tests.exe/.pdb, gate-tests.exe/.pdb и trace_transport.py
с причиной blocked by policy. Другим способом удаление не повторялось.
Удалённых generated целей нет; старые запрещённые остатки и активный старый EXE сохранены.
Свободное место до/после попытки cleanup: 47 447 576 576 / 46 900 064 256 байт;
эта разница включает фоновую работу и не является результатом очистки.
Подробности: .agent-build/transport-check.json, transport-cleanup-plan.json,
live-transport.json, package-check.json и release/retention.json.

## Повторное подключение, 2026-10-26

По обращению владельца проверен текущий распакованный кандидат 0.2.3: EXE и sciter.dll
совпали с manifest. При этом работали main/viewer из RustDesk-Agent.running-0.2.2.exe;
старый main при создании viewer запускает свой текущий EXE. Старые disconnected GUI
закрыты после проверки exact path/target; отдельные MCP-процессы других чатов не затронуты.
Запущены main и viewer из проверенного RustDesk-Agent.exe 0.2.3, цель прежняя, заданная владельцем.

На новом viewer воспроизведён relay connect около 127–134 мс после RelayResponse,
затем `deadline has elapsed` через 18 с в ожидании обмена ключами. То есть активация
0.2.3 подтверждена, но сама ошибка соединения пока не устранена. При следующих попытках
сервер сообщил target offline/not exist. Пользователю предложен полный перезапуск
принимающего приложения; результат этого шага ещё ожидается.
MCP показывает connected=false; агентный ввод не выполнялся. Код и сборки в этом прогоне
не менялись; новая сборка без журнала принимающей стороны не выпускалась.

## Исправление поставочного ZIP, 2026-10-26

После сообщения владельца о пакете около 150 МБ обнаружено несоответствие квитанции:
текущий ZIP имел 167 020 638 байт и SHA256
`0e8c85d5bbac1a0734aef775ceb93d906315ddd2503fd497d4afb0361cfec3e5`.
Его EXE/manifest действительно относились к 0.2.3 и совпадали по inventory hashes,
но кроме разрешённого payload в ZIP были runtime и локальная data запущенной папки.
Происхождение повторной упаковки не доказано; старый проверенный ZIP был заменён
после producer/QA, а не просто оказался предыдущей версией EXE.

Папка data в такой поставке могла перенести identity/настройки окна первого ПК на второй.
Секретное содержимое data не читалось. Исходный ZIP перемещён целиком в игнорируемую
`.agent-build/quarantine/portable-0.2.3-with-local-data.zip`; это частный snapshot,
не релиз/не rollback. Он не публикуется и не удаляется как generated cache.

Архив восстановлен из .agent-build/mcp-package: тот же проверенный EXE 0.2.3,
sourceBuildId `b26355f36342ee75a766c89e009bdc02489c2808c780e0bcd4bef68d5fc51c5b`.
Exact inventory 803 файла, включая manifest: data/runtime отсутствуют, каждый hash совпал.
Размер 17 607 331 байт; SHA256
`7e8b00db261b4c213cc59a5b0594c2589c83ac3b3d5ceb124bfe53a8eb0fa45c`.
Portable self-test после исправления прошёл с первого прогона 2/2; пользовательская data
и глобальные настройки Codex/PATH не изменились. EXE не перекомпилировывался.
Предыдущий проверенный package checkpoint 0.2.2 сохранён; его исходный SHA256 совпал.

Владелец подтвердил перезапуск принимающей стороны, но окна приложения не видит;
её актуальный ID не подтверждён. MCP сейчас не обнаруживает подключённый viewer.
Полная приёмка и обновление второго ПК не объявляются завершёнными.

Новый запрос: публичный URBoRTEX/MegaRustDesk и обновления по кнопке из Releases.
Спецификация — docs/superpowers/specs/2026-10-26-portable-updates-design.md;
независимая проверка не обнаружила незакрытых решений для планирования.
Спека согласована; актуальная реализация и ограничения описаны в разделе 0.3.0 выше.
