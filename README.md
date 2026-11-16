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

**1.2.0**: интерфейс полностью переведён на нативный Tauri 2 + React. 24 экрана, тёмная и
светлая темы, DPI 100–200%, reduced motion, клавиатура и Narrator, RU/EN, безопасные операции,
визуальная регрессия и офлайн-установщик с WebView2. Старый egui-клиент удалён; собран ровно один
GUI. Подробности — [docs/release-validation-002.md](docs/release-validation-002.md).
**1.2.0**: the interface has moved to a native Tauri 2 + React client: 24 screens, dark and light
themes, DPI 100–200%, reduced motion, keyboard and Narrator support, RU/EN, safe operations, visual
regression and an offline installer with WebView2. The egui client is gone; exactly one GUI ships.
See [docs/release-validation-002.md](docs/release-validation-002.md).

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
| Приёмка интерфейса 002 / UI acceptance 002 | [docs/release-validation-002.md](docs/release-validation-002.md) | [docs/release-validation-002.md](docs/release-validation-002.md) |
| Доступность / Accessibility | [docs/accessibility-002.md](docs/accessibility-002.md) | [docs/accessibility-002.md](docs/accessibility-002.md) |
| Галерея эталонов / Baseline gallery | [apps/freespace-desktop/ui/tests/visual/gallery/index.html](apps/freespace-desktop/ui/tests/visual/gallery/index.html) | same |
| Упаковка / Packaging | [installer/](installer/) | [installer/](installer/) |
| План редизайна UI / UI redesign plan | [docs/ui-redesign-plan.md](docs/ui-redesign-plan.md) | [docs/ui-redesign-plan.md](docs/ui-redesign-plan.md) |
| Производительность UI / UI performance | [docs/performance-002.md](docs/performance-002.md) | [docs/performance-002.md](docs/performance-002.md) |

Правила для AI-агентов описаны в [AGENTS.md](AGENTS.md).
Rules for AI agents are described in [AGENTS.md](AGENTS.md).

Канонические артефакты Spec Kit находятся в [specs/001-freespace-disk-analyzer/](specs/001-freespace-disk-analyzer/) и [specs/002-native-ui-redesign/](specs/002-native-ui-redesign/).
Canonical Spec Kit artifacts live in [specs/001-freespace-disk-analyzer/](specs/001-freespace-disk-analyzer/) and [specs/002-native-ui-redesign/](specs/002-native-ui-redesign/).

## Интерфейс / UI

Нативное приложение — `apps/freespace-desktop` (Tauri 2 + React): один GUI, 24 экрана и общие
дизайн-токены. Интерактивный макет языка интерфейса вынесен в отдельный проект
FreeSpace-Page (GitHub Pages) как визуальный референс; он не входит в поставку. Визуальные
эталоны и галерея для ревью —
[apps/freespace-desktop/ui/tests/visual/gallery/](apps/freespace-desktop/ui/tests/visual/gallery/).
The native client lives in `apps/freespace-desktop` (Tauri 2 + React): one GUI, 24 screens and
shared design tokens. The interactive UI-language mock now lives in the separate FreeSpace-Page
GitHub Pages project as a visual reference and is not part of the distribution. The visual
baselines and the review gallery are in
[apps/freespace-desktop/ui/tests/visual/gallery/](apps/freespace-desktop/ui/tests/visual/gallery/).

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
```

Приложение / Desktop client (Tauri 2 + React):

```powershell
cd apps/freespace-desktop/ui
npm ci
npm run tauri dev
```

Приложению нужны Rust stable 1.85+ и Node 20.19+; фронтенд-тесты —
`npm run test:unit`, `npm run test:e2e`, `npm run test:a11y` и `npm run test:visual`.
The client needs Rust stable 1.85+ and Node 20.19+; frontend checks are
`npm run typecheck`, `npm run lint`, `npm run test:unit`, `npm run test:e2e`, `npm run test:a11y`
and `npm run test:visual`.

Требования / Requirements: Rust stable 1.85+ (edition 2024), Windows 10 21H2+ / Windows 11 x64.
Тулчейн фиксируется в [rust-toolchain.toml](rust-toolchain.toml), линты и профили сборки — в
[Cargo.toml](Cargo.toml). Неинтерактивная проверка выполняется в CI
([.github/workflows/ci.yml](.github/workflows/ci.yml)).
The toolchain is pinned in [rust-toolchain.toml](rust-toolchain.toml); lints and build profiles
live in [Cargo.toml](Cargo.toml). CI ([.github/workflows/ci.yml](.github/workflows/ci.yml)) runs
the same checks non-interactively.
