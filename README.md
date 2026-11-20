# FreeSpace

**FreeSpace** — портативная кроссплатформенная утилита для Windows и Linux, которая анализирует и
освобождает дисковое пространство: дерево размеров, поиск дубликатов и устаревших файлов, снимки
диска, безопасные операции с файлами и автоматизация через CLI.

**FreeSpace** is a portable cross-platform utility for Windows and Linux that analyses and reclaims
disk space: a size tree, duplicate and stale-file discovery, disk snapshots, safe file operations
and CLI automation.

## Статус / Status

**2.0.0** — Windows 10 21H2+/11 x64 и Linux x86_64. 9 экранов в 6 группах, тёмная и светлая темы,
DPI 100–200%, reduced motion, клавиатура и Narrator, RU/EN. Удаление выполняется в системную
корзину (Recycle Bin / XDG trash) или безвозвратно; карантина и установщиков больше нет. CLI
сохраняет паритет с GUI и печатает JSON, включая `freespace capabilities --json`.

**2.0.0** — Windows 10 21H2+/11 x64 and Linux x86_64. 9 screens in 6 groups, dark and light
themes, 100–200% DPI, reduced motion, keyboard and Narrator support, RU/EN. Deletion uses the system
trash (Recycle Bin / XDG trash) or is permanent; the application quarantine and installers are gone.
The CLI keeps parity with the GUI and emits JSON, including `freespace capabilities --json`.

## Платформы и поставка / Platforms and distribution

| Платформа / Platform | Артефакт / Artifact |
|----------------------|---------------------|
| Windows 10 21H2+ / Windows 11 x64 | `freespace-portable-<version>-windows-x64.zip` |
| Linux x86_64 | `freespace-portable-<version>-linux-x86_64.tar.gz` |

- Только portable-сборки: установщики NSIS/MSI/deb/rpm, AppImage, Flatpak и Snap не выпускаются.
- Portable builds only: no NSIS/MSI/deb/rpm installers, AppImage, Flatpak or Snap.
- Сборки не подписаны; Windows SmartScreen может предупредить о неизвестном издателе. Проверяйте
  `SHA256SUMS.txt` из релиза.
- Builds are unsigned; Windows SmartScreen may warn about an unknown publisher. Verify
  `SHA256SUMS.txt` from the release.
- Все данные приложения — `data` рядом с исполняемыми файлами; записей в реестре нет.
- All application data lives in `data` next to the executables; no registry entries are created.

## Документация / Documentation

Канонические документы — на русском; английские переводы лежат в `docs/en/`.
Canonical documents are written in Russian; English translations live in `docs/en/`.

| Документ / Document | RU | EN |
|---------------------|----|----|
| Руководство пользователя / User guide | [docs/ru/user-guide.md](docs/ru/user-guide.md) | [docs/en/user-guide.md](docs/en/user-guide.md) |
| Контракт CLI / CLI contract | [docs/ru/cli.md](docs/ru/cli.md) | [docs/en/cli.md](docs/en/cli.md) |
| Как устроен репозиторий / Repository guide | [docs/ru/repository-guide.md](docs/ru/repository-guide.md) | [docs/en/repository-guide.md](docs/en/repository-guide.md) |
| Конституция / Constitution | [.specify/memory/constitution.md](.specify/memory/constitution.md) | [docs/en/constitution.md](docs/en/constitution.md) |
| Спецификация 001 / Specification 001 | [specs/001-freespace-disk-analyzer/spec.md](specs/001-freespace-disk-analyzer/spec.md) | [docs/en/spec.md](docs/en/spec.md) |
| Спецификация 002 / Specification 002 | [specs/002-native-ui-redesign/spec.md](specs/002-native-ui-redesign/spec.md) | [docs/en/002-native-ui-redesign/spec.md](docs/en/002-native-ui-redesign/spec.md) |
| FreeSpace 2.0: спецификация / specification | [specs/003-cross-platform-portable/spec.md](specs/003-cross-platform-portable/spec.md) | — |
| FreeSpace 2.0: план и задачи / plan and tasks | [specs/003-cross-platform-portable/](specs/003-cross-platform-portable/) | — |
| Архитектурное решение / ADR | [docs/ru/adr/0001-tauri-react-gui.md](docs/ru/adr/0001-tauri-react-gui.md) | [docs/en/adr/0001-tauri-react-gui.md](docs/en/adr/0001-tauri-react-gui.md) |

История изменений — в [CHANGELOG.md](CHANGELOG.md).
Release history is in [CHANGELOG.md](CHANGELOG.md).

## Требования / Requirements

- **Windows**: Windows 10 21H2+ или Windows 11, x64. Для GUI нужен WebView2 Evergreen; архив может
  содержать опциональный fixed runtime в папке `webview2`.
- **Windows**: Windows 10 21H2+ or Windows 11, x64. The GUI needs the Evergreen WebView2 runtime;
  an archive may bundle an optional fixed runtime in the `webview2` folder.
- **Linux**: x86_64, WebKitGTK 4.1 и GTK3 из репозитория дистрибутива. «Портативная» означает
  отсутствие установки, а не отсутствие системных библиотек.
- **Linux**: x86_64 with WebKitGTK 4.1 and GTK3 from the distribution repository. "Portable" means
  no installation, not the absence of system libraries.
- **Сборка / Build**: Rust stable 1.85+ (edition 2024), Node.js 20.19+. Тулчейн зафиксирован в
  [rust-toolchain.toml](rust-toolchain.toml).
- **Build**: Rust stable 1.85+ (edition 2024), Node.js 20.19+. The toolchain is pinned in
  [rust-toolchain.toml](rust-toolchain.toml).

Linux build dependencies (Ubuntu 22.04 baseline):

```bash
sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
  libxdo-dev build-essential patchelf
```

## Сборка / Build

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Приложение / Desktop client:

```bash
cd apps/freespace-desktop/ui
npm ci
npm run tauri dev
```

Фронтенд-проверки / Frontend checks: `npm run typecheck`, `npm run lint`, `npm run test:unit`,
`npm run test:e2e`, `npm run test:a11y`. Неинтерактивная проверка выполняется в CI:
[.github/workflows/ci.yml](.github/workflows/ci.yml).

Portable-архивы собираются скриптами в `installer/` и release-workflow; релиз публикуется вручную
из выбранного тега. Порядок проверки описан в
[specs/003-cross-platform-portable/quickstart.md](specs/003-cross-platform-portable/quickstart.md).

## Хранение и офлайн / Storage and offline

Все данные — `settings.toml`, база журнала, кеш хешей, профиль WebView2/WebKit и блокировка CLI —
находятся в папке `data` рядом с исполняемым файлом. `%APPDATA%`, `%LOCALAPPDATA%`,
`%ProgramData%` и временные каталоги не используются. Сетевых вызовов и телеметрии нет.

All data — `settings.toml`, the journal database, the hash cache, the WebView2/WebKit profile and
the CLI lock — lives in the `data` folder next to the executable. `%APPDATA%`, `%LOCALAPPDATA%`,
`%ProgramData%` and temporary directories are never used. There are no network calls and no
telemetry.

## Возможности платформ / Platform capabilities

Точный набор возможностей выводит `freespace capabilities --json`. Windows использует MFT/USN,
Recycle Bin, ACL, атрибуты, ADS, Restart Manager и длинные пути. Linux использует обход дерева,
XDG trash, `statvfs` и `PATH_MAX`; `shred`, `acl`, `attrs`, `unlock` и `context-menu` на Linux
возвращают `E_UNSUPPORTED`. Подробности — в [руководстве пользователя](docs/ru/user-guide.md) и
[контракте CLI](docs/ru/cli.md).

The exact capability set is printed by `freespace capabilities --json`. Windows uses MFT/USN,
Recycle Bin, ACL, attributes, ADS, Restart Manager and long paths. Linux uses the directory walk,
XDG trash, `statvfs` and `PATH_MAX`; `shred`, `acl`, `attrs`, `unlock` and `context-menu` return
`E_UNSUPPORTED` on Linux. See the [user guide](docs/en/user-guide.md) and the
[CLI contract](docs/en/cli.md).

## Правила для AI-агентов / AI agent rules

Правила описаны в [AGENTS.md](AGENTS.md). Rules are described in [AGENTS.md](AGENTS.md).
