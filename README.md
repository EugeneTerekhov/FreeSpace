# FreeSpace

**FreeSpace** — нативное приложение для Windows, которое анализирует и освобождает дисковое
пространство: дерево размеров, поиск дубликатов и устаревших файлов, снимки диска, безопасные
операции с файлами и автоматизация через CLI.

**FreeSpace** is a native Windows application for analyzing and reclaiming disk space: a size
tree, duplicate and stale-file discovery, disk snapshots, safe file operations and CLI automation.

## Статус / Status

**1.2.0** — нативный клиент Tauri 2 + React: 11 экранов в 6 группах, тёмная и светлая темы,
DPI 100–200%, reduced motion, клавиатура и Narrator, RU/EN, безопасные операции и офлайн-установщик
с WebView2. CLI сохраняет паритет с GUI и печатает JSON.

**1.2.0** — native Tauri 2 + React client: 11 screens in 6 groups, dark and light themes,
100–200% DPI, reduced motion, keyboard and Narrator support, RU/EN, safe operations and an offline
installer with WebView2. The CLI keeps parity with the GUI and emits JSON.

## Документация / Documentation

Канонические документы — на русском; английские переводы лежат в `docs/en/`.
Canonical documents are written in Russian; English translations live in `docs/en/`.

| Документ / Document | RU | EN |
|---------------------|----|----|
| Руководство пользователя / User guide | [docs/ru/user-guide.md](docs/ru/user-guide.md) | [docs/en/user-guide.md](docs/en/user-guide.md) |
| Как устроен репозиторий / Repository guide | [docs/ru/repository-guide.md](docs/ru/repository-guide.md) | [docs/en/repository-guide.md](docs/en/repository-guide.md) |
| Контракт CLI / CLI contract | [docs/ru/cli.md](docs/ru/cli.md) | — |
| Конституция / Constitution | [.specify/memory/constitution.md](.specify/memory/constitution.md) | [docs/en/constitution.md](docs/en/constitution.md) |
| Спецификация 001 / Specification 001 | [specs/001-freespace-disk-analyzer/spec.md](specs/001-freespace-disk-analyzer/spec.md) | [docs/en/spec.md](docs/en/spec.md) |
| Спецификация 002 / Specification 002 | [specs/002-native-ui-redesign/spec.md](specs/002-native-ui-redesign/spec.md) | [docs/en/002-native-ui-redesign/spec.md](docs/en/002-native-ui-redesign/spec.md) |
| План, задачи, исследования / Plan, tasks, research | [specs/001-freespace-disk-analyzer/](specs/001-freespace-disk-analyzer/) | [docs/en/](docs/en/) |
| Архитектурное решение / ADR | [docs/ru/adr/0001-tauri-react-gui.md](docs/ru/adr/0001-tauri-react-gui.md) | [docs/en/adr/0001-tauri-react-gui.md](docs/en/adr/0001-tauri-react-gui.md) |
| Упаковка / Packaging | [installer/](installer/) | [installer/](installer/) |

История изменений — в [CHANGELOG.md](CHANGELOG.md).
Release history is in [CHANGELOG.md](CHANGELOG.md).

## Требования / Requirements

Windows 10 21H2+ или Windows 11, x64. Rust stable 1.85+ (edition 2024), Node.js 20.19+ для
интерфейса. Тулчейн зафиксирован в [rust-toolchain.toml](rust-toolchain.toml).
Windows 10 21H2+ or Windows 11, x64. Rust stable 1.85+ (edition 2024), Node.js 20.19+ for the
interface. The toolchain is pinned in [rust-toolchain.toml](rust-toolchain.toml).

## Сборка / Build

```powershell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Приложение / Desktop client:

```powershell
cd apps/freespace-desktop/ui
npm ci
npm run tauri dev
```

Фронтенд-проверки / Frontend checks: `npm run typecheck`, `npm run lint`, `npm run test:unit`,
`npm run test:e2e`, `npm run test:a11y`. Неинтерактивная проверка выполняется в CI:
[.github/workflows/ci.yml](.github/workflows/ci.yml).

## Хранение и офлайн / Storage and offline

Все данные — `settings.toml`, база, карантин, кеш хешей и блокировка CLI — находятся в папке
`data` рядом с исполняемым файлом. `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` и временные
каталоги не используются. Сетевых вызовов и телеметрии нет.
All data — `settings.toml`, the database, quarantine, the hash cache and the CLI lock — lives in
the `data` folder next to the executable. `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` and
temporary directories are never used. There are no network calls and no telemetry.

## Правила для AI-агентов / AI agent rules

Правила описаны в [AGENTS.md](AGENTS.md). Rules are described in [AGENTS.md](AGENTS.md).
