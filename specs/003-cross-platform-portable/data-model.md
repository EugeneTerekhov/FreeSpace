# Data Model: FreeSpace 2.0

**Feature**: `003-cross-platform-portable` | **Date**: 2026-11-19

## 1. Изменения верхнего уровня

| Сущность | Было | Стало |
|---|---|---|
| `QuarantineItem` | Карантин с retention, restore, checksum | **Удалена** |
| `DeleteMode` | `RecycleBin`, `Quarantine`, `Permanent` | `RecycleBin`, `Permanent` |
| `DeleteOptions` | `mode`, `reason`, `retention_days`, `available_bytes`, `allow_protected` | `mode`, `reason`, `available_bytes`, `allow_protected` |
| `DeletedItem` | `path`, `size`, `quarantine_id`, `error` | `path`, `size`, `error` |
| `JournalEntryView` | `quarantine_ids`, `reversible` | `reversible` (только rename), без `quarantine_ids` |
| `OperationKind` | `Delete`, `Shred`, `Move`, `Rename`, `Permissions`, `Attributes`, `Restore`, … | Без новых `Restore`; `Shred` остаётся для чтения журнала |
| `Settings` | `quarantine_retention_days` | Поле удалено; старый TOML читается |
| `Capabilities` | — | Новая сущность платформы |

## 2. Platform / Capabilities

```text
Capabilities
├── platform: windows | linux | other
├── scan_methods: { mft: bool, usn: bool, walk: bool }
├── trash: bool
├── permanent: bool
├── shred: bool
├── acl: bool
├── attrs: bool
├── ads: bool
├── unlock: bool
├── context_menu: bool
├── monitor: bool
├── elevation: bool
├── long_paths: bool
├── case_sensitive: bool
└── max_path: Option<usize>
```

Инварианты:

- `permanent` MUST быть `true` на обеих платформах;
- `trash` MUST быть `true` там, где платформа предоставляет системную корзину;
- `shred` MUST быть `true` только на Windows (CLI); в GUI возможность отсутствует;
- `scan_methods.mft`/`usn` MUST быть `false` на Linux;
- `case_sensitive` MUST быть `true` на Linux;
- GUI MUST NOT показывать действие, для которого соответствующая возможность `false`.

## 3. Volume

```text
Volume
├── id: Uuid                     # стабильный идентификатор
├── mount_point: PathBuf         # C:\ или /, /home, /mnt/data
├── label: String
├── file_system: FileSystem
├── total_bytes / free_bytes
├── cluster_size: Option<u32>
├── scan_method: Option<ScanMethod>
├── is_elevated: bool
├── is_removable: bool
└── status: Available | Inaccessible | …
```

Правила:

- `Volume::id_for_mount_point` MUST нормализовать путь по правилам платформы: Windows — диск и
  верхний регистр; Linux — точный путь без завершающего `/` (кроме `/`).
- На Linux `volume_root` для произвольного пути — ближайшая точка монтирования; для файла —
  том, на котором он лежит.
- `ScanMethod` на Linux всегда `DirectoryWalk`.

## 4. Удаление

```text
DeleteRequest
├── targets: Vec<PathBuf>
├── mode: recycle_bin | permanent
├── reason: String
└── confirmed: bool

DeleteOutcome
├── operation_id: Uuid
├── status
├── items: Vec<DeletedItem>
├── summary: ResultSummary
├── journal_id: Option<u64>
└── undoable: bool              # false для удаления; true только для rename
```

Правила:

- `recycle_bin` на Windows — `IFileOperation`; на Linux — XDG trash.
- `permanent` — `fs::remove_file`/`fs::remove_dir_all` с предварительной проверкой scope и
  protected paths.
- При недоступной корзине — ошибка до изменения данных; permanent предлагается отдельно.
- `DeleteMode::Quarantine` отсутствует; `delete --mode quarantine` — ошибка валидации.
- `quarantine_ids` и `retention_days` отсутствуют в API.

## 5. Журнал и миграция SQLite

### 5.1 Схема

`user_version`: 1 → 2.

MIGRATION_1 остаётся неизменной. Добавляется MIGRATION_2:

```sql
DROP INDEX IF EXISTS quarantine_status_idx;
DROP INDEX IF EXISTS quarantine_expiry_idx;
DROP TABLE IF EXISTS quarantine;
```

`SCHEMA_VERSION` = 2.

### 5.2 Совместимость

- Старые записи `journal.details` могут содержать `delete_mode: "quarantine"` и
  `quarantine_ids`; это непрозрачный JSON, чтение не ломается.
- `JournalEntryView.reversible` вычисляется по наличию `details.renames`, а не по
  `undo_available`, чтобы старые записи карантина не показывали нерабочий undo.
- `restore`/`quarantine` в старых записях отображаются как legacy-аудит; новые записи не
  создаются.
- Каталог `data/quarantine` не удаляется автоматически; документация описывает ручную очистку.
- `settings.toml` с `quarantine_retention_days` загружается: ключ неизвестен и игнорируется.

### 5.3 Инварианты

- Каждая успешная операция удаления MUST иметь запись в журнале.
- `undo_available` для удаления MUST быть `false`; для rename — по факту успешного плана.
- Журнал MUST сохранять все старые строки после миграции.

## 6. Настройки

Удаляется:

- `quarantine_retention_days` из `CONFIG_KEYS`, `Settings`, CLI `config`, GUI Settings, DTO.

Остаются без изменений:

- `language`, `theme`, `units`, `duplicate_keep_rule`, `outdated_threshold_days`,
  `confirm_destructive_actions`, `monitor_threshold_bytes`, `monitor_interval_secs`,
  `exclusions`, `context_menu_enabled`.

`context_menu_enabled` на Linux всегда `false` и не отображается (возможность недоступна).

## 7. Portable-артефакты (release-модель)

```text
PortableArtifact
├── platform: windows-x64 | linux-x86_64
├── version: SemVer
├── file_name: String
├── sha256: String
└── contents: [gui, cli, docs, data-readme, optional-webview2]

ReleaseSelection
├── tag: String                 # vX.Y.Z
├── version: SemVer
├── draft: bool
└── artifacts: Vec<PortableArtifact>
```

Инварианты:

- `tag` MUST совпадать с `v<version>`;
- артефактов ровно два (Windows ZIP, Linux tar.gz) + `SHA256SUMS.txt`;
- установщиков среди артефактов нет;
- публикация — ручная.

## 8. DTO-изменения для GUI/CLI

| DTO | Изменение |
|---|---|
| `DeleteMode` | `'recycle_bin' \| 'permanent'` |
| `OperationPreview` | без `quarantine` полей |
| `OperationProgress` | без `quarantine_ids` |
| `JournalEntryView` | без `quarantine_ids`; `reversible` только rename |
| `Settings`/`SettingsPatch` | без `quarantine_retention_days` |
| `Capabilities` | новый DTO в bootstrap и `capabilities --json` |
| `JournalFilter` | `shred` остаётся для старых записей |

## 9. Данные, которые не меняются

- Дерево узлов, размеры, снапшоты (формат и версия), отчёты, хеш-кеш, настройки путей, формат
  `.fssnap`, JSON-конверт CLI.
- Формат журнала (таблица `journal`) и его поля; меняется только состав деталей и трактовка
  обратимости.
