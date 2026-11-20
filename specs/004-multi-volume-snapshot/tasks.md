# Tasks: Мультитомный снимок

**Feature**: `004-multi-volume-snapshot` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

**Правила**: тесты критических модулей пишутся до реализации; `[x]` ставится только после
зелёных `cargo fmt`, `cargo clippy -D warnings`, `cargo test` и обновления документации.

## Phase 1 — Подготовка

- [x] **T001** Зафиксировать базовую линию: `cargo test --workspace`, `cargo clippy --workspace
  --all-targets -- -D warnings`, UI unit-тесты. Записать известные падения в
  `specs/004-multi-volume-snapshot/quickstart.md` (раздел «Регрессии»).
- [x] **T002** Обновить `.specify/feature.json` на `specs/004-multi-volume-snapshot` (локальный
  файл, не коммитится).

## Phase 2 — Ядро сканирования (`freespace-scan`)

- [x] **T010** Тест: `scan_many` по двум временным каталогам создаёт одну `FsTree` с двумя
  корнями, суммы `nodes_count` и `tree.roots().count()` совпадают. Файл:
  `crates/freespace-scan/src/session.rs`.
- [x] **T011** Тест: ошибка одного корня не мешает второму; `failures` содержит путь и ошибку,
  `roots` содержит успешный корень.
- [x] **T012** Тест: отмена после первого корня не запускает второй; `cancelled = true`.
- [x] **T013** Тест: общий прогресс монотонен и заканчивается `fraction = 1.0`; наблюдатель
  получает `Finished`.
- [x] **T014** Реализовать `RootScanOutcome`, `ScanFailure`, `ScanManyResult` и
  `scan_into(request, tree, observer)`; вынести из `scan()` логику одного корня без
  `recompute_totals`/`compact`/`validate`.
- [x] **T015** Реализовать `scan_many(requests, observer)`: одна арена, последовательные корни,
  изоляция ошибок, общая агрегация, один `recompute_totals`/`validate`/`compact`.
- [x] **T016** Реализовать обёртку наблюдателя для общего прогресса `(index + local) / N` и
  финального события.
- [x] **T017** Сохранить `scan()` как обёртку над `scan_into`; убедиться, что существующие тесты
  `scan_walk`, `scan_mft`, `cleanup`, `duplicates` не изменились.
- [x] **T018** Экспортировать новые типы из `crates/freespace-scan/src/lib.rs`.

**Зависимости**: T010–T013 до T014–T016; T017 после T014.

## Phase 3 — Модель приложения (`freespace-app`)

- [x] **T020** Тест: `validate_scan_request` принимает 1..=32 путей, отклоняет пустой список,
  повторы, пересечения и относительные пути. Файл: `crates/freespace-app/src/validation.rs`.
- [x] **T021** DTO: заменить `ScanRequest.path` на `paths: Vec<String>`; добавить
  `Task.volume_ids`; обновить `MAX_SCAN_ROOTS`. Файл: `crates/freespace-app/src/dto/mod.rs`.
- [x] **T022** Реализовать валидацию корней: нормализация ключей, запрет повторов и пересечений,
  лимит 32. Файл: `crates/freespace-app/src/validation.rs`.
- [x] **T023** Переписать `run_scan` на `scan_many`: собрать запросы по путям, обогатить ADS один
  раз, вернуть `ScanOutcome` с `roots`, `failures`, `warnings`, `issues`, `cancelled`.
  Файл: `crates/freespace-app/src/scan.rs`.
- [x] **T024** Тест: `TreeSnapshot::new_multi` хранит все тома и сессии; `volume()`/`session()`
  возвращают первые; `volume_for_path` выбирает самый длинный mount point. Файл:
  `crates/freespace-app/src/tree.rs`.
- [x] **T025** Реализовать `TreeSnapshot.volumes/sessions`, `new_multi`, `volumes()`, `sessions()`,
  `volume_for_path`; `new` оставить совместимым.
- [x] **T026** Тест: отчёт/снимок на мультитомном снимке использует первый том и все корни (без
  изменения формата). Файл: `crates/freespace-app/src/export.rs` или `snapshots.rs`.
- [x] **T027** Обновить `operations`/`duplicates`, чтобы том журнала и задачи брался из пути цели
  через `volume_for_path` (fallback — первый том).

**Зависимости**: T021 до T022–T023; T025 до T026–T027.

## Phase 4 — Tauri и IPC

- [x] **T030** Тест: `reserve_scans` атомарно занимает все тома, при занятом любом возвращает
  `E_BUSY` и не оставляет частичных броней; `release_scans` освобождает все. Файл:
  `apps/freespace-desktop/src-tauri/src/state.rs`.
- [x] **T031** Реализовать `reserve_scans`/`release_scans` и `Task.volume_ids`; обновить
  `reserve_scan` для одного тома.
- [x] **T032** Обновить `scan_start`: валидация всех путей, канонизация, определение томов,
  атомарное резервирование, запуск воркера. Файл:
  `apps/freespace-desktop/src-tauri/src/commands/scan.rs`.
- [x] **T033** Обновить воркер: `run_scan` → снимок с `volumes`/`sessions`; `partial` при ошибке
  корня; `failed` при нуле успешных; сводка с `volumes`/`methods`; освобождение всех броней.
- [x] **T034** Добавить команду `snapshot_volumes(generation)` в
  `commands/volumes.rs`, `security.rs`, `lib.rs`; покрыть тестом capabilities.
- [x] **T035** Обновить `events.rs`: `ScanSummary.methods/volumes`; `Task.volume_ids`.
- [x] **T036** Обновить `commands/analysis.rs` и `commands/operations.rs`: том задачи/журнала из
  пути цели. Тест: операция на втором томе пишет второй `volume_id`.
- [x] **T037** Проверить `commands/filesystem.rs` и `inspection`: пути по умолчанию для
  мультитомного снимка; при необходимости использовать первый корень и явно документировать.
- [x] **T038** Повысить `IPC_VERSION` до 2 и обновить capability-тесты.

**Зависимости**: T031 до T032–T033; T034 после T033.

## Phase 5 — UI

- [x] **T040** Обновить `ui/src/api/dto.ts`: `ScanRequest.paths`, `Task.volume_ids`.
- [x] **T041** Добавить `api.snapshotVolumes(generation)` в `ui/src/api/client.ts`,
  `transport.ts`, `mockTransport.ts`.
- [x] **T042** `snapshotStore`: `volumes: VolumeSummary[]`, `setVolumes`, производный `volume`;
  сброс при новой генерации.
- [x] **T043** `App.tsx`: после `scan-completed` запрашивать `snapshotVolumes(event.generation)` и
  обновлять стор.
- [x] **T044** `Overview.tsx`: выбор тома нажатием на карточку, кнопка «Анализировать», индикатор
  сканирования по `volume_ids`, селектор типа сканирования рядом с кнопкой.
- [x] **T045** `Tree.tsx`: сумма размеров всех корней, контекст с числом томов, корректный
  fallback при пустом снимке.
- [x] **T046** Обновить `Duplicates.tsx`, `Owners.tsx`, `Snapshots.tsx`, `ExportAction.tsx`:
  использовать первый том только как путь по умолчанию, данные — из всего снимка.
- [x] **T047** i18n: добавить ключи в `locales/ru.toml` и `locales/en.toml`, перегенерировать
  `ui/src/i18n/generated/*` штатным скриптом.
- [x] **T048** Unit-тесты: стор, обзор (выбор и запуск), дерево (два корня, сумма), mock transport.
- [x] **T049** e2e/a11y: сценарий двух томов на mock-транспорте; проверить клавиатуру и фокус
  карточек томов.

**Зависимости**: T040–T043 до T044–T046; T047 до T048–T049.

## Phase 6 — CLI

- [x] **T050** Тест: `scan` с двумя путями возвращает `volumes` из двух элементов, `roots` из
  двух и суммарный `total`; `tree` печатает два корня. Файлы: `tests/cli_contract.rs`,
  `crates/freespace-cli/src/commands/scan.rs`.
- [x] **T051** `cli.rs`: `Scan.paths` и `Tree.paths` вместо `path`; обновить `main.rs`.
- [x] **T052** `commands/scan.rs`: `ScanArgs.paths`, `scan_many`, JSON `volumes`/`methods`,
  суммарные счётчики, предупреждение для `--snapshot-out` при нескольких корнях.
- [x] **T053** Сохранить одиночный путь: существующие тесты и поля не ломаются.

**Зависимости**: T051–T052 после T023.

## Phase 7 — Контракты и фикстуры

- [x] **T060** Обновить `tests/contract/*` и `tests/contract/fixtures/*` под `paths`,
  `volume_ids`, `snapshot_volumes`, `methods`/`volumes`; перегенерировать фикстуры штатным
  скриптом.
- [x] **T061** Прогнать контрактные тесты ядра, анализа, инспекции и файловых операций.
- [x] **T062** Обновить `tests/tauri_e2e` только там, где используется `scan_start`/`Task`.

**Зависимости**: T038 до T060.

## Phase 8 — Документация

- [x] **T070** Создать английское зеркало `docs/en/004-multi-volume-snapshot/` (spec, plan,
  research, data-model, quickstart, contracts, tasks) без кириллицы; ID не менять.
- [x] **T071** Обновить `docs/ru/user-guide.md` и `docs/en/user-guide.md`: мультитомное
  сканирование, состав снимка, ограничения экспорта.
- [x] **T072** Обновить `CHANGELOG.md` и `docs/ru/repository-guide.md`/`docs/en/repository-guide.md`
  при необходимости.
- [x] **T073** Отметить выполненные задачи `[x]` в `tasks.md`; не трогать `checklists/quality.md`.

## Phase 9 — Проверка

- [x] **T080** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace` — зелёные.
- [x] **T081** UI: `npm test`, `npm run lint`, e2e/a11y — зелёные.
- [ ] **T082** Ручной прогон quickstart на двух тестовых томах (GUI + CLI).
- [ ] **T083** Проверить бюджеты: два тома 8 ТБ ≤ 120 с, 10 млн узлов ≤ 4 ГБ, корневая страница
  ≤ 200 мс.
- [ ] **T084** Коммит по Conventional Commits (`feat: scan several volumes into one snapshot`).
