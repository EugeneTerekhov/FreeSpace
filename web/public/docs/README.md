# FreeSpace

**FreeSpace** — нативное приложение для Windows, которое анализирует и освобождает дисковое
пространство: строит дерево размеров и интерактивную Treemap-карту, находит дубликаты, устаревшие
файлы и пустые папки, снимает блокировки с файлов, работает с длинными путями, правами NTFS,
метаданными и архивами, сравнивает снимки состояния диска и автоматизирует очистку через CLI.

**FreeSpace** is a native Windows application for analyzing and reclaiming disk space: it builds a
size tree and an interactive treemap, finds duplicates, stale files and empty folders, unlocks
files, handles long paths, NTFS permissions, metadata and archives, compares disk snapshots, and
automates cleanup via CLI.

## Статус / Status

Фазы 3–20 (US1–US18) завершены: сканирование NTFS через MFT и USN-журнал, fallback-обход
каталогов, дерево размеров с поиском и сортировкой, интерактивная Treemap-карта с зумом и
легендой, разблокировка файлов через Restart Manager и удаление при перезагрузке, топ файлов и
группировка по типам, поиск дубликатов с побайтовой проверкой и кешем хешей, шредер с
предупреждением об SSD и TRIM, рекомендации по очистке, пустые папки и фильтры устаревания,
снимки `.fssnap` с сравнением, безопасные операции с Корзиной, карантином, журналом и отменой,
точный учёт места с hard links, reparse points, сжатием, sparse и ADS, файловый менеджер длинных
путей, массовое переименование с предпросмотром и откатом, пакетная смена атрибутов, владельца и
прав NTFS, метаданные EXIF/аудио/видео/PE с фильтрацией, отчёт по владельцам, просмотр ZIP/7z/
TAR/GZ/ISO без распаковки, экспорт CSV/XML/HTML/XLSX и автоматизация CLI с блокировкой
параллельных запусков и экспортом задания Планировщика, контекстное меню Проводника (выключено по
умолчанию), мониторинг записи на диск с ETW и резервным режимом по счётчикам, повышение прав по
требованию, дашборд всех дисков и локальное хранение в папке `data` рядом с исполняемым файлом.
CLI: `scan`, `tree`, `top`, `types`,
`filter`, `duplicates`, `owners`, `cleanup`, `unlock`, `shred`, `snapshot`, `delete`, `rename`,
`attrs`, `acl`, `archives`, `export`, `schedule`, `drives`, `config`, `monitor` и `context-menu`
(T026–T169).
Phases 3–20 (US1–US18) are complete: NTFS scanning through the MFT and the USN journal, the
directory-walk fallback, the size tree with search and sorting, the interactive treemap with zoom
and legend, file unlocking through the Restart Manager and deletion on reboot, top files and type
grouping, duplicate search with byte verification and a hash cache, the shredder with SSD warnings
and TRIM, cleanup recommendations, empty folders and stale-file filters, `.fssnap` snapshots with
comparison, safe operations with the Recycle Bin, quarantine, journal and undo, exact space
accounting for hard links, reparse points, compression, sparse files and ADS, the long-path file
manager, bulk rename with preview and rollback, batch attribute, owner and NTFS permission changes,
EXIF/audio/video/PE metadata with filtering, per-owner reports, ZIP/7z/TAR/GZ/ISO inspection
without unpacking, CSV/XML/HTML/XLSX export, CLI automation with parallel-run locking and Task
Scheduler export, Explorer context-menu integration (disabled by default), real-time write monitoring
through ETW with a counter-based fallback, on-demand elevation, the all-drive dashboard and
application-local storage in the `data` folder next to the executable. CLI: `scan`,
`tree`, `top`, `types`, `filter`, `duplicates`, `owners`, `cleanup`, `unlock`, `shred`, `snapshot`,
`delete`, `rename`, `attrs`, `acl`, `archives`, `export`, `schedule`, `drives`, `config`, `monitor`
and `context-menu` (T026–T169).

## Хранение и контекстное меню / Storage and context menu

Все данные — `settings.toml`, `freespace.db`, карантин, кеш хешей и блокировка CLI — находятся в
папке `data` рядом с исполняемым файлом. `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` и временные
каталоги не используются. Пункты контекстного меню отсутствуют по умолчанию и включаются настройкой
`context_menu_enabled`; портативная сборка собирается с `--no-default-features` без этого компонента.
All data lives in the `data` folder next to the executable; `%APPDATA%`, `%LOCALAPPDATA%`,
`%ProgramData%` and temporary directories are never used. Explorer entries are disabled by default
and are controlled by `context_menu_enabled`; the portable build is produced with
`--no-default-features` and contains no context-menu component.

## Документация / Documentation

| Документ / Document | RU | EN |
|---------------------|----|----|
| Конституция проекта / Project constitution | [ru/constitution.md](ru/constitution.md) | [en/constitution.md](en/constitution.md) |
| Спецификация / Specification | [ru/spec.md](ru/spec.md) | [en/spec.md](en/spec.md) |
| План реализации / Implementation plan | [ru/plan.md](ru/plan.md) | [en/plan.md](en/plan.md) |
| Задачи / Tasks | [ru/tasks.md](ru/tasks.md) | [en/tasks.md](en/tasks.md) |
| Исследования / Research | [ru/research.md](ru/research.md) | [en/research.md](en/research.md) |
| Модель данных / Data model | [ru/data-model.md](ru/data-model.md) | [en/data-model.md](en/data-model.md) |
| Быстрый старт / Quickstart | [ru/quickstart.md](ru/quickstart.md) | [en/quickstart.md](en/quickstart.md) |
| Контракты / Contracts | [ru/contracts/](ru/contracts/) | [en/contracts/](en/contracts/) |
| Чек-листы / Checklists | [ru/checklists/](ru/checklists/) | [en/checklists/](en/checklists/) |
| Как пользоваться репозиторием / Repository guide | [ru/repository-guide.md](ru/repository-guide.md) | [en/repository-guide.md](en/repository-guide.md) |
| Руководство пользователя / User guide | [ru/user-guide.md](ru/user-guide.md) | [en/user-guide.md](en/user-guide.md) |
| Контракт CLI / CLI contract | [docs/cli.md](docs/cli.md) | [docs/cli.md](docs/cli.md) |
| Валидация релиза / Release validation | [docs/release-validation.md](docs/release-validation.md) | [docs/release-validation.md](docs/release-validation.md) |
| Упаковка / Packaging | [installer/](installer/) | [installer/](installer/) |

Правила для AI-агентов описаны в [AGENTS.md](AGENTS.md).
Rules for AI agents are described in [AGENTS.md](AGENTS.md).

Канонические артефакты Spec Kit находятся в [specs/001-freespace-disk-analyzer/](specs/001-freespace-disk-analyzer/).
Canonical Spec Kit artifacts live in [specs/001-freespace-disk-analyzer/](specs/001-freespace-disk-analyzer/).

## Процесс / Process

Проект ведётся по методологии [GitHub Spec Kit](https://github.com/github/spec-kit):
конституция → specify → clarify → plan → checklist → tasks → implement → converge.
The project follows [GitHub Spec Kit](https://github.com/github/spec-kit): constitution → specify →
clarify → plan → checklist → tasks → implement → converge.

## Сборка / Build

```powershell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo run -p freespace-gui
```

Требования / Requirements: Rust stable 1.85+ (edition 2024), Windows 10 21H2+ / Windows 11 x64.
Тулчейн фиксируется в [rust-toolchain.toml](rust-toolchain.toml), линты и профили сборки — в
[Cargo.toml](Cargo.toml). Неинтерактивная проверка выполняется в CI
([.github/workflows/ci.yml](.github/workflows/ci.yml)).
The toolchain is pinned in [rust-toolchain.toml](rust-toolchain.toml); lints and build profiles
live in [Cargo.toml](Cargo.toml). CI ([.github/workflows/ci.yml](.github/workflows/ci.yml)) runs
the same checks non-interactively.
