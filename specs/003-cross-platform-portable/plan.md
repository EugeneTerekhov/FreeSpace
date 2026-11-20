# План реализации: FreeSpace 2.0 — кроссплатформенный portable-релиз

**Branch**: `003-cross-platform-portable` | **Date**: 2026-11-19 | **Spec**: [spec.md](./spec.md)

## Summary

FreeSpace 2.0 добавляет Linux x86_64 как вторую целевую платформу, переводит поставку на два
portable-архива (Windows ZIP, Linux tar.gz) и упрощает продукт: карантин удаляется, шредер
остаётся только в Windows CLI, GUI сокращается до 9 экранов, права/атрибуты/пакетное
переименование уходят в CLI. Публикация релиза становится ручной и привязанной к выбранному тегу.

Ключевое архитектурное решение — **общий интерфейс платформы**. `freespace-platform` превращается
в набор узких object-safe трейтов (`PathPolicy`, `Volumes`, `Trash`, `Metadata`, `MonitorBackend`,
`Elevation`, `UnlockService`, `ShredService`, `ContextMenuService`, …), а `windows` и `unix`
реализации выбираются в одной точке сборки. Верхние слои не содержат `cfg(windows)`/`cfg(unix)` и
работают через `Capabilities`.

## Technical Context

**Language/Version**: Rust 1.85+ (edition 2024), TypeScript 5.6+, Node.js 20.19+.

**Target Platforms**:

- Windows 10 21H2+ / Windows 11 x64: MFT/USN, Recycle Bin, ACL, ADS, атрибуты, Restart Manager,
  context menu, long paths, UAC.
- Linux x86_64: walk-сканирование, XDG trash, POSIX-права, символические ссылки и точки
  монтирования. MFT/USN/ACL/ADS/unlock/context menu/shred недоступны.
- macOS, ARM и мобильные платформы — вне области 2.0.

**Primary Dependencies**:

- Без изменений: `tauri` 2, `rusqlite` (bundled), `jwalk`, `rayon`, `serde`, `zip`, `sevenz-rust`,
  `tar`, `flate2`, `kamadak-exif`, `lofty`, `mp4parse`, `pelite`, `csv`, `quick-xml`,
  `rust_xlsxwriter`, `clap`.
- Windows-only (target dependencies): `windows`, `winreg`, `ferrisetw`.
- Linux-only (target dependencies): `rustix` (fs/process: `statvfs`, `geteuid`, `flock`,
  `fsync`), `percent-encoding` (XDG `.trashinfo`). Оба добавляются с обоснованием в
  `research.md`; `libc` напрямую не вводится.
- Tauri Linux build dependencies: `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`,
  `librsvg2-dev`, `libxdo-dev`, `build-essential`, `patchelf`.
- Release automation: GitHub Actions, `softprops/action-gh-release@v2`, `Swatinem/rust-cache@v2`.

**Storage**:

- `data/` рядом с исполняемым файлом: `settings.toml`, `freespace.db`, кеш хешей, профиль
  WebView2/WebKit. `data/quarantine` больше не создаётся; старый каталог не удаляется
  автоматически.
- Миграция SQLite: `user_version` 1 → 2, `DROP TABLE quarantine` (+ индексы). MIGRATION_1
  остаётся неизменной для существующих баз.
- Старые записи журнала с `delete_mode: quarantine` читаются как legacy-аудит; новые не
  создаются.

**Performance Goals**:

- Windows: без изменений — MFT, 8 ТБ ≤ 60 с на потребительском SSD; USN-инкремент.
- Linux: walk, 1 млн файлов на локальном SSD ≤ 5 минут; отклик UI ≤ 200 мс; память ≤ 4 ГБ на
  10 млн узлов; прогресс не реже одного события в секунду.
- Общие: отклик UI ≤ 200 мс, отмена длительных операций, виртуализация дерева/таблиц.

**Constraints**:

- Офлайн и без телеметрии; никаких сетевых вызовов в runtime.
- Все системные вызовы — только в `freespace-platform`; верхние крейты не используют `windows`,
  `winreg`, `ferrisetw`, `rustix` напрямую.
- Разрушающие операции — только через `freespace-ops`: preview, подтверждение, журнал,
  системная корзина / permanent, защищённые пути.
- Длинные пути Windows — verbatim-префиксы; Linux — `PATH_MAX` без иллюзий.
- Portable-поставка: `data` рядом с exe; никаких записей в системные каталоги/реестр.
- Подпись кода отсутствует; SmartScreen-предупреждение документируется.

**Scale/Scope**: 9 экранов, 2 платформы, 2 portable-архива, CLI + GUI, RU/EN, до 10 млн узлов,
один major-релиз 2.0.0.

## 1. Архитектура платформенного слоя

### 1.1 Границы абстракции

| Область | Решение |
|---|---|
| Дерево, агрегация, отчёты, снапшоты, дубликаты, хеши, SQLite/journal, настройки, rename-план | Общий код, без трейтов |
| Пути, тома, allocated size, атрибуты/владелец, корзина, elevation, reveal, monitor, storage hints, instance lock, защищённые пути, command-line quoting | Отдельные реализации за общим трейтом |
| MFT/USN, ETW, context menu, shred, ACL/ADS, long paths, UAC | Windows-only: отдельный трейт с `Unsupported`-реализацией либо `cfg(windows)`-модуль |
| `/proc`, `statvfs`, XDG trash, `geteuid`, `flock` | Unix-only внутри unix-бэкенда |

`cfg` допускается только в:

- `freespace-platform/src/lib.rs` (composition root);
- target-зависимостях `Cargo.toml`;
- Windows-only модулях `freespace-scan` (`mft_scan`, `usn`) и `freespace-ops::shred`;
- Windows-only интеграционных тестах и CLI-справке.

### 1.2 Трейты и композиция

```rust
pub trait PathPolicy: Send + Sync {
    fn normalize(&self, path: &Path) -> Result<PathBuf>;
    fn display(&self, path: &Path) -> String;
    fn validate_length(&self, path: &Path) -> Result<()>;
    fn equivalent_key(&self, path: &Path) -> String;
    fn volume_root(&self, path: &Path) -> Result<PathBuf>;
    fn join_within_limit(&self, parent: &Path, name: &str) -> Result<PathBuf>;
    fn ensure_within(&self, root: &Path, target: &Path) -> Result<()>;
}

pub trait Volumes: Send + Sync {
    fn list(&self) -> Result<Vec<VolumeInfo>>;
    fn describe(&self, path: &Path) -> Result<VolumeInfo>;
}

pub trait Trash: Send + Sync {
    fn trash(&self, paths: &[PathBuf]) -> Result<u32>;
}

pub trait Metadata: Send + Sync {
    fn attributes(&self, path: &Path) -> Result<u32>;
    fn set_attributes(&self, path: &Path, set: u32, clear: u32) -> Result<u32>;
    fn owner(&self, path: &Path) -> Result<Option<String>>;
    fn allocated_size(&self, path: &Path) -> Result<u64>;
    fn allocated_ranges(&self, path: &Path) -> Result<Vec<Range<u64>>>;
    fn streams(&self, path: &Path) -> Result<Vec<StreamInfo>>;
    fn link_kind(&self, path: &Path) -> Result<Option<LinkKind>>;
}

pub trait MonitorBackend: Send + Sync {
    fn snapshot(&self) -> Result<Vec<ProcessIo>>;
    fn counters_for(&self, pid: u32) -> Result<ProcessIo>;
}

pub struct Platform {
    pub paths: &'static dyn PathPolicy,
    pub volumes: &'static dyn Volumes,
    pub trash: &'static dyn Trash,
    pub metadata: &'static dyn Metadata,
    pub monitor: &'static dyn MonitorBackend,
    pub elevation: &'static dyn Elevation,
    pub unlock: &'static dyn UnlockService,
    pub shred: &'static dyn ShredService,
    pub context_menu: &'static dyn ContextMenuService,
    pub storage: &'static dyn StorageHints,
    pub instance_lock: &'static dyn InstanceLock,
    pub capabilities: Capabilities,
}

#[cfg(windows)]
pub fn platform() -> &'static Platform { &windows::PLATFORM }

#[cfg(unix)]
pub fn platform() -> &'static Platform { &unix::PLATFORM }
```

Требования к трейтам:

- object-safe, `Send + Sync`, без generics и без `windows`-типов в сигнатурах;
- возвращают `freespace_core::Result<T>`; платформенные ошибки конвертируются внутри;
- для недоступных возможностей — `Error::Unsupported(...)`, который уже маппится в
  `E_UNSUPPORTED`;
- фасадные функции (`paths::normalize`, `volumes::list_volumes`, `recycle::recycle`) сохраняются
  как тонкие делегаты к `platform()`, чтобы уменьшить diff в первом PR;
- новый код использует `platform()` и `Capabilities` напрямую.

`Capabilities` — сериализуемый DTO:

```rust
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Capabilities {
    pub platform: PlatformId,        // windows | linux | other
    pub scan_methods: ScanMethods,   // mft, usn, walk
    pub trash: bool,
    pub permanent: bool,
    pub shred: bool,
    pub acl: bool,
    pub attrs: bool,
    pub ads: bool,
    pub unlock: bool,
    pub context_menu: bool,
    pub monitor: bool,
    pub elevation: bool,
    pub long_paths: bool,
    pub case_sensitive: bool,
    pub max_path: Option<usize>,
}
```

### 1.3 Структура `freespace-platform`

```text
crates/freespace-platform/src/
├── lib.rs              # трейты, Capabilities, Platform, platform()
├── common/             # общие типы и ошибки, не зависящие от ОС
├── paths.rs            # фасад PathPolicy
├── volumes.rs          # фасад Volumes
├── ...
├── windows/
│   ├── mod.rs          # PLATFORM, Capabilities
│   ├── paths.rs        # verbatim, MAX_PATH
│   ├── volumes.rs      # GetLogicalDrives/GetVolumeInformation
│   ├── trash.rs        # IFileOperation
│   ├── metadata.rs     # security/ads/reparse/allocated
│   ├── monitor.rs      # ETW + process_io
│   ├── unlock.rs       # Restart Manager
│   ├── shred.rs        # overwrite + trim
│   └── ...
└── unix/
    ├── mod.rs
    ├── paths.rs        # POSIX, PATH_MAX, mount-aware volume_root
    ├── volumes.rs      # /proc/self/mounts + statvfs
    ├── trash.rs        # XDG trash
    ├── metadata.rs     # mode/uid/gid, symlink, st_blocks
    ├── monitor.rs      # /proc/<pid>/io
    ├── unsupported.rs  # null-реализации Windows-only трейтов
    └── ...
```

## 2. Удаление карантина

Затрагиваемые слои:

1. **`freespace-ops`**: удалить `quarantine.rs`; из `delete.rs` убрать `Option<&Quarantine>`,
   `quarantine_id`, `retention_days`, ветку режима и `quarantine_ids`; из `undo.rs` — restore и
   `affected_paths`, оставить откат rename. `store.rs`: schema v2, `DROP TABLE quarantine`,
   удалить `quarantine_*`, `read_quarantine_item` и тесты. `operation.rs`: `DeleteMode` без
   `Quarantine`, `OperationKind` без новых `Restore`; старые значения в `details` не ломают
   чтение.
2. **`freespace-app`**: убрать `Quarantine` из `OperationService`/`execute_delete`/`undo`/state,
   `quarantine_ids` из DTO, `quarantine_retention_days` из настроек; `journal_view` считает
   `reversible` только по наличию `renames`.
3. **CLI**: `delete --mode` принимает только `recycle|permanent`; `journal restore` и удаление
   через `undo` убираются; `undo` остаётся для rename.
4. **Tauri**: удалить `operation_restore`, `journal_restore`, `AppState::quarantine`.
5. **UI**: `DeleteObjectDialog` — два режима; `Journal` — без restore и счётчика quarantine;
   `Settings` — без срока хранения; DTO/моки/тесты.
6. **Данные**: старый `data/quarantine` не трогаем; миграция только БД; в user guide — раздел
   «что делать со старым каталогом».

## 3. Shred и экраны

- `freespace-ops::shred` компилируется только на Windows; `freespace-app` и Tauri-команды
  шредера удаляются. CLI-команда `shred` на не-Windows возвращает `E_UNSUPPORTED`; в `--help`
  помечается «Windows only».
- `ScreenId` сокращается до 9 значений; удаляются `Shred.tsx`, `FileOps.tsx`, `Permissions.tsx`,
  `Rename.tsx`; `RenameDialog` остаётся для контекстного меню дерева.
- Legacy-алиасы: `shred` → `journal`, `fileops`/`rename`/`permissions` → `tree`.
- Tauri: удаляются `shred_*`, `permissions_*`, `attributes_*`; `rename_preview`/`rename_apply`
  остаются.
- i18n-ключи удалённых экранов убираются, каталоги перегенерируются; палитра команд, hotkeys,
  моки и e2e обновляются.

## 4. Portable-поставка

### 4.1 Windows

- `installer/portable.ps1`: убрать хардкод версии, читать её из `Cargo.toml`; при отсутствии
  `ui/dist` вызывать `npm run build`; копировать `docs/ru/user-guide.md` и
  `docs/en/user-guide.md` (сейчас скрипт ссылается на несуществующий `docs/user-guide.md`);
  выход — `dist/freespace-portable-<ver>-windows-x64.zip`.
- Удалить NSIS-секцию из `tauri.conf.json`, `installer/nsis/hooks.nsh` и упоминания установщика
  из документации; `bundle.targets` больше не используются, сборка идёт с `--no-bundle`.
- WebView2: архив работает с Evergreen Runtime; опциональный fixed runtime — по флагу, как
  сейчас.

### 4.2 Linux

- `installer/portable.sh`: сборка `npm run build`, `cargo build --release --no-default-features
  -p freespace-cli -p freespace-desktop`, staging `freespace-gui`, `freespace`, docs, `data/`,
  `run.sh`; выход — `dist/freespace-portable-<ver>-linux-x86_64.tar.gz`.
- tar.gz, не AppImage: `data` должна лежать рядом с исполняемым файлом, а AppImage read-only.
- Сборка на Ubuntu 22.04; runtime-зависимости WebKitGTK 4.1/GTK3 документируются в README
  архива и user guide.
- `.desktop`-файл не устанавливается (portable); может лежать в архиве как пример.

## 5. CI и релиз

### 5.1 CI

- Windows-джоб: без изменений (fmt/clippy/test, frontend, e2e).
- Новый Linux-джоб (`ubuntu-22.04`): установка GTK/WebKit-зависимостей, `cargo fmt --check`
  (один раз), `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `cargo check --no-default-features`.
- Windows-only тесты (`scan_mft`, `unlock`, `elevation`, `permissions`, `context_menu`,
  `long_paths`) помечаются `#![cfg(windows)]`; Linux-специфичные тесты — `#![cfg(unix)]`.
- Добавить контрактный тест `Capabilities` для обоих бэкендов и тесты XDG trash на временных
  каталогах.
- Не добавлять `cargo test` для GUI на Linux (Tauri требует дисплей); достаточно
  `cargo check -p freespace-desktop`.

### 5.2 Release workflow

`.github/workflows/release.yml`:

```yaml
on:
  workflow_dispatch:
    inputs:
      tag:
        description: Existing tag (vX.Y.Z)
        required: true
      draft:
        description: Create a draft release
        type: boolean
        default: true
permissions:
  contents: write
```

- checkout указанного тега, `fetch-depth: 0`;
- проверка `tag == v<workspace version>` и существования тега на remote;
- matrix `windows-latest` / `ubuntu-22.04`;
- сборка portable-архивов и `SHA256SUMS.txt`;
- финальный job скачивает артефакты и через `softprops/action-gh-release@v2` создаёт/обновляет
  **draft** с `tag_name`, `files`, `body` из CHANGELOG; публикация — вручную;
- обычный `push` тега workflow не запускает;
- при падении любого matrix-джоба черновик не создаётся/не обновляется.

### 5.3 Версионирование

- Источник истины — `[workspace.package].version` в `Cargo.toml`.
- `scripts/version.mjs`:
  - `set <semver>` — обновляет `Cargo.toml`, `tauri.conf.json` (если поле остаётся),
    `ui/package.json` + lock через `npm version --no-git-tag-version`;
  - `check` — сверяет все версии и тег, возвращает ненулевой код при расхождении.
- `installer/portable.ps1`/`portable.sh` читают версию из `Cargo.toml`, без хардкода.
- README и user guide не содержат версии в тексте либо обновляются скриптом.
- Релиз 2.0.0: CHANGELOG `[Unreleased]` → `[2.0.0]`, разделы `Added/Changed/Removed/Fixed`.

## 6. Project Structure

```text
crates/freespace-platform/
├── src/lib.rs                  # трейты, Capabilities, Platform
├── src/common/
├── src/windows/
└── src/unix/

crates/freespace-ops/
├── src/delete.rs               # recycle_bin | permanent
├── src/undo.rs                 # rename only
└── src/store.rs                # schema v2

crates/freespace-scan/
├── src/walk.rs                 # общий
├── src/mft_scan.rs             # cfg(windows)
└── src/usn.rs                  # cfg(windows)

apps/freespace-desktop/
├── src-tauri/src/commands/     # без shred/permissions/attrs
└── ui/src/
    ├── app/routing.ts          # 9 экранов
    └── screens/                # без Shred/FileOps/Permissions/Rename

installer/
├── portable.ps1                # Windows ZIP
└── portable.sh                 # Linux tar.gz

.github/workflows/
├── ci.yml                      # windows + ubuntu
└── release.yml                 # workflow_dispatch + draft

scripts/
└── version.mjs                 # set/check
```

## 7. Constitution Check

| Принцип | Соответствие | Обоснование |
|---------|--------------|-------------|
| I. Безопасность разрушающих операций | PASS | Остаются preview, подтверждение, журнал, системная корзина и protected paths. Карантин удаляется, но восстановление обеспечивает ОС. Undo сохранён для rename. |
| II. Точность учёта места | PASS | Linux: `st_blocks`, sparse; Windows: существующие механизмы. Отчёты указывают тип размера. |
| III. Производительность | PASS | Windows MFT/USN без изменений; Linux walk с отдельным бюджетом 1 млн файлов ≤ 5 мин; UI ≤ 200 мс. |
| IV. Нативная точность платформ | PASS | WinAPI и POSIX за общим интерфейсом; недоступные возможности — `Capabilities`/`E_UNSUPPORTED`, без эмуляции. |
| V. Тест-первый подход | PASS | Контрактные тесты платформы, XDG trash, миграции БД; Windows-only тесты гейтятся, Linux-тесты добавляются. |
| VI. Паритет CLI и GUI | PASS | Каждая возможность GUI есть в CLI; CLI предоставляет больше (shred, ACL, attrs, bulk rename). |
| VII. Приватность и офлайн | PASS | Runtime без сети; portable-архивы и сборка не добавляют телеметрии. |
| Tech constraints | PASS | Portable-only поставка; Tauri сохранён (ADR 0001); системные вызовы только в `freespace-platform`. |

## 8. Delivery Phases

| Этап | Содержание | Результат |
|------|------------|-----------|
| 0. SDD | Конституция 2.0.0, spec/plan/research/data-model/contracts/tasks, docs RU/EN | Согласованная рамка 2.0 |
| 1. Платформенный интерфейс | Трейты, `Capabilities`, `Platform`, Windows-адаптер, фасады | Поведение Windows не меняется |
| 2. Удаление карантина | ops/store/app/CLI/Tauri/UI, миграция schema v2 | Карантина нет, тесты зелёные |
| 3. Shred и экраны | Windows-only shred, 9 экранов, CLI-only ACL/attrs/rename screen | GUI упрощён, CLI сохранён |
| 4. Linux backend | paths/volumes/metadata/trash/monitor/reveal/lock + walk-скан | Linux CLI/GUI работают |
| 5. Portable-поставка | Windows ZIP, Linux tar.gz, скрипты, docs | Два артефакта |
| 6. CI и релиз | ubuntu-джоб, `release.yml`, `scripts/version.mjs` | Ручной draft-релиз по тегу |
| 7. Приёмка | Windows 10/11 + Ubuntu 22.04/24.04, миграция, артефакты | 2.0.0 готов |

## 9. Complexity Tracking

| Решение | Зачем | Отклонённая альтернатива |
|---------|-------|--------------------------|
| Трейты по возможностям + единая точка сборки | Убирает `cfg` из бизнес-логики, делает платформы заменяемыми и тестируемыми | `cfg` по месту — быстро, но расползается и ломает паритет |
| `Capabilities` в GUI/CLI | Честное поведение вместо «серых» кнопок и невнятных ошибок | Прятать ошибки за общим «not implemented» — плохой UX |
| tar.gz вместо AppImage | `data` рядом с exe; AppImage read-only | AppImage потребовал бы менять модель хранения |
| XDG trash, а не карантин на Linux | Стандарт, восстановление системными средствами, не тратит место на очищаемом томе | Карантин на Linux дублирует корзину и занимает место |
| Shred только Windows CLI | На SSD/CoW гарантий нет; GUI создаёт ложное чувство безопасности | Оставить GUI-шредер — вводит в заблуждение |
| Ручной `workflow_dispatch` | Не каждый тег — релиз; публикация остаётся человеку | Авторелиз по тегу создаёт мусорные релизы |
| Ubuntu 22.04 baseline | Совместимость glibc/WebKitGTK | Сборка на новом образе поднимает минимальную glibc |

## 10. Risks and Mitigations

| Риск | Мера |
|------|------|
| Linux walk не укладывается в бюджет | Профилировать `jwalk`, исключения, отключать лишние метаданные; бюджет 1 млн файлов ≤ 5 мин; не обещать 8 ТБ/60 с |
| XDG trash недоступна на части томов | Ошибка + предложение permanent с отдельным подтверждением; тесты на read-only/сетевом mount |
| Регистрозависимые ФС ломают rename/duplicates | `PathPolicy::equivalent_key`; тесты `Report.txt`/`report.txt` |
| `PATH_MAX` и длинные пути | Проверка до операции; тесты на границе |
| Миграция БД теряет старые записи | MIGRATION_1 неизменна, MIGRATION_2 только drop; тест чтения legacy-журнала |
| Удаление карантина ломает сохранённые настройки | serde `default` без `deny_unknown_fields`; ключ игнорируется; тест старого TOML |
| Несовпадение версий в релизе | `scripts/version.mjs check` + проверка тега в workflow |
| Частичный релиз при падении matrix | Финальный job зависит от обоих; черновик создаётся только при успехе |
| Tauri на Linux требует системных библиотек | Ubuntu 22.04 + явный список пакетов; понятная ошибка запуска |
| Неподписанные Windows-сборки | Документировать SmartScreen; не обещать «тихую» установку |
| Отсутствие GUI-тестов на Linux | `cargo check` + ручной smoke по quickstart; Playwright остаётся Windows-only |
| Долгая сборка/размер архивов | Кеш cargo/npm; `strip`; WebView2 fixed runtime — по флагу |

## 11. Post-Design Constitution Re-Check

После этапов 1–5 проверяется:

- в `freespace-ops/app/cli/tauri` нет `cfg(windows)`/`cfg(unix)` в бизнес-логике;
- `Capabilities` совпадает с фактическим поведением на Windows и Linux;
- удаление всегда проходит preview → confirm → journal; карантина нет;
- системная корзина и permanent работают, restore — средствами ОС;
- 9 экранов, legacy-переходы не падают;
- старый журнал и настройки читаются после миграции;
- portable-архивы не пишут вне `data`;
- release-workflow не запускается на push тега и создаёт только draft;
- версии совпадают; артефакты неподписаны и задокументированы.
