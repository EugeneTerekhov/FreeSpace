# План реализации: FreeSpace — анализатор и менеджер дискового пространства Windows

**Branch**: `001-freespace-disk-analyzer` | **Date**: 2026-11-15 | **Spec**: [spec.md](./spec.md)

## Summary

FreeSpace — нативное Windows-приложение (Rust + egui/eframe) для анализа и управления дисковым
пространством. Ядро — высокопроизводительный сканер NTFS на основе прямого чтения MFT с
инкрементальным обновлением через USN-журнал и fallback-обходом для прочих файловых систем.
Поверх общего ядра работают графический интерфейс, CLI и системные интеграции. Приложение
объединяет визуализацию (дерево, Treemap), поиск дубликатов, безопасную очистку с карантином и
журналом, продвинутую работу с файловой системой (ссылки, длинные пути, права NTFS, метаданные,
архивы) и автоматизацию (отчёты, расписание, контекстное меню, мониторинг).

## Technical Context

**Language/Version**: Rust 1.85+ (edition 2024), Windows x64

**Primary Dependencies**:
- UI: `eframe`/`egui` (нативный immediate-mode GUI), `egui_extras` (виртуализированные таблицы)
- Системные вызовы: `windows` (официальный crate Microsoft), `winreg`
- Параллелизм: `rayon`, `crossbeam-channel`, `jwalk` (fallback-обход)
- Хранение: `rusqlite` (bundled SQLite — журнал, карантин, настройки), `serde` + `toml`/`json`
- Идентификаторы: `uuid` (v4 для операций и сессий, v5 для стабильных идентификаторов томов)
- Снимки: `serde` + `ciborium` (CBOR) + `zstd`
- Хеширование: `sha2`, `md-5`, `memmap2`
- Архивы: `zip`, `sevenz-rust`, `tar` + `flate2`, `iso9660`; `unrar` — опционально (RAR)
- Метаданные: `kamadak-exif`, `lofty` (аудио), `mp4parse` (видео), `pelite` (версии PE)
- Отчёты: `csv`, `quick-xml`, `rust_xlsxwriter`, HTML-шаблоны
- CLI: `clap` v4
- Локализация: `rust-i18n` (RU/EN, переключение в рантайме)
- Массовое переименование: `regex` (линейный по времени движок без катастрофического
  бэктрекинга)
- Поиск/фильтры: `globset`
- Мониторинг: `ferrisetw` (ETW, провайдер `Microsoft-Windows-Kernel-File`) с fallback на
  `GetProcessIoCounters`

**Dependencies added by phases 16–20** (versions resolved by Cargo):
- Metadata: `kamadak-exif 0.6`, `lofty 0.25`, `mp4parse 0.17`, `pelite 0.10`.
- Archives: `zip 9` (deflate/bzip2/zstd/lzma), `sevenz-rust 0.6`, `tar 0.4`, `flate2 1`,
  `iso9660 0.1`; `unrar 0.5` is an optional feature `rar` because it needs a native library.
- Reports: `csv 1.4`, `quick-xml 0.42`, `rust_xlsxwriter 0.102` with the `constant_memory`
  feature so large XLSX exports stream to a temporary file.
- CLI locks use the existing `windows` crate (`LockFileEx`); no new dependency is required.
- Visual identity: `image 0.25` with the `png` feature decodes the embedded splash artwork and window icon; the `embed-resource 3` build dependency embeds `icon.ico` into the executable. `image` is already in the `eframe` dependency tree, so no new runtime crate is added.

**Storage**: только каталог `data` рядом с исполняемым файлом. SQLite (журнал операций, индекс
карантина, кеш хешей), TOML-настройки, карантин и CLI-блокировка находятся в этом каталоге; файлы
снимков `.fssnap` (CBOR + zstd, версионированная схема). `%APPDATA%`, `%LOCALAPPDATA%`,
`%ProgramData%` и временные каталоги не используются.

**Testing**: `cargo test` (unit + integration), `assert_cmd`/`predicates` (CLI), `tempfile` и
`assert_fs` (фикстуры ФС), `criterion` (бенчмарки), административные тесты на NTFS/VHD — только по
явному флагу окружения

**Target Platform**: Windows 10 21H2+ и Windows 11, x64; NTFS как основная ФС

**Project Type**: desktop-app + CLI (Cargo workspace)

**Performance Goals**:
- Анализ NTFS-тома до 8 ТБ через MFT — не более 60 с на потребительском SSD
- Отклик интерфейса на действия пользователя — не более 200 мс при сканировании
- Отрисовка Treemap — не менее 30 кадров/с на типовых сценариях, плавный зум
- Поиск дубликатов на 1 ТБ — не более 10 минут (при прогреве файлового кеша)
- Инкрементальное обновление по USN — не более 5 с на 100 тыс. изменений

**Constraints**:
- Полная работа офлайн, отсутствие телеметрии и сетевых запросов
- Бюджет памяти на сессию сканирования 10 млн узлов — не более 4 ГБ
- Поддержка путей до 32 000 символов без системной политики LongPathsEnabled
- UAC-elevation по требованию; корректная деградация без прав администратора
- Все разрушающие операции — только после подтверждения и с записью в журнал
- GUI и CLI используют общие крейты ядра; паритет возможностей обязателен
- Никаких записей в `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` и временные каталоги; при
  отсутствии прав на запись — работа в памяти с уведомлением, без системного fallback
- Контекстное меню отключено по умолчанию, управляется настройкой `context_menu_enabled`;
  портативная сборка собирается с `--no-default-features` без компонента `context-menu`

**Scale/Scope**: до 10 млн файлов на том, тома до 8 ТБ, 23 пользовательские истории, 96
функциональных требований; 8 крейтов ядра/утилит + GUI-приложение

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Принцип | Соответствие | Обоснование |
|---------|--------------|-------------|
| I. Безопасность разрушающих операций | PASS | Единый слой операций: предпросмотр, выбор способа удаления, карантин/Корзина, журнал, undo, защита системных путей |
| II. Точность учёта места | PASS | `SizeMetrics` (логический/на диске/физический), дедупликация hard links по FRN, ADS, sparse/compressed |
| III. Производительность на больших объёмах | PASS | MFT + USN, `rayon`, потоковая выдача результатов, LOD Treemap, виртуализация таблиц |
| IV. Нативная точность Windows | PASS | `windows` crate, verbatim-пути, Restart Manager, ACL API, TRIM, реестр, ETW |
| V. Тест-первый подход | PASS | Фикстуры ФС, интеграционные тесты, регрессия отсутствия системных каталогов, административные тесты за флагом |
| VI. Паритет CLI и GUI | PASS | Общие крейты ядра, CLI на `clap` с JSON-выводом, `context-menu uninstall` и документированными кодами возврата |
| VII. Приватность и офлайн | PASS | Нет сетевых зависимостей и телеметрии; все данные локальны в каталоге приложения |

Нарушений, требующих обоснования в Complexity Tracking, нет.

## Project Structure

### Documentation (this feature)

```text
specs/001-freespace-disk-analyzer/
├── plan.md              # Этот файл
├── research.md          # Phase 0: технические решения и альтернативы
├── data-model.md        # Phase 1: модель данных
├── quickstart.md        # Phase 1: сценарии проверки
├── contracts/           # Phase 1: контракты CLI, отчётов и снимков
│   ├── cli.md
│   ├── report-formats.md
│   └── snapshot-format.md
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2: задачи (/speckit.tasks)
```

### Source Code (repository root)

```text
Cargo.toml                        # workspace, общие профили сборки
rust-toolchain.toml               # фиксация стабильного тулчейна
crates/
├── freespace-core/               # доменная модель, дерево, метрики размеров, настройки
│   ├── src/
│   └── tests/
├── freespace-platform/           # WinAPI-обёртки: тома, MFT/USN, блокировки, ACL, TRIM, реестр, ETW
│   ├── src/
│   └── tests/
├── freespace-scan/               # сканеры MFT и обхода дерева, инкрементальные сессии
│   ├── src/
│   └── tests/
├── freespace-ops/                # операции: удаление, карантин, журнал, шредер, переименование, права
│   ├── src/
│   └── tests/
├── freespace-analysis/           # дубликаты, очистка, метаданные, владельцы, архивы
│   ├── src/
│   └── tests/
├── freespace-report/             # экспорт CSV/XML/HTML/Excel, снимки и сравнение
│   ├── src/
│   └── tests/
└── freespace-cli/                # командная строка (clap), JSON-вывод, коды возврата
    ├── src/
    └── tests/
apps/
└── freespace-desktop/            # Tauri 2 + React: 11 экранов в 6 группах
    ├── src-tauri/
    └── ui/
tests/                            # межкрейтовые интеграционные сценарии (CLI + ядро)
benches/                          # общие бенчмарки сканирования и хеширования
docs/ru/                          # общие русские документы
docs/en/                          # английские переводы
```

**Structure Decision**: Cargo workspace с разделением по ответственности. `freespace-core` не
зависит от WinAPI и содержит доменную модель и алгоритмы, что позволяет тестировать их без
системных вызовов. `freespace-platform` инкапсулирует все обращения к Windows API. Сканирование,
операции, анализ и отчёты — отдельные крейты, зависящие от `core` (и `platform` при необходимости).
GUI и CLI — тонкие фронтенды над общими крейтами, что обеспечивает паритет возможностей.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| — | — | — |

Нарушений конституции нет; сложность структуры оправдана требованиями изоляции WinAPI и паритета
CLI/GUI.

## Storage and context-menu policy

- `freespace-core::settings::data_root()` — единственная точка вычисления каталога данных:
  `<exe_dir>\data`. Через неё разрешаются `settings.toml`, `freespace.db`, `Quarantine` и
  `cli.lock`; `portable_mode` и `portable.flag` удалены.
- `context_menu_enabled` (по умолчанию `false`) управляет записями Проводника. CLI
  `config set context_menu_enabled` и GUI применяют значение сразу; `context-menu install` и
  `uninstall` синхронизируют настройку.
- `freespace-platform` получил необязательную Cargo-функцию `context-menu` (по умолчанию
  включена). Портативная сборка использует `--no-default-features`, поэтому не содержит
  `winreg`, команды `context-menu` и пунктов меню.
- Установщик Inno Setup больше не регистрирует пункты; деинсталлятор вызывает
  `context-menu uninstall` для обеих областей и удаляет `{app}\data`.
