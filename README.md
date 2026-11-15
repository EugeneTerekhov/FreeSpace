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

Спецификация, план и задачи готовы (Spec-Driven Development). Реализация ещё не начата.
Specification, plan and tasks are ready (Spec-Driven Development). Implementation has not started.

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
```

Требования / Requirements: Rust stable 1.85+ (edition 2024), Windows 10 21H2+ / Windows 11 x64.
