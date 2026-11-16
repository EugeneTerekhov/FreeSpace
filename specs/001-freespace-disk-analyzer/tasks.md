# Задачи: FreeSpace — анализатор и менеджер дискового пространства Windows

**Input**: Документы проектирования из `/specs/001-freespace-disk-analyzer/`

**Prerequisites**: plan.md (обязательно), spec.md (обязательно), research.md, data-model.md, contracts/, quickstart.md

**Tests**: Тестовые задачи включены для критических модулей согласно Конституции, принцип V
(тест-первый подход к операциям с файловой системой). Для чисто UI-задач отдельные тесты не
предусмотрены.

**Organization**: Задачи сгруппированы по пользовательским историям, чтобы каждую можно было
реализовать и проверить независимо.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: может выполняться параллельно (разные файлы, нет зависимостей)
- **[Story]**: пользовательская история (US1…US23)
- В описании указан точный путь к файлу

## Path Conventions

- Workspace-крейты: `crates/<name>/src/`
- GUI-приложение: `apps/freespace-gui/src/`
- Межкрейтовые тесты: `tests/`
- Бенчмарки: `benches/`
- Локализация: `locales/`

---

## Phase 1: Setup (общая инфраструктура)

**Purpose**: Инициализация workspace, инструментов и скелетов крейтов

- [x] T001 Создать Cargo workspace в `Cargo.toml` и зафиксировать тулчейн в `rust-toolchain.toml` (Rust 1.85+, edition 2024)
- [x] T002 [P] Настроить линты workspace, профили сборки и `.gitignore` в `Cargo.toml` и `.gitignore`
- [x] T003 [P] Создать скелеты крейтов `crates/freespace-core`, `crates/freespace-platform`, `crates/freespace-scan`, `crates/freespace-ops`, `crates/freespace-analysis`, `crates/freespace-report`, `crates/freespace-cli` (Cargo.toml + src/lib.rs)
- [x] T004 [P] Создать скелет GUI-приложения `apps/freespace-gui` (eframe, Cargo.toml + src/main.rs)
- [x] T005 [P] Настроить CI GitHub Actions для `fmt`, `clippy -D warnings`, `test` на windows-latest в `.github/workflows/ci.yml`
- [x] T006 [P] Создать файлы локализации `locales/ru.toml` и `locales/en.toml` и подключить `rust-i18n`
- [x] T007 [P] Настроить журналирование `tracing` и базовые типы ошибок в `crates/freespace-core/src/error.rs`
- [x] T008 [P] Создать каталоги `ru/`, `en/`, `docs/` и базовый `README.md` с командами сборки

---

## Phase 2: Foundational (блокирующие предпосылки)

**Purpose**: Общая модель данных, платформенный слой и каркасы, без которых не может начаться ни одна история

**⚠️ CRITICAL**: Ни одна пользовательская история не начинается до завершения этой фазы

- [x] T009 Реализовать `SizeMetrics` (логический/на диске/физический/ADS/экономия) в `crates/freespace-core/src/size.rs`
- [x] T010 Реализовать арену `FsNode` (SoA, `NodeId = u32`, имена в UTF-8-арене) в `crates/freespace-core/src/tree.rs`
- [x] T011 Реализовать `Volume`, `ScanSession` и перечисления статусов в `crates/freespace-core/src/model.rs`
- [x] T012 [P] Реализовать `Settings` (serde + TOML) и хранение в портативном режиме в `crates/freespace-core/src/settings.rs`
- [x] T013 [P] Реализовать слой путей: verbatim-пути, нормализация, предел 32 000 символов в `crates/freespace-platform/src/paths.rs`
- [x] T014 [P] Реализовать обёртки WinAPI (handle RAII, преобразование ошибок) в `crates/freespace-platform/src/win.rs`
- [x] T015 [P] Реализовать перечисление томов (`GetLogicalDrives`, `GetVolumeInformationW`) в `crates/freespace-platform/src/volumes.rs`
- [x] T016 Реализовать схему SQLite и миграции (журнал, карантин, кеш хешей) в `crates/freespace-ops/src/store.rs`
- [x] T017 Реализовать модель `Operation`/`JournalEntry` и машину состояний в `crates/freespace-ops/src/operation.rs`
- [x] T018 [P] Реализовать движок предпросмотра и подтверждения операций в `crates/freespace-ops/src/confirm.rs`
- [x] T019 [P] Реализовать карантин (перемещение, индекс, восстановление) в `crates/freespace-ops/src/quarantine.rs`
- [x] T020 [P] Реализовать каркас CLI: clap, глобальные флаги, JSON-конверт, коды возврата в `crates/freespace-cli/src/main.rs`
- [x] T021 [P] Реализовать каркас GUI: окно, навигация, фоновые задачи и канал прогресса в `apps/freespace-gui/src/app.rs`
- [x] T022 [P] Создать тестовые фикстуры ФС и утилиты в `tests/common/mod.rs`
- [x] T023 [P] Настроить бенчмарки criterion (сканирование, хеширование, Treemap) в `benches/`
- [x] T024 Реализовать загрузку локалей и переключение языка в рантайме в `crates/freespace-core/src/i18n.rs`
- [x] T025 Определить доменные коды ошибок `E_*` и их соответствие кодам возврата в `crates/freespace-core/src/error.rs`

**Checkpoint**: фундамент готов — истории можно реализовывать параллельно

---

## Phase 3: User Story 1 — Быстрое сканирование тома и дерево размеров (P1) 🎯 MVP

**Goal**: анализ тома через MFT с fallback-обходом, построение дерева, поиск, сортировка и прогресс

**Independent Test**: просканировать тестовый том, сверить суммы с эталоном, проверить поиск/сортировку и пометку недоступных каталогов

- [x] T026 [P] [US1] Тест MFT-перечисления и сумм на эталонном томе (admin-gated) в `tests/scan_mft.rs`
- [x] T027 [P] [US1] Тест fallback-обхода и пометки недоступных каталогов в `tests/scan_walk.rs`
- [x] T028 [US1] Реализовать открытие тома и `FSCTL_ENUM_USN_DATA` в `crates/freespace-platform/src/mft.rs`
- [x] T029 [US1] Реализовать разбор `USN_RECORD_V2` и построение дерева в `crates/freespace-scan/src/mft_scan.rs`
- [x] T030 [US1] Реализовать инкрементальное обновление по USN (`JournalID`, `NextUSN`) в `crates/freespace-scan/src/usn.rs`
- [x] T031 [US1] Реализовать fallback-обход (`jwalk` + `rayon`) и определение метода в `crates/freespace-scan/src/walk.rs`
- [x] T032 [US1] Реализовать потоковую выдачу результатов, прогресс и учёт исключённых путей в `crates/freespace-scan/src/session.rs`
- [x] T033 [US1] Реализовать поиск/фильтры (`globset`) и сортировку в `crates/freespace-core/src/query.rs`
- [x] T034 [US1] Реализовать команды CLI `scan` и `tree` с JSON в `crates/freespace-cli/src/commands/scan.rs`
- [x] T035 [US1] Реализовать виртуализированное дерево в GUI в `apps/freespace-gui/src/views/tree.rs`
- [x] T036 [US1] Интегрировать прогресс и отмену сканирования в GUI в `apps/freespace-gui/src/views/scan.rs`

**Checkpoint**: US1 полностью функциональна и может быть продемонстрирована как MVP

---

## Phase 4: User Story 2 — Интерактивная карта Treemap (P1)

**Goal**: визуальная карта с пропорциональными площадями, зумом, подсказками и раскраской по типам

**Independent Test**: проверить пропорциональность площадей, зум, подсказки и легенду на наборе с известными размерами

- [x] T037 [P] [US2] Тест раскладки squarified и пропорциональности площадей в `apps/freespace-gui/src/treemap/layout.rs`
- [x] T038 [US2] Реализовать алгоритм squarified treemap в `apps/freespace-gui/src/treemap/layout.rs`
- [x] T039 [US2] Реализовать отрисовку с уровнями детализации и кэшем раскладки в `apps/freespace-gui/src/treemap/view.rs`
- [x] T040 [US2] Реализовать зум, «хлебные крошки» и подсказки в `apps/freespace-gui/src/treemap/interaction.rs`
- [x] T041 [US2] Реализовать раскраску по типам файлов и легенду в `apps/freespace-gui/src/treemap/colors.rs`
- [x] T042 [P] [US2] Бенчмарк отрисовки на 1 млн узлов в `benches/treemap.rs`

**Checkpoint**: US2 работает на данных US1 независимо от остальных историй

---

## Phase 5: User Story 3 — Разблокировка занятых файлов (P1)

**Goal**: определение блокирующих процессов, мягкое/жёсткое снятие блокировки, удаление при перезагрузке

**Independent Test**: удержать файл тестовым процессом, найти блокировку, снять её и проверить доступность файла

- [x] T043 [P] [US3] Тест списка блокировок через Restart Manager в `tests/unlock.rs`
- [x] T044 [US3] Реализовать `RmStartSession`/`RmGetList` в `crates/freespace-platform/src/locks.rs`
- [x] T045 [US3] Реализовать мягкое/жёсткое снятие и удаление при перезагрузке в `crates/freespace-ops/src/unlock.rs`
- [x] T046 [US3] Реализовать CLI `unlock` (list/soft/force) в `crates/freespace-cli/src/commands/unlock.rs`
- [x] T047 [US3] Реализовать диалог блокировок в GUI в `apps/freespace-gui/src/views/unlock.rs`

---

## Phase 6: User Story 4 — Топ файлов и группировка по типам (P1)

**Goal**: плоский топ-N файлов и агрегация занятого места по расширениям/категориям

**Independent Test**: сверить топ-список при разных N и суммы по расширениям с эталоном

- [x] T048 [P] [US4] Тест агрегации топ-N и типов в `tests/top_types.rs`
- [x] T049 [US4] Реализовать топ-N (частичная сортировка) в `crates/freespace-analysis/src/top.rs`
- [x] T050 [US4] Реализовать группировку по расширениям и категориям в `crates/freespace-analysis/src/types.rs`
- [x] T051 [US4] Реализовать CLI `top` и `types` в `crates/freespace-cli/src/commands/top.rs`
- [x] T052 [US4] Реализовать вкладки «Топ» и «Типы» в GUI в `apps/freespace-gui/src/views/top_types.rs`

**Checkpoint**: MVP (US1–US4) готов к демонстрации

---

## Phase 7: User Story 5 — Поиск дубликатов (P2)

**Goal**: группы полных совпадений, правила выбора копии и безопасное удаление

**Independent Test**: набор с известными дубликатами и «ложными» файлами одинакового размера — проверить отсутствие ложных совпадений и сохранность копии

- [x] T053 [P] [US5] Тест конвейера хеширования и отсутствия ложных совпадений в `tests/duplicates.rs`
- [x] T054 [US5] Реализовать двухэтапное хеширование и побайтовую проверку в `crates/freespace-analysis/src/hash.rs`
- [x] T055 [US5] Реализовать кеш хешей в SQLite в `crates/freespace-analysis/src/hash_cache.rs`
- [x] T056 [US5] Реализовать группировку дубликатов и правила keep в `crates/freespace-analysis/src/duplicates.rs`
- [x] T057 [US5] Реализовать CLI `duplicates` в `crates/freespace-cli/src/commands/duplicates.rs`
- [x] T058 [US5] Реализовать экран дубликатов в GUI в `apps/freespace-gui/src/views/duplicates.rs`

---

## Phase 8: User Story 6 — Шредер и удаление при перезагрузке (P2)

**Goal**: безвозвратная перезапись, предупреждения SSD/TRIM и системная очередь удаления

**Independent Test**: уничтожить тестовый набор и убедиться в невозможности восстановления независимым инструментом

- [x] T059 [P] [US6] Тест перезаписи/удаления и предупреждений SSD в `tests/shred.rs`
- [x] T060 [US6] Реализовать шредер (проходы, паттерны, write-through, rename/truncate/delete) в `crates/freespace-ops/src/shred.rs`
- [x] T061 [US6] Реализовать определение SSD и TRIM в `crates/freespace-platform/src/trim.rs`
- [x] T062 [US6] Реализовать CLI `shred` и проверку `--yes` в `crates/freespace-cli/src/commands/shred.rs`
- [x] T063 [US6] Реализовать UI шредера с предупреждением и подтверждением в `apps/freespace-gui/src/views/shred.rs`

---

## Phase 9: User Story 7 — Устаревшие файлы, пустые папки и рекомендации (P2)

**Goal**: фильтры по датам, категории временных данных, пустые каталоги и рекомендации по очистке

**Independent Test**: набор с известными датами и пустыми каталогами — проверить точность фильтров и полноту списка

- [x] T064 [P] [US7] Тест фильтров дат и поиска пустых каталогов в `tests/cleanup.rs`
- [x] T065 [US7] Реализовать фильтр устаревания по максимуму дат с пометкой достоверности в `crates/freespace-analysis/src/cleanup.rs`
- [x] T066 [US7] Реализовать категории temp/cache/log/backup и рекомендации в `crates/freespace-analysis/src/recommend.rs`
- [x] T067 [US7] Реализовать поиск пустых каталогов в `crates/freespace-scan/src/empty_dirs.rs`
- [x] T068 [US7] Реализовать CLI `cleanup suggest` и `cleanup empty-dirs` в `crates/freespace-cli/src/commands/cleanup.rs`
- [x] T069 [US7] Реализовать экран рекомендаций и пустых папок в GUI в `apps/freespace-gui/src/views/cleanup.rs`

---

## Phase 10: User Story 8 — Снимки состояния и сравнение (P2)

**Goal**: сохранение снимков `.fssnap` и сравнение изменений по каталогам и файлам

**Independent Test**: два снимка с известными различиями — проверить полноту и сортировку отчёта

- [x] T070 [P] [US8] Тест round-trip снимка и сравнения в `tests/snapshot.rs`
- [x] T071 [US8] Реализовать формат `.fssnap` (CBOR + zstd, колонки, checksum) в `crates/freespace-report/src/snapshot.rs`
- [x] T072 [US8] Реализовать сравнение снимков и дельту в `crates/freespace-report/src/diff.rs`
- [x] T073 [US8] Реализовать CLI `snapshot create` и `snapshot diff` в `crates/freespace-cli/src/commands/snapshot.rs`
- [x] T074 [US8] Реализовать экран снимков и сравнения в GUI в `apps/freespace-gui/src/views/snapshots.rs`

---

## Phase 11: User Story 9 — Безопасные операции, журнал и отмена (P2)

**Goal**: единый сценарий подтверждения с выбором способа удаления, журнал, карантин и undo

**Independent Test**: выполнить удаление, восстановить объект из карантина, проверить журнал и отмену

- [x] T075 [P] [US9] Тест выбора режима удаления, карантина и undo в `tests/safe_ops.rs`
- [x] T076 [US9] Реализовать диалог подтверждения с выбором «Корзина/Карантин/Безвозвратно» в `apps/freespace-gui/src/views/confirm.rs`
- [x] T077 [US9] Реализовать удаление в Корзину через `IFileOperation` в `crates/freespace-platform/src/recycle.rs`
- [x] T078 [US9] Реализовать проверку свободного места и сценарий нехватки в `crates/freespace-ops/src/delete.rs`
- [x] T079 [US9] Реализовать журнал операций, undo и восстановление в `crates/freespace-ops/src/undo.rs` и `apps/freespace-gui/src/views/journal.rs`
- [x] T080 [US9] Реализовать CLI `delete --mode` с проверками подтверждения в `crates/freespace-cli/src/commands/delete.rs`
- [x] T081 [US9] Реализовать защиту системных путей, запрет выхода за выбранную область и дополнительное подтверждение в `crates/freespace-ops/src/guarded.rs`

---

## Phase 12: User Story 10 — Точный учёт места: ссылки, сжатие, sparse, ADS (P3)

**Goal**: корректный физический и логический размер с учётом ссылок, сжатия, разреженности и потоков

**Independent Test**: набор с hard links, junction, сжатыми/разреженными файлами и ADS — сверить с эталоном

- [x] T082 [P] [US10] Тест дедупликации hard links, junction, sparse/compressed и ADS в `tests/accounting.rs`
- [x] T083 [US10] Реализовать определение reparse points и ссылок в `crates/freespace-platform/src/reparse.rs`
- [x] T084 [US10] Реализовать точный размер на диске (`GetCompressedFileSizeW`, allocated ranges) в `crates/freespace-platform/src/allocated.rs`
- [x] T085 [US10] Реализовать дедупликацию физического размера по FRN в `crates/freespace-analysis/src/accounting.rs`
- [x] T086 [US10] Реализовать перечисление ADS (`FindFirstStreamW`) в `crates/freespace-platform/src/ads.rs`
- [x] T087 [US10] Реализовать отображение метрик и ADS в GUI в `apps/freespace-gui/src/views/size_details.rs`
- [x] T088 [US10] Реализовать CLI-флаги учёта ADS и режима размера в `crates/freespace-cli/src/commands/scan.rs`

---

## Phase 13: User Story 11 — Файловый менеджер длинных путей (P3)

**Goal**: операции с путями длиннее 260 символов без системной политики

**Independent Test**: создать пути 261/1000/32000 символов и выполнить rename/move/copy/delete

- [x] T089 [P] [US11] Тест операций на длинных путях и предельных значениях в `tests/long_paths.rs`
- [x] T090 [US11] Реализовать файловый менеджер длинных путей в `crates/freespace-ops/src/long_path_ops.rs`
- [x] T091 [US11] Реализовать проверку предела пути и понятные ошибки до изменения данных в `crates/freespace-platform/src/paths.rs`
- [x] T092 [US11] Реализовать UI файлового менеджера длинных путей в `apps/freespace-gui/src/views/long_paths.rs`

---

## Phase 14: User Story 12 — Массовое переименование (P3)

**Goal**: правила RegEx/шаблонов, предпросмотр, разрешение конфликтов и откат

**Independent Test**: применить правило к 1000 файлам, проверить предпросмотр, конфликты, результат и откат

- [x] T093 [P] [US12] Тест правил переименования, конфликтов и отката в `tests/rename.rs`
- [x] T094 [US12] Реализовать движок правил переименования в `crates/freespace-ops/src/rename.rs`
- [x] T095 [US12] Реализовать предпросмотр, обнаружение конфликтов и журнал отката в `crates/freespace-ops/src/rename_plan.rs`
- [x] T096 [US12] Реализовать CLI `rename` в `crates/freespace-cli/src/commands/rename.rs`
- [x] T097 [US12] Реализовать UI массового переименования в `apps/freespace-gui/src/views/rename.rs`

---

## Phase 15: User Story 13 — Атрибуты и права NTFS (P3)

**Goal**: пакетная смена атрибутов, владельца и прав с отчётом об ошибках

**Independent Test**: сменить атрибуты, владельца и права на тестовом наборе, проверить отчёт и повышение прав

- [x] T098 [P] [US13] Тест смены атрибутов/владельца/прав (admin-gated) в `tests/permissions.rs`
- [x] T099 [US13] Реализовать установку и снятие атрибутов в `crates/freespace-ops/src/attributes.rs`
- [x] T100 [US13] Реализовать работу с ACL и владельцем (привилегии, `SetNamedSecurityInfoW`) в `crates/freespace-platform/src/security.rs`
- [x] T101 [US13] Реализовать пакетную смену прав и отчёт об ошибках в `crates/freespace-ops/src/acl.rs`
- [x] T102 [US13] Реализовать CLI `attrs` и `acl` в `crates/freespace-cli/src/commands/attrs.rs`
- [x] T103 [US13] Реализовать UI прав и атрибутов в `apps/freespace-gui/src/views/permissions.rs`

---

## Phase 16: User Story 14 — Метаданные и расширенная фильтрация (P3)

**Goal**: чтение EXIF, аудио/видео и версий PE, фильтрация и сортировка по метаданным

**Independent Test**: подготовить файлы с известными метаданными и проверить отображение, фильтры и устойчивость к повреждениям

- [x] T104 [P] [US14] Тест чтения EXIF/аудио/видео/PE и повреждённых файлов в `tests/metadata.rs`
- [x] T105 [US14] Реализовать чтение EXIF в `crates/freespace-analysis/src/metadata/exif.rs`
- [x] T106 [US14] Реализовать чтение аудио/видео метаданных в `crates/freespace-analysis/src/metadata/media.rs`
- [x] T107 [US14] Реализовать чтение версий PE в `crates/freespace-analysis/src/metadata/pe.rs`
- [x] T108 [US14] Реализовать фоновое извлечение и кеш метаданных в `crates/freespace-analysis/src/metadata/mod.rs`
- [x] T109 [US14] Реализовать столбцы и фильтры метаданных в GUI в `apps/freespace-gui/src/views/metadata.rs`
- [x] T110 [US14] Реализовать CLI-фильтры метаданных в `crates/freespace-cli/src/commands/filter.rs` и соответствующий контракт

---

## Phase 17: User Story 15 — Анализ по владельцам (P3)

**Goal**: сводка по владельцам и фильтрация дерева по владельцу

**Independent Test**: набор с файлами разных владельцев — проверить суммы, фильтр и категорию «неизвестно»

- [x] T111 [P] [US15] Тест агрегации по владельцам в `tests/owners.rs`
- [x] T112 [US15] Реализовать интернирование SID и сводку владельцев в `crates/freespace-analysis/src/owners.rs`
- [x] T113 [US15] Реализовать фильтр дерева по владельцу в `crates/freespace-core/src/query.rs`
- [x] T114 [US15] Реализовать отчёт и UI владельцев в `apps/freespace-gui/src/views/owners.rs` и `crates/freespace-report/src/owners.rs`

---

## Phase 18: User Story 16 — Просмотр архивов без распаковки (P3)

**Goal**: содержимое ZIP/7z/TAR/ISO/RAR с размерами и экспортом списка

**Independent Test**: открыть архивы известных форматов, проверить список, сортировку, пароли и повреждения

- [x] T115 [P] [US16] Тест чтения архивов и обработки паролей/повреждений в `tests/archives.rs`
- [x] T116 [US16] Реализовать ридеры ZIP/7z/TAR/ISO в `crates/freespace-analysis/src/archives/mod.rs`
- [x] T117 [US16] Реализовать опциональный RAR с проверкой наличия библиотеки в `crates/freespace-analysis/src/archives/rar.rs`
- [x] T118 [US16] Реализовать лимиты ресурсов и обработку ошибок архивов в `crates/freespace-analysis/src/archives/limits.rs`
- [x] T119 [US16] Реализовать CLI `archives` и UI просмотра архивов в `crates/freespace-cli/src/commands/archives.rs` и `apps/freespace-gui/src/views/archives.rs`

---

## Phase 19: User Story 17 — Экспорт отчётов (P4)

**Goal**: выгрузка в CSV/XML/HTML/XLSX с выбором состава и потоковой записью

**Independent Test**: экспортировать один результат во все форматы и сверить суммы и открытие в целевых инструментах

- [x] T120 [P] [US17] Тест схем CSV/XML/HTML/XLSX и согласованности сумм в `tests/export.rs`
- [x] T121 [US17] Реализовать потоковый CSV-экспорт в `crates/freespace-report/src/csv.rs`
- [x] T122 [US17] Реализовать XML-экспорт (`quick-xml`) в `crates/freespace-report/src/xml.rs`
- [x] T123 [US17] Реализовать HTML-экспорт с экранированием в `crates/freespace-report/src/html.rs`
- [x] T124 [US17] Реализовать XLSX-экспорт (`rust_xlsxwriter`) в `crates/freespace-report/src/xlsx.rs`
- [x] T125 [US17] Реализовать CLI `export` и диалог экспорта в GUI в `crates/freespace-cli/src/commands/export.rs` и `apps/freespace-gui/src/views/export.rs`

---

## Phase 20: User Story 18 — CLI и запуск по расписанию (P4)

**Goal**: полный CLI-паритет, JSON-схемы, блокировка параллельных запусков и экспорт задания Планировщика

**Independent Test**: выполнить основные команды в неинтерактивном режиме, проверить JSON, коды возврата и XML-задание

- [x] T126 [P] [US18] Тест JSON-схем и кодов возврата всех команд в `tests/cli_contract.rs`
- [x] T127 [US18] Реализовать экспорт задания Планировщика (XML) в `crates/freespace-cli/src/commands/schedule.rs`
- [x] T128 [US18] Реализовать блокировку и очередь параллельных запусков в `crates/freespace-cli/src/lock.rs`
- [x] T129 [US18] Задокументировать CLI-контракт и коды возврата в `docs/cli.md`
- [x] T130 [US18] Реализовать профили автоматизации в `crates/freespace-ops/src/automation.rs`

---

## Phase 21: User Story 19 — Интеграция в контекстное меню (P4)

**Goal**: пункты анализа и разблокировки в Проводнике с установкой и удалением

**Independent Test**: установить интеграцию, проверить пункты для файла/папки/диска, удалить интеграцию

- [ ] T131 [P] [US19] Тест установки и удаления записей реестра в HKCU в `tests/context_menu.rs`
- [ ] T132 [US19] Реализовать установку/удаление контекстного меню (`winreg`) в `crates/freespace-platform/src/context_menu.rs`
- [ ] T133 [US19] Реализовать корректную передачу путей (Unicode, пробелы, длинные пути) в `crates/freespace-cli/src/main.rs`
- [ ] T134 [US19] Реализовать CLI `context-menu install/uninstall` в `crates/freespace-cli/src/commands/context_menu.rs`

---

## Phase 22: User Story 20 — Мониторинг записи на диск (P4)

**Goal**: события записи с процессом, путём и скоростью, пороги и история

**Independent Test**: запустить тестовую запись известного объёма и проверить событие, скорость и уведомление

- [ ] T135 [P] [US20] Тест ETW-парсера и fallback на счётчики ввода-вывода в `tests/monitor.rs`
- [ ] T136 [US20] Реализовать ETW-сессию `Microsoft-Windows-Kernel-File` (`ferrisetw`) в `crates/freespace-platform/src/etw.rs`
- [ ] T137 [US20] Реализовать fallback `GetProcessIoCounters` и хвост USN в `crates/freespace-scan/src/monitor_fallback.rs`
- [ ] T138 [US20] Реализовать пороги, исключения процессов и путей, историю и уведомления в `crates/freespace-analysis/src/monitor.rs`
- [ ] T139 [US20] Реализовать CLI `monitor` (JSON Lines) и UI мониторинга с работой в области уведомлений в `crates/freespace-cli/src/commands/monitor.rs` и `apps/freespace-gui/src/views/monitor.rs`

---

## Phase 23: User Story 21 — Системные каталоги и повышение прав (P4)

**Goal**: UAC-elevation по требованию, доступ к защищённым каталогам и корректная деградация

**Independent Test**: проверить работу без прав и с правами, доступ к `System Volume Information`, отказ от повышения

- [ ] T140 [P] [US21] Тест повышения прав и доступа к системным каталогам (admin) в `tests/elevation.rs`
- [ ] T141 [US21] Реализовать UAC-elevation по требованию (`ShellExecute` runas) в `crates/freespace-platform/src/elevation.rs`
- [ ] T142 [US21] Реализовать доступ к защищённым каталогам и пометку недоступности в `crates/freespace-scan/src/access.rs`
- [ ] T143 [US21] Реализовать UI запроса повышения прав в `apps/freespace-gui/src/views/elevation.rs`

---

## Phase 24: User Story 22 — Дашборд всех дисков (P4)

**Goal**: сводка по томам, быстрый запуск анализа и динамика по снимкам

**Independent Test**: проверить отображение всех томов, корректность цифр и переход к анализу

- [ ] T144 [P] [US22] Тест сводки по томам и динамики снимков в `tests/dashboard.rs`
- [ ] T145 [US22] Реализовать типы и сбор сводки по томам в `crates/freespace-core/src/dashboard.rs` и `crates/freespace-platform/src/volumes.rs`
- [ ] T146 [US22] Реализовать экран дашборда и быстрый запуск анализа в `apps/freespace-gui/src/views/dashboard.rs`
- [ ] T147 [US22] Реализовать CLI `drives` в `crates/freespace-cli/src/commands/drives.rs`

---

## Phase 25: User Story 23 — Настройки, локализация и портативный режим (P4)

**Goal**: настройки, полные переводы RU/EN, портативное хранение данных

**Independent Test**: переключить язык и тему, задать исключения, запустить портативную сборку и проверить отсутствие записей в профиль

- [ ] T148 [P] [US23] Тест переключения языка, настроек и портативного режима в `tests/settings.rs`
- [ ] T149 [US23] Заполнить полные локали `locales/ru.toml` и `locales/en.toml`
- [ ] T150 [US23] Реализовать экран настроек в GUI в `apps/freespace-gui/src/views/settings.rs`
- [ ] T151 [US23] Реализовать портативный режим и fallback хранения настроек в `crates/freespace-core/src/settings.rs`
- [ ] T152 [US23] Реализовать CLI `config get/set` в `crates/freespace-cli/src/commands/config.rs`
- [ ] T153 [US23] Реализовать регрессионный тест отсутствия сетевых вызовов и телеметрии в `tests/privacy.rs`

---

## Phase 26: Polish & Cross-Cutting Concerns

**Purpose**: производительность, документация, упаковка и финальная валидация

- [ ] T154 [P] Оптимизировать MFT-скан по результатам бенчмарков в `crates/freespace-scan/src/mft_scan.rs`
- [ ] T155 [P] Оптимизировать память дерева и арены в `crates/freespace-core/src/tree.rs`
- [ ] T156 [P] Прогнать `cargo fmt --check` и `cargo clippy -D warnings`, устранить предупреждения
- [ ] T157 [P] Подготовить пользовательскую документацию RU/EN в `docs/`, `ru/`, `en/`
- [ ] T158 [P] Подготовить портативный ZIP и установщик Inno Setup в `installer/freespace.iss`
- [ ] T159 [P] Создать `CHANGELOG.md` и зафиксировать версию 1.0.0
- [ ] T160 Прогнать все сценарии `quickstart.md` на Windows 10 и Windows 11 и зафиксировать результаты в `docs/release-validation.md`
- [ ] T161 Синхронизировать зеркала документации в `ru/` и `en/` и проверить соответствие конституции

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: без зависимостей, стартует сразу
- **Phase 2 (Foundational)**: зависит от Phase 1 и блокирует все истории
- **US1–US23**: зависят от Phase 2; US1 — источник данных для большинства историй
- **Polish (Phase 26)**: после завершения выбранных историй

### User Story Dependencies

- **US1 (P1)**: после Phase 2 — не зависит от других историй
- **US2 (P1)**: после US1 (данные дерева)
- **US3 (P1)**: после Phase 2 (не зависит от US1)
- **US4 (P1)**: после US1
- **US5 (P2)**: после US1 и US9 (безопасное удаление); поиск независим
- **US6 (P2)**: после US9 (подтверждения и журнал)
- **US7 (P2)**: после US1
- **US8 (P2)**: после US1
- **US9 (P2)**: после Phase 2; базовые операции доступны другим историям
- **US10 (P3)**: после US1
- **US11 (P3)**: после Phase 2 (слой путей)
- **US12 (P3)**: после US9 (undo)
- **US13 (P3)**: после Phase 2; зависит от US21 для повышения прав
- **US14 (P3)**: после US1
- **US15 (P3)**: после US1
- **US16 (P3)**: после US1
- **US17 (P4)**: после US1; использует данные US4, US5, US15 по мере готовности
- **US18 (P4)**: после US1 и US17 (экспорт в расписании)
- **US19 (P4)**: после Phase 2
- **US20 (P4)**: после Phase 2
- **US21 (P4)**: после Phase 2; блокирует административные части US13
- **US22 (P4)**: после Phase 2 и US8 (динамика)
- **US23 (P4)**: после Phase 2

### Within Each User Story

- Тесты (если включены) пишутся и падают до реализации
- Модели → сервисы → CLI/UI → интеграция
- История завершается независимой проверкой

### Parallel Opportunities

- Все задачи с `[P]` в Phase 1 и Phase 2 выполняются параллельно
- После Phase 2 истории P1 (US1–US4) могут идти параллельно с учётом зависимостей US2/US4 от US1
- Истории P3 и P4 могут выполняться параллельно после готовности своих зависимостей
- Внутри историй параллельны задачи с разными файлами (модели, тесты, CLI, UI)

---

## Parallel Example: US1

```text
Task: "T026 [P] [US1] Тест MFT-перечисления и сумм в tests/scan_mft.rs"
Task: "T027 [P] [US1] Тест fallback-обхода в tests/scan_walk.rs"
```

## Parallel Example: US5

```text
Task: "T053 [P] [US5] Тест конвейера хеширования в tests/duplicates.rs"
Task: "T054 [US5] Двухэтапное хеширование в crates/freespace-analysis/src/hash.rs"
Task: "T055 [US5] Кеш хешей в SQLite в crates/freespace-analysis/src/hash_cache.rs"
```

## Parallel Example: US17

```text
Task: "T121 [US17] CSV-экспорт в crates/freespace-report/src/csv.rs"
Task: "T122 [US17] XML-экспорт в crates/freespace-report/src/xml.rs"
Task: "T123 [US17] HTML-экспорт в crates/freespace-report/src/html.rs"
Task: "T124 [US17] XLSX-экспорт в crates/freespace-report/src/xlsx.rs"
```

## Parallel Example: P3-истории

```text
Task: "US10 — учёт ссылок, сжатия, sparse и ADS"
Task: "US11 — файловый менеджер длинных путей"
Task: "US14 — метаданные"
Task: "US16 — просмотр архивов"
```

---

## Implementation Strategy

### MVP First (P1)

1. Phase 1: Setup
2. Phase 2: Foundational
3. US1 → независимая проверка
4. US2, US3, US4 → независимые проверки
5. **STOP and VALIDATE**: MVP демонстрирует сканирование, дерево, Treemap, разблокировку, топ и типы

### Incremental Delivery

1. MVP (P1) → демонстрация
2. P2: US5–US9 → очистка, дубликаты, шредер, снимки, безопасные операции
3. P3: US10–US16 → продвинутая файловая система, метаданные, архивы
4. P4: US17–US23 → отчёты, CLI/расписание, контекстное меню, мониторинг, права, дашборд, настройки
5. Polish → релиз 1.0.0

### Parallel Team Strategy

- После Phase 2: разработчик A — US1; разработчик B — US3; разработчик C — US18/US19 (CLI/интеграции)
- После US1: US2, US4, US5, US8, US10, US14, US15, US16 распределяются параллельно
- US9 (безопасность) выполняется до US5/US6/US12, чтобы очистка сразу шла через общий слой

---

## Notes

- Задачи с `[P]` — разные файлы, нет зависимостей
- Метка `[Story]` обеспечивает трассируемость к спецификации
- Тесты критических модулей обязательны по конституции; UI-тесты опциональны
- Коммиты — Conventional Commits, после каждой задачи или логической группы
- Останавливайтесь на checkpoints для независимой проверки историй
- Избегайте задач без конкретных путей и зависимостей, ломающих независимость историй
