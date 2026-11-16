---
description: "Task list for FreeSpace native UI redesign 002"
---

# Задачи: FreeSpace — единый нативный интерфейс нового поколения

**Input**: Документы проектирования из `/specs/002-native-ui-redesign/`

**Prerequisites**: [plan.md](./plan.md) (обязательно), [spec.md](./spec.md) (обязательно), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: Тестовые задачи обязательны. Конституция, принцип V, и FR-115…FR-122 требуют
тест-первого подхода для критических модулей; UI проверяется Vitest/RTL, Playwright, axe,
визуальной регрессией и Tauri E2E.

**Organization**: Задачи сгруппированы по пользовательским историям. Старый egui-интерфейс
удаляется в Phase 1; второй UI, beta-флаг и side-by-side не создаются. До завершения 24
экранов приложение не релизится, старый UI не является fallback.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: может выполняться параллельно (разные файлы, нет зависимостей)
- **[Story]**: пользовательская история (US1…US11)
- В описании указан точный путь к файлу

## Path Conventions

- Workspace-крейты: `crates/<name>/src/`
- GUI Rust: `apps/freespace-desktop/src-tauri/src/`; в задачах `src-tauri/...` — сокращение этого пути
- GUI React: `apps/freespace-desktop/ui/src/`; в задачах `ui/...` — сокращение этого пути
- Тесты frontend: `apps/freespace-desktop/ui/tests/`
- Tauri E2E: `tests/tauri_e2e/`
- Контракты и фикстуры: `specs/002-native-ui-redesign/contracts/`
- Локализация: `locales/`
- Документация: `docs/`, `ru/`, `en/`

---

## Phase 1: Setup (общая инфраструктура)

**Purpose**: Зафиксировать ADR, заменить старый GUI новым скелетом, настроить сборку и тесты.

- [ ] T001 Проверить и утвердить ADR `docs/adr/0001-tauri-react-gui.md` (решение, альтернативы, Constitution Check, WebView2, безопасность, полная замена egui)
- [ ] T002 Обновить workspace в `Cargo.toml`: убрать `apps/freespace-gui`, добавить `apps/freespace-desktop` и `crates/freespace-app`, обновить `default-members`
- [ ] T003 [P] Перенести чистую геометрию Treemap и вспомогательные функции из `apps/freespace-gui/src/treemap/layout.rs` в `crates/freespace-app/src/treemap/layout.rs` с тестами
- [ ] T004 Удалить `apps/freespace-gui/` и неиспользуемые `eframe`/`egui`-зависимости из `Cargo.toml`; обновить `Cargo.lock`; зафиксировать, что история git — источник паритета
- [ ] T005 [P] Создать скелет `crates/freespace-app` (`Cargo.toml`, `src/lib.rs`, `src/error.rs`, `src/dto/mod.rs`, `src/treemap/mod.rs`)
- [ ] T006 [P] Создать скелет Tauri-приложения `apps/freespace-desktop/src-tauri/` (`Cargo.toml`, `build.rs`, `src/main.rs`, `src/lib.rs`, `tauri.conf.json`, `capabilities/default.json`)
- [ ] T007 [P] Создать React/Vite-скелет `apps/freespace-desktop/ui/` (`package.json`, `vite.config.ts`, `tsconfig.json`, `index.html`, `src/main.tsx`, `src/App.tsx`, `public/boot.js`)
- [ ] T008 Добавить frontend-зависимости и `package-lock.json`; настроить скрипты `dev`, `build`, `tauri`, `typecheck`, `lint`, `test:unit`, `test:e2e`, `test:visual`, `test:a11y`, `check:i18n`, `check:tokens`, `check:network`
- [ ] T009 Настроить строгий TypeScript и ESLint (`eslint`, `typescript-eslint`, `eslint-plugin-react-hooks`, `eslint-plugin-react-refresh`) в `apps/freespace-desktop/ui/eslint.config.js`
- [ ] T010 Настроить Vitest + React Testing Library + jsdom в `apps/freespace-desktop/ui/vitest.config.ts` и `tests/setup.ts`
- [ ] T011 Настроить Playwright в `apps/freespace-desktop/ui/playwright.config.ts` (webServer Vite, проекты chromium, screenshots, trace)
- [ ] T012 Настроить `@axe-core/playwright` и каталог визуальных эталонов `apps/freespace-desktop/ui/tests/visual/`
- [ ] T013 Создать генератор локализации `apps/freespace-desktop/ui/scripts/gen-i18n.mjs` (TOML → `src/i18n/generated/{ru,en,keys}.ts`) и проверку `check:i18n`
- [ ] T014 [P] Создать проверки `check:tokens` (запрет жёстких цветов) и `check:network` (network guard) в `apps/freespace-desktop/ui/scripts/`
- [ ] T015 Перенести иконки и splash из `apps/freespace-gui/assets/` в `apps/freespace-desktop/src-tauri/icons/` и `ui/public/`; обновить ссылки в `tauri.conf.json`
- [ ] T016 Настроить CI в `.github/workflows/ci.yml`: Rust (fmt, clippy, test), frontend (npm ci, typecheck, lint, unit), Tauri E2E, visual/a11y, кэши
- [ ] T017 Обновить `.gitignore` для `apps/freespace-desktop/ui/node_modules/`, `dist/`, `test-results/`, `playwright-report/`; не игнорировать сгенерированные i18n-каталоги
- [ ] T018 Настроить CSP, `withGlobalTauri: false`, минимальные capabilities и release-конфигурацию devtools в `apps/freespace-desktop/src-tauri/tauri.conf.json`
- [ ] T019 [P] Добавить базовые npm-зависимости для E2E (`tauri-driver`, `webdriverio`) и конфигурацию `tests/tauri_e2e/wdio.conf.ts`
- [ ] T020 [P] Создать каталоги контрактных фикстур `tests/contract/` и заготовку golden-проверок команд

---

## Phase 2: Foundational (блокирующие предпосылки)

**Purpose**: Прикладной слой, IPC, безопасность, настройки, UI-кит, тестовая инфраструктура.
**⚠️ CRITICAL**: Ни одна пользовательская история не начинается до завершения этой фазы.

- [ ] T021 Реализовать конверт ошибок и коды `E_*` в `crates/freespace-app/src/error.rs` с тестами
- [ ] T022 [P] Реализовать валидацию путей, enum, размеров и лимитов в `crates/freespace-app/src/validation.rs` с тестами
- [ ] T023 [P] Реализовать курсорную пагинацию и `generation` в `crates/freespace-app/src/paging.rs` с тестами
- [ ] T024 [P] Реализовать DTO страниц, узлов, тайлов, операций и журнала в `crates/freespace-app/src/dto/` с serde-тестами
- [ ] T025 Реализовать реестр задач, отмену и прогресс в `apps/freespace-desktop/src-tauri/src/state.rs` с тестами
- [ ] T026 [P] Реализовать события и throttle/sequence в `apps/freespace-desktop/src-tauri/src/events.rs` с тестами
- [ ] T027 [P] Реализовать канонизацию путей, allowlist команд и защиту области в `apps/freespace-desktop/src-tauri/src/security.rs` с тестами
- [ ] T028 Реализовать миграцию `[ui]` в `crates/freespace-app/src/settings.rs`: backup `settings.toml.bak`, версия схемы, идемпотентность, тесты
- [ ] T029 [P] Реализовать сохранение и валидацию состояния окна в `crates/freespace-app/src/window_state.rs` с тестами
- [ ] T030 Реализовать boot-инъекцию темы/языка/reduced motion в `apps/freespace-desktop/src-tauri/src/boot.rs` с тестами
- [ ] T031 Реализовать размещение WebView2 в `data/webview2` и offline-аргументы в `apps/freespace-desktop/src-tauri/src/main.rs`; зафиксировать результат spike в `research.md`
- [ ] T032 Реализовать `app_bootstrap`, `app_capabilities`, `app_settings_*`, `app_window_state_*` в `apps/freespace-desktop/src-tauri/src/commands/app.rs` с golden-тестами
- [ ] T033 Создать дизайн-токены и темы в `apps/freespace-desktop/ui/src/styles/{tokens,themes,global}.css`; контраст-тест в `tests/unit/tokens.test.ts`
- [ ] T034 [P] Реализовать UI-кит, часть 1: Button, IconButton, Field, SearchField, Select, Combobox, Toggle, Checkbox, Radio, Segmented, Tabs, Chip, Badge, Tooltip, Kbd в `ui/src/components/` с RTL-тестами
- [ ] T035 [P] Реализовать UI-кит, часть 2: Progress, Meter, Card, StatCard, SummaryCard, Dialog, ConfirmDialog, Toast, Banner, EmptyState, ErrorState, Loading/Skeleton в `ui/src/components/` с RTL-тестами
- [ ] T036 Реализовать AppShell, Sidebar, Header, StatusBar, Breadcrumbs, SplitPane в `ui/src/app/` с RTL-тестами
- [ ] T037 Реализовать транспорт `TauriTransport` и `MockTransport` + типизированный клиент в `ui/src/api/` с тестами
- [ ] T038 Реализовать Zustand stores shell/tasks/settings в `ui/src/stores/` с тестами
- [ ] T039 Реализовать реестр 24 экранов, навигацию, восстановление последнего экрана и глубокие ссылки в `ui/src/app/routing.ts` с тестами
- [ ] T040 Реализовать error boundary, нормализацию ошибок и локальное логирование в `ui/src/app/ErrorBoundary.tsx` с тестами
- [ ] T041 Реализовать i18n runtime, `MessageKey`, `Intl`-форматирование и missing-key тест в `ui/src/i18n/`
- [ ] T042 Реализовать `boot.js`, синхронизацию темы и тест отсутствия вспышки в `ui/public/boot.js` и `tests/unit/theme-boot.test.ts`
- [ ] T043 Реализовать палитру команд и реестр горячих клавиш в `ui/src/app/CommandPalette.tsx` с тестами
- [ ] T044 Создать тестовые фикстуры, render helpers и сценарии MockTransport в `ui/tests/helpers/`
- [ ] T045 [P] Создать page objects и фикстуры Playwright для 24 экранов в `ui/tests/e2e/pages/`
- [ ] T046 [P] Создать harness визуальной регрессии (viewports, темы, DPI, допуски) в `ui/tests/visual/harness.ts`
- [ ] T047 [P] Создать helper доступности (axe + клавиатура) в `ui/tests/a11y/axe.ts`
- [ ] T048 [P] Создать генератор синтетического дерева 10 млн узлов в `crates/freespace-app/examples/gen_fixture.rs` и harness замеров
- [ ] T049 Создать golden-фикстуры успеха/ошибок для всех команд в `tests/contract/fixtures/` и тест соответствия клиента `app_capabilities`

**Checkpoint**: фундамент готов — истории можно реализовывать параллельно.

---

## Phase 3: User Story 1 — Ядро: диски, сканирование, дерево, карта (P1) 🎯 MVP

**Goal**: дашборд томов, запуск/отмена сканирования, виртуализированное дерево, Canvas Treemap и панель деталей.

**Independent Test**: на тестовом VHD/временном каталоге просканировать известную структуру, сверить суммы, проверить прогресс, отмену, дерево, карту и отклик.

### Tests for User Story 1

- [ ] T050 [P] [US1] Golden-тесты `volumes_*`, `scan_*`, `tree_*`, `treemap_*` в `tests/contract/core_commands.rs`
- [ ] T051 [P] [US1] Vitest-тесты состояний и клавиатуры экранов «Диски», «Сканирование», «Дерево», «Карта» в `ui/tests/unit/screens/core/`
- [ ] T052 [P] [US1] Playwright E2E ядра на MockTransport в `ui/tests/e2e/core.spec.ts`

### Implementation for User Story 1

- [ ] T053 [US1] Реализовать сервис и команды томов в `crates/freespace-app/src/volumes.rs` и `apps/freespace-desktop/src-tauri/src/commands/volumes.rs`
- [ ] T054 [US1] Реализовать сервис и команды сканирования, прогресс и отмену в `crates/freespace-app/src/scan.rs` и `src-tauri/src/commands/scan.rs`
- [ ] T055 [US1] Реализовать постраничное дерево, поиск и предков в `crates/freespace-app/src/tree.rs` и `src-tauri/src/commands/tree.rs`
- [ ] T056 [US1] Реализовать агрегацию и раскладку Treemap в `crates/freespace-app/src/treemap/` с тестами на 10 млн узлов
- [ ] T057 [US1] Реализовать команду `treemap_tiles` и бинарный/компактный ответ в `src-tauri/src/commands/treemap.rs`
- [ ] T058 [US1] Реализовать экран «Диски» в `ui/src/screens/Dashboard.tsx` с карточками томов и быстрым анализом
- [ ] T059 [US1] Реализовать экран «Сканирование» в `ui/src/screens/Scan.tsx` с прогрессом и отменой
- [ ] T060 [US1] Реализовать общий `DataTable`/`TreeTable` с виртуализацией в `ui/src/components/DataTable.tsx`
- [ ] T061 [US1] Реализовать экран «Дерево» в `ui/src/screens/Tree.tsx` с сортировкой, фильтрами и синхронизацией выделения
- [ ] T062 [US1] Реализовать Canvas Treemap и hit-testing в `ui/src/treemap/` с тестами геометрии и клавиатуры
- [ ] T063 [US1] Реализовать экран «Карта» в `ui/src/screens/Treemap.tsx` с «хлебными крошками» и панелью деталей
- [ ] T064 [US1] Реализовать панель деталей и синхронизацию дерево↔карта в `ui/src/components/DetailsPanel.tsx`
- [ ] T065 [US1] Добавить визуальные эталоны четырёх экранов ядра в `ui/tests/visual/core/`
- [ ] T066 [US1] Измерить 200 мс / 30 fps / 4 ГБ на 10 млн узлов и записать отчёт в `docs/performance-002.md`

**Checkpoint**: US1 полностью работает и проверяется независимо; это MVP.

---

## Phase 4: User Story 2 — Анализ и очистка (P1)

**Goal**: крупнейшие файлы, типы, дубликаты, очистка и снимки на общем UI-ките.

**Independent Test**: на контрольном наборе проверить суммы, фильтры, правила дубликатов, рекомендации и сравнение снимков без разрушающих операций.

### Tests for User Story 2

- [ ] T067 [P] [US2] Rust-тесты `top_*`, `types_*`, `duplicates_*`, `cleanup_*`, `snapshots_*` в `tests/contract/analysis_commands.rs`
- [ ] T068 [P] [US2] Vitest-тесты пяти экранов и общих фильтров в `ui/tests/unit/screens/analysis/`
- [ ] T069 [P] [US2] Playwright E2E анализа на MockTransport в `ui/tests/e2e/analysis.spec.ts`

### Implementation for User Story 2

- [ ] T070 [US2] Реализовать сервисы и команды топ-файлов и типов в `crates/freespace-app/src/top_types.rs` и `src-tauri/src/commands/top_types.rs`
- [ ] T071 [US2] Реализовать сервис и команды дубликатов с прогрессом в `crates/freespace-app/src/duplicates.rs` и `src-tauri/src/commands/duplicates.rs`
- [ ] T072 [US2] Реализовать сервис и команды очистки и пустых каталогов в `crates/freespace-app/src/cleanup.rs` и `src-tauri/src/commands/cleanup.rs`
- [ ] T073 [US2] Реализовать сервис и команды снимков и сравнения в `crates/freespace-app/src/snapshots.rs` и `src-tauri/src/commands/snapshots.rs`
- [ ] T074 [US2] Реализовать общий `FilterBar` и сохранённые фильтры в `ui/src/components/FilterBar.tsx`
- [ ] T075 [US2] Реализовать экран «Крупнейшие файлы» в `ui/src/screens/TopFiles.tsx`
- [ ] T076 [US2] Реализовать экран «Типы» с диаграммой и легендой в `ui/src/screens/Types.tsx`
- [ ] T077 [US2] Реализовать экран «Дубликаты» с группами и выбором копий в `ui/src/screens/Duplicates.tsx`
- [ ] T078 [US2] Реализовать экран «Очистка» с карточками риска в `ui/src/screens/Cleanup.tsx`
- [ ] T079 [US2] Реализовать экран «Снимки» и таблицу diff в `ui/src/screens/Snapshots.tsx`
- [ ] T080 [US2] Добавить визуальные эталоны пяти экранов анализа в `ui/tests/visual/analysis/`

**Checkpoint**: US1 и US2 работают независимо; удаление копий ещё не подтверждается без US3.

---

## Phase 5: User Story 3 — Безопасные операции (P1)

**Goal**: единый предпросмотр, подтверждение, журнал, карантин и undo для всех разрушающих действий.

**Independent Test**: на временном каталоге выполнить удаление в Корзину, карантин и безвозвратно; проверить журнал, восстановление, отмену и отказ без прав.

### Tests for User Story 3

- [ ] T081 [P] [US3] Rust-тесты предпросмотра, токенов, подтверждения, undo/restore и конкурентности в `tests/contract/operations_commands.rs`
- [ ] T082 [P] [US3] Vitest-тесты `ConfirmDialog`, предпросмотра и прогресса в `ui/tests/unit/operations/`
- [ ] T083 [P] [US3] E2E на временном каталоге: recycle/quarantine/permanent/undo в `ui/tests/e2e/operations.spec.ts`

### Implementation for User Story 3

- [ ] T084 [US3] Реализовать сервис операций, preview-токены и TTL в `crates/freespace-app/src/operations.rs` с тестами
- [ ] T085 [US3] Реализовать команды `operation_*` и `journal_*` в `src-tauri/src/commands/operations.rs`
- [ ] T086 [US3] Реализовать `OperationPreviewDialog` с предупреждениями и выбором способа в `ui/src/components/OperationPreviewDialog.tsx`
- [ ] T087 [US3] Реализовать `OperationProgress` с отменой и undo в `ui/src/components/OperationProgress.tsx`
- [ ] T088 [US3] Интегрировать предпросмотр в «Топ», «Дубликаты» и «Очистку» в `ui/src/screens/`
- [ ] T089 [US3] Реализовать экран «Журнал» с фильтрами, undo и restore в `ui/src/screens/Journal.tsx`
- [ ] T090 [US3] Реализовать требования elevation и отказ без изменения данных в `ui/src/operations/elevation.ts` и `src-tauri/src/commands/app.rs`
- [ ] T091 [US3] Реализовать защиту системных путей и запрет выхода за область в `crates/freespace-app/src/operations.rs` с тестами
- [ ] T092 [US3] Реализовать блокировку конфликтующих операций на томе в `crates/freespace-app/src/operations.rs` с тестами
- [ ] T093 [US3] Добавить визуальные эталоны предпросмотра, прогресса и журнала в `ui/tests/visual/operations/`
- [ ] T094 [US3] Провести аудит: 100% разрушающих действий проходят preview/confirm/journal; отчёт в `docs/operations-audit-002.md`

**Checkpoint**: все очистки используют единый безопасный слой.

---

## Phase 6: User Story 4 — ФС и операции (P2)

**Goal**: журнал, детали размера, длинные пути, переименование и права.

**Independent Test**: на временной структуре с длинными путями и тестовыми ACL выполнить операции и проверить предпросмотр, журнал и ошибки.

### Tests for User Story 4

- [ ] T095 [P] [US4] Rust-тесты `size_details`, `long_paths`, `rename`, `permissions`, `attributes` в `tests/contract/filesystem_commands.rs`
- [ ] T096 [P] [US4] Vitest-тесты пяти экранов в `ui/tests/unit/screens/filesystem/`
- [ ] T097 [P] [US4] E2E длинных путей, переименования и прав на временном каталоге в `ui/tests/e2e/filesystem.spec.ts`

### Implementation for User Story 4

- [ ] T098 [US4] Реализовать сервис и команду `size_details` в `crates/freespace-app/src/size_details.rs` и `src-tauri/src/commands/filesystem.rs`
- [ ] T099 [US4] Реализовать страницу и валидацию длинных путей в `crates/freespace-app/src/long_paths.rs`
- [ ] T100 [US4] Реализовать предпросмотр и применение переименования в `crates/freespace-app/src/rename.rs`
- [ ] T101 [US4] Реализовать предпросмотр и применение ACL/атрибутов в `crates/freespace-app/src/permissions.rs`
- [ ] T102 [US4] Реализовать экран «Детали размера» в `ui/src/screens/SizeDetails.tsx`
- [ ] T103 [US4] Реализовать экран «Длинные пути» в `ui/src/screens/LongPaths.tsx`
- [ ] T104 [US4] Реализовать экран «Переименование» с diff старого/нового имени в `ui/src/screens/Rename.tsx`
- [ ] T105 [US4] Реализовать экран «Права» с предпросмотром ACL и атрибутов в `ui/src/screens/Permissions.tsx`
- [ ] T106 [US4] Расширить «Журнал» фильтрами по типу, пути и результату в `ui/src/screens/Journal.tsx`
- [ ] T107 [US4] Добавить визуальные эталоны пяти экранов ФС в `ui/tests/visual/filesystem/`

**Checkpoint**: US4 работает независимо; длинные пути и права не обходят `freespace-ops`.

---

## Phase 7: User Story 5 — Инспекция и отчёты (P2)

**Goal**: метаданные, владельцы, архивы, экспорт и разблокировка.

**Independent Test**: на наборе с метаданными, архивом, известными владельцами и удерживаемым файлом проверить чтение, экспорт и список блокировок без изменения файлов.

### Tests for User Story 5

- [ ] T108 [P] [US5] Rust-тесты `metadata`, `owners`, `archives`, `export`, `unlock` в `tests/contract/inspection_commands.rs`
- [ ] T109 [P] [US5] Vitest-тесты пяти экранов в `ui/tests/unit/screens/inspection/`
- [ ] T110 [P] [US5] E2E инспекции и экспорта на MockTransport в `ui/tests/e2e/inspection.spec.ts`

### Implementation for User Story 5

- [ ] T111 [US5] Реализовать сервис и команду метаданных в `crates/freespace-app/src/metadata.rs` и `src-tauri/src/commands/inspection.rs`
- [ ] T112 [US5] Реализовать страницы владельцев и их файлов в `crates/freespace-app/src/owners.rs`
- [ ] T113 [US5] Реализовать страницы архивов и экспорт списка без распаковки в `crates/freespace-app/src/archives.rs`
- [ ] T114 [US5] Реализовать предпросмотр, запуск и отмену экспорта в `crates/freespace-app/src/export.rs`
- [ ] T115 [US5] Реализовать список блокировок, мягкое/жёсткое снятие и reboot-очередь в `crates/freespace-app/src/unlock.rs`
- [ ] T116 [US5] Реализовать экран «Метаданные» в `ui/src/screens/Metadata.tsx`
- [ ] T117 [US5] Реализовать экран «Владельцы» в `ui/src/screens/Owners.tsx`
- [ ] T118 [US5] Реализовать экран «Архивы» в `ui/src/screens/Archives.tsx`
- [ ] T119 [US5] Реализовать экран «Экспорт» в `ui/src/screens/Export.tsx`
- [ ] T120 [US5] Реализовать экран «Разблокировка» в `ui/src/screens/Unlock.tsx`
- [ ] T121 [US5] Добавить визуальные эталоны пяти экранов инспекции в `ui/tests/visual/inspection/`

**Checkpoint**: инспекция не изменяет данные при просмотре; экспорт совпадает с UI.

---

## Phase 8: User Story 6 — Операции и настройки (P2)

**Goal**: шредер, мониторинг записи, elevation, настройки и «О программе».

**Independent Test**: на временных файлах проверить настройки, шредер, мониторинг, elevation и сведения о версии.

### Tests for User Story 6

- [ ] T122 [P] [US6] Rust-тесты `shred_*`, `monitor_*`, `settings_*`, `about_*`, `elevation_*` в `tests/contract/settings_commands.rs`
- [ ] T123 [P] [US6] Vitest-тесты пяти экранов в `ui/tests/unit/screens/settings/`
- [ ] T124 [P] [US6] E2E настроек, шредера (временный файл) и мониторинга в `ui/tests/e2e/settings.spec.ts`

### Implementation for User Story 6

- [ ] T125 [US6] Реализовать предпросмотр и запуск шредера в `crates/freespace-app/src/shred.rs` и `src-tauri/src/commands/shred.rs`
- [ ] T126 [US6] Реализовать мониторинг, события и исключения в `crates/freespace-app/src/monitor.rs` и `src-tauri/src/commands/monitor.rs`
- [ ] T127 [US6] Реализовать статус/запрос elevation и причину в `src-tauri/src/commands/app.rs`
- [ ] T128 [US6] Реализовать `settings_get/set/reset/validate_paths` в `crates/freespace-app/src/settings.rs` и `src-tauri/src/commands/settings.rs`
- [ ] T129 [US6] Реализовать `about_info` и `about_licenses` в `crates/freespace-app/src/about.rs`
- [ ] T130 [US6] Реализовать экран «Шредер» в `ui/src/screens/Shred.tsx`
- [ ] T131 [US6] Реализовать экран «Мониторинг записи» в `ui/src/screens/Monitor.tsx`
- [ ] T132 [US6] Реализовать экран «Elevation» в `ui/src/screens/Elevation.tsx`
- [ ] T133 [US6] Реализовать экран «Настройки» в `ui/src/screens/Settings.tsx`
- [ ] T134 [US6] Реализовать экран «О программе» в `ui/src/screens/About.tsx`
- [ ] T135 [US6] Добавить визуальные эталоны пяти экранов настроек в `ui/tests/visual/settings/`

**Checkpoint**: все 24 экрана реализованы; начинается полировка.

---

## Phase 9: User Story 7 — Темы, DPI и motion (P2)

**Goal**: тёмная/светлая/системная тема, DPI 100–200%, reduced motion без вспышки.

**Independent Test**: переключить темы на всех экранах, перезапустить, сменить системную тему, проверить DPI и reduced motion.

- [ ] T136 [P] [US7] Финальные токены тёмной/светлой темы и палитры Treemap в `ui/src/styles/tokens.css` с контраст-тестом
- [ ] T137 [US7] Синхронизация системной темы через `matchMedia` в `ui/src/app/theme.ts` с тестами
- [ ] T138 [US7] Проверить отсутствие вспышки на 24 экранах и зафиксировать тест в `ui/tests/e2e/theme.spec.ts`
- [ ] T139 [US7] Проверить 100/125/150/200% DPI на всех экранах; исправить обрезание и наложения в `ui/src/styles/`
- [ ] T140 [US7] Реализовать reduced motion и `prefers-contrast` в `ui/src/styles/global.css` с тестами
- [ ] T141 [US7] Добавить эталоны тем и DPI для критических экранов в `ui/tests/visual/themes/`
- [ ] T142 [US7] Проверить multi-monitor смену DPI и сохранение `WindowState` в `crates/freespace-app/src/window_state.rs`
- [ ] T143 [US7] Проверить палитры графиков/карты на различимость и контраст подписей в `ui/src/treemap/palette.ts`

**Checkpoint**: обе темы и DPI готовы к приёмке.

---

## Phase 10: User Story 8 — Доступность (P2)

**Goal**: клавиатура, фокус, ARIA, live-region и Narrator на всех экранах.

**Independent Test**: пройти десять ключевых сценариев без мыши и с Narrator.

- [ ] T144 [P] [US8] Реализовать focus management и возврат фокуса в диалогах в `ui/src/app/focus.ts` с тестами
- [ ] T145 [US8] Реализовать клавиатурную навигацию TreeGrid/DataTable/Treemap в `ui/src/components/` с тестами
- [ ] T146 [US8] Реализовать ARIA-роли, имена, состояния и `aria-live` в `ui/src/components/` и `ui/src/app/`
- [ ] T147 [US8] Добавить skip-link, landmarks и логический порядок Tab в `ui/src/app/AppShell.tsx`
- [ ] T148 [US8] Реализовать троттлинг объявлений прогресса и ошибок в `ui/src/app/live.ts`
- [ ] T149 [US8] Прогнать axe на 24 экранах и исправить критические нарушения в `ui/tests/a11y/screens.spec.ts`
- [ ] T150 [US8] Провести ручной аудит Narrator десяти сценариев и записать отчёт в `docs/accessibility-002.md`
- [ ] T151 [US8] Проверить, что статус не передаётся только цветом, на всех экранах и в Treemap в `ui/src/screens/` и `ui/src/treemap/`
- [ ] T152 [US8] Добавить клавиатурные E2E-сценарии в `ui/tests/e2e/keyboard.spec.ts`

**Checkpoint**: доступность подтверждена автоматически и вручную.

---

## Phase 11: User Story 9 — Локализация RU/EN (P3)

**Goal**: 100% строк из каталога, переключение без потери состояния, совпадение ключей.

**Independent Test**: переключить язык на всех экранах, проверить missing-key, числа/даты/единицы и отсутствие кириллицы в EN.

- [ ] T153 [P] [US9] Извлечь и добавить все ключи 24 экранов в `locales/ru.toml` и `locales/en.toml`
- [ ] T154 [US9] Проверить генерацию `ui/src/i18n/generated/` и отсутствие diff в CI
- [ ] T155 [US9] Реализовать плюрализацию, числа, даты и единицы через `Intl` в `ui/src/i18n/format.ts` с тестами
- [ ] T156 [US9] Реализовать переключение языка без потери состояния в `ui/src/stores/settings.ts` с тестами
- [ ] T157 [US9] Добавить тест отсутствия missing-key и кириллицы в EN в `ui/tests/unit/i18n.test.ts`
- [ ] T158 [US9] Добавить визуальные эталоны RU/EN для критических экранов в `ui/tests/visual/locales/`
- [ ] T159 [US9] Проверить локализацию ошибок, операций и уведомлений на всех экранах в `ui/src/i18n/` и `locales/`

**Checkpoint**: локализация полная; переключение не теряет состояние.

---

## Phase 12: User Story 10 — Визуальная регрессия и тесты (P3)

**Goal**: полная матрица скриншотов, проверки токенов/строк/сети, стабильный CI.

**Independent Test**: изменить токен/строку и убедиться, что проверка падает; вернуть и убедиться, что проходит.

- [ ] T160 [US10] Сгенерировать полную матрицу 24×2×4×2 эталонов в `ui/tests/visual/baselines/`
- [ ] T161 [US10] Настроить PR-набор (8 критических экранов + все 24 в тёмной теме 100% RU) в `playwright.config.ts`
- [ ] T162 [US10] Реализовать `check:tokens` и `check:i18n` как блокирующие проверки CI в `ui/scripts/` и `.github/workflows/ci.yml`
- [ ] T163 [US10] Реализовать `check:network` и CSP-проверку в E2E в `ui/scripts/check-network.mjs` и `tests/tauri_e2e/`
- [ ] T164 [US10] Настроить Tauri E2E (`tauri-driver` + WebdriverIO) в `.github/workflows/ci.yml` и `tests/tauri_e2e/`
- [ ] T165 [US10] Настроить стабильность визуальных тестов: шрифты, DPI, допуски, повторные прогоны в `ui/tests/visual/` и `playwright.config.ts`
- [ ] T166 [US10] Создать галерею скриншотов для ревью в `ui/tests/visual/gallery/` и ссылку из `docs/`
- [ ] T167 [US10] Добавить тест, что новый экран без эталона/теста не проходит приёмку в `ui/tests/visual/coverage.test.ts`
- [ ] T168 [US10] Запустить полный прогон тестов на Windows CI и устранить flaky-тесты в `.github/workflows/ci.yml` и `ui/tests/`

**Checkpoint**: визуальная и функциональная регрессия защищена.

---

## Phase 13: User Story 11 — Единая поставка и замена (P3)

**Goal**: один GUI, офлайн-установщик, portable, удаление `data` и меню, паритет CLI.

**Independent Test**: собрать установщик/portable на чистой Windows без сети, установить, запустить, удалить и проверить отсутствие следов вне `data`.

- [ ] T169 [US11] Убедиться, что `apps/freespace-gui` и egui-зависимости отсутствуют в workspace и поставке; проверить `Cargo.toml` и `Cargo.lock`
- [ ] T170 [US11] Настроить Tauri NSIS-установщик в `apps/freespace-desktop/src-tauri/tauri.conf.json` и хуки в `installer/nsis/`
- [ ] T171 [US11] Реализовать offline WebView2 (fixed runtime/offlineInstaller) и проверку версии в `src-tauri/`
- [ ] T172 [US11] Собрать portable ZIP с `data` и WebView2 в `installer/portable.ps1`
- [ ] T173 [US11] Перенести удаление `data` и `context-menu uninstall` в NSIS-хуки в `installer/nsis/`; удалить/вывести `installer/freespace.iss` из поставки
- [ ] T174 [US11] Сохранить имя `freespace-gui.exe`, ярлыки, иконку и splash в `installer/nsis/` и `apps/freespace-desktop/src-tauri/tauri.conf.json`; проверить установку/обновление
- [ ] T175 [US11] Проверить, что настройки, журнал, карантин, кеш и профиль WebView2 находятся только в `data`, в `apps/freespace-desktop/src-tauri/src/main.rs` и `tests/tauri_e2e/storage.spec.ts`
- [ ] T176 [US11] Проверить установку/запуск/удаление на чистой Windows без сети и записать отчёт в `docs/release-validation-002.md`
- [ ] T177 [US11] Прогнать golden-тесты CLI и заполнить матрицу паритета 24 экранов в `specs/002-native-ui-redesign/contracts/parity.md`
- [ ] T178 [US11] Проверить миграцию существующих настроек/журнала/карантина/снимков и откат по резервной копии в `crates/freespace-app/src/settings.rs` и `tests/contract/settings_commands.rs`
- [ ] T179 [US11] Обновить `README.md`, `docs/user-guide.md`, `docs/cli.md` и зеркала `ru/`/`en/` под новый UI
- [ ] T180 [US11] Обновить `CHANGELOG.md` и версию по Conventional Commits
- [ ] T181 [US11] Провести финальную приёмку 24/24, 0 сети, AA, DPI, RU/EN, один UI, CLI-паритет в `specs/002-native-ui-redesign/contracts/parity.md` и `docs/release-validation-002.md`

**Checkpoint**: релиз готов; старый UI удалён, второй UI отсутствует.

---

## Phase 14: Polish & Cross-Cutting Concerns

**Purpose**: производительность, качество кода, документация и финальные проверки.

- [ ] T182 [P] Оптимизировать старт: boot, first paint, first data screen до 3/5 с в `ui/src/app/` и `src-tauri/src/boot.rs`
- [ ] T183 [P] Оптимизировать память: освобождение страниц/тайлов при уходе с экрана в `ui/src/stores/` и `src-tauri/src/state.rs`
- [ ] T184 [P] Проверить и улучшить сообщения об ошибках и пустые состояния на всех 24 экранах в `ui/src/screens/` и `ui/src/components/`
- [ ] T185 [P] Провести аудит зависимостей: версии, лицензии, отсутствие лишних пакетов, обоснование в `research.md`
- [ ] T186 Удалить мёртвый код, временные флаги и неиспользуемые DTO в `crates/freespace-app/` и `ui/src/`
- [ ] T187 Обновить `docs/ui-redesign-plan.md` статусом «реализовано» и ссылками на 002
- [ ] T188 Прогнать `quickstart.md` от начала до конца на чистой машине и записать результаты
- [ ] T189 Выполнить `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` в `Cargo.toml` и `apps/freespace-desktop/src-tauri/Cargo.toml`
- [ ] T190 Выполнить `npm ci`, `npm run typecheck`, `npm run lint`, `npm run test:unit`, `npm run test:e2e`, `npm run test:visual`, `npm run test:a11y` в `apps/freespace-desktop/ui/package.json`
- [ ] T191 Проверить, что в `en/` нет кириллицы, ID (FR-, SC-, T-, CHK-) не изменены, зеркала 002 обновлены в `en/002-native-ui-redesign/`
- [ ] T192 Подготовить финальный отчёт: производительность, доступность, офлайн, визуальная регрессия, паритет в `docs/release-validation-002.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: без зависимостей; удаление egui выполняется здесь.
- **Phase 2 (Foundational)**: зависит от Phase 1; блокирует все истории.
- **Phase 3 (US1)**: после Phase 2; MVP.
- **Phase 4 (US2)**: после Phase 2; использует общий UI-кит и фильтры. Разрушающие действия
  требуют Phase 5.
- **Phase 5 (US3)**: после US1; блокирует разрушающие действия в US2 и US4–US6.
- **Phase 6 (US4)**: после US3; использует журнал и предпросмотр.
- **Phase 7 (US5)**: после Phase 2; может идти параллельно US4.
- **Phase 8 (US6)**: после Phase 2; может идти параллельно US4/US5.
- **Phase 9 (US7)**: после US1 и UI-кита; полировка всех экранов.
- **Phase 10 (US8)**: после US1; лучше параллельно с экранами, но финальный аудит — после US6.
- **Phase 11 (US9)**: после появления экранов; финальная проверка — после US6.
- **Phase 12 (US10)**: после US1–US9; финальная матрица.
- **Phase 13 (US11)**: после 24 экранов и US10; финальная замена/поставка.
- **Phase 14 (Polish)**: после всех историй.

### User Story Dependencies

- **US1 (P1)**: база; не зависит от других историй.
- **US2 (P1)**: зависит от US1 (данные) и US3 (подтверждение разрушающих действий).
- **US3 (P1)**: зависит от US1; обязательна для US2, US4, US5 (unlock), US6 (shred).
- **US4 (P2)**: зависит от US1 и US3.
- **US5 (P2)**: зависит от US1; unlock использует US3.
- **US6 (P2)**: зависит от US1; shred использует US3.
- **US7 (P2)**: зависит от US1 и UI-кита.
- **US8 (P2)**: зависит от US1; финальный аудит после US6.
- **US9 (P3)**: зависит от экранов US1–US6.
- **US10 (P3)**: зависит от US1–US9.
- **US11 (P3)**: зависит от 24 экранов, US10 и CLI-паритета.

### Within Each User Story

- Тесты пишутся и падают до реализации (конституция, принцип V).
- DTO/сервисы — до команд; команды — до экранов.
- Общие компоненты — до специализированных экранов.
- Визуальные эталоны — после стабилизации экрана.
- E2E — после появления реального/мок-транспорта.

### Parallel Opportunities

- Phase 1: T003, T005, T006, T007, T014, T019, T020.
- Phase 2: T022–T024, T026–T027, T029, T034–T035, T045–T048.
- US1: T050–T052 (тесты), затем T053–T064 по файлам; T065 после экранов.
- US2: T067–T069 (тесты), затем T070–T079; T080 после экранов.
- US3: T081–T083 (тесты), затем T084–T092; T093–T094 после.
- US4/US5/US6: тесты и backend-сервисы можно вести параллельно по разным файлам.
- US7/US8/US9: аудиты и правки можно вести параллельно после появления экранов.
- US10: генерация эталонов и CI-настройка — параллельно, но финальный прогон последовательный.
- US11: installer/portable/offline проверки — параллельно, финальная приёмка последовательная.

---

## Parallel Example: User Story 1

```text
# Тесты (разные файлы):
T050 Golden-тесты команд ядра
T051 Vitest-тесты экранов ядра
T052 Playwright E2E ядра

# Реализация (разные файлы):
T053 volumes service/commands
T054 scan service/commands
T055 tree service/commands
T056 treemap layout
T057 treemap command
T058 Dashboard
T059 Scan
T060 DataTable
T061 Tree
T062 Canvas Treemap
T063 Treemap screen
T064 DetailsPanel
```

## Parallel Example: User Story 3

```text
T081 Rust-тесты операций
T082 Vitest-тесты диалогов
T083 E2E временного каталога
T084 operations service
T085 operation/journal commands
T086 OperationPreviewDialog
T087 OperationProgress
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1: ADR, удаление egui, скелет Tauri/React, CI.
2. Phase 2: прикладной слой, IPC, безопасность, настройки, UI-кит, тестовая база.
3. Phase 3: US1 — диски, сканирование, дерево, карта.
4. **STOP and VALIDATE**: quickstart, сценарий 1, 200 мс/30 fps/4 ГБ.
5. Демонстрация MVP; релиз ещё не выполняется, старый UI удалён.

### Incremental Delivery

1. US1 → MVP.
2. US3 → безопасные операции (обязательно до очистки).
3. US2 → анализ и очистка.
4. US4 → ФС и операции.
5. US5 → инспекция и отчёты.
6. US6 → операции и настройки.
7. US7–US9 → темы, доступность, локализация.
8. US10 → полная матрица тестов.
9. US11 → установщик, офлайн, удаление egui, приёмка.
10. Polish → финальные проверки.

### Parallel Team Strategy

- Rust-инженер: `freespace-app`, команды, тесты, производительность.
- Frontend-инженер: UI-кит, shell, экраны, доступность.
- Дизайнер/QA: токены, скриншоты, axe/Narrator, быстрый старт.
- После Phase 2 команды могут работать параллельно по историям, но US3 имеет приоритет.

---

## Notes

- [P] — разные файлы, нет зависимостей.
- [Story] — трассировка к spec.md.
- Старый egui удаляется в Phase 1; его исходники доступны в истории git, но не собираются.
- Все разрушающие сценарии — только на временных каталогах/VHD; admin-тесты за
  `FREESPACE_ADMIN_TESTS=1`.
- 0 сетевых вызовов, 0 хардкод-цветов, 0 строк вне локализации — блокирующие проверки.
- Коммиты — Conventional Commits; одна задача или логическая группа на коммит.
- Отмечать `[x]` только после выполнения критериев готовности задачи и зелёных проверок.
