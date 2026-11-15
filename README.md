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

Фазы 3–8 (US1–US6) завершены: сканирование NTFS через MFT и USN-журнал, fallback-обход
каталогов, дерево размеров с поиском и сортировкой, интерактивная Treemap-карта с зумом и
легендой, разблокировка файлов через Restart Manager и удаление при перезагрузке, топ файлов и
группировка по типам, поиск дубликатов с побайтовой проверкой и кешем хешей, шредер с
предупреждением об SSD и TRIM, а также команды CLI `scan`, `tree`, `top`, `types`, `duplicates`,
`unlock` и `shred` (T026–T063). Следующая фаза — US7: устаревшие файлы, пустые папки и
рекомендации по очистке (T064–T072).
Phases 3–8 (US1–US6) are complete: NTFS scanning through the MFT and the USN journal, the
directory-walk fallback, the size tree with search and sorting, the interactive treemap with zoom
and legend, file unlocking through the Restart Manager and deletion on reboot, top files and type
grouping, duplicate search with byte verification and a hash cache, the shredder with SSD warnings
and TRIM, and the CLI commands `scan`, `tree`, `top`, `types`, `duplicates`, `unlock` and `shred`
(T026–T063). Next phase — US7: stale files, empty folders and cleanup recommendations (T064–T072).

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
