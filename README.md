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

Фаза 1 (Setup) завершена: создан Cargo workspace, скелеты крейтов и GUI, CI и локализация.
Следующая фаза — Foundational (T009–T025).
Phase 1 (Setup) is complete: the Cargo workspace, crate and GUI skeletons, CI and localization
are in place. Next phase — Foundational (T009–T025).

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
