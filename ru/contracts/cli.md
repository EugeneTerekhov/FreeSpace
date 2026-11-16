# Контракт: командная строка FreeSpace

**Feature**: `001-freespace-disk-analyzer` | **Version**: 1 | **Date**: 2026-11-15

CLI использует то же ядро, что и GUI, и обязан обеспечивать паритет возможностей (FR-074…FR-078).

## Общие правила

- Формат: `freespace <command> [subcommand] [arguments] [options]`.
- Глобальные опции: `--json`, `--quiet`, `--yes`, `--no-color`, `--lang <ru|en>`, `--config <path>`,
  `--log-level <error|warn|info|debug>`, `--lock-timeout <секунды>`.
- `--json` переключает вывод в машиночитаемый конверт (см. ниже); человекочитаемый вывод
  направляется в stdout, ошибки — в stderr.
- Разрушающие команды в неинтерактивном режиме MUST требовать `--yes`; без него команда
  завершается с кодом `5` и ничего не изменяет.
- Пути принимаются обычные и длинные (verbatim); оболочка передаёт их как есть.
- Все команды идемпотентны в части чтения; разрушающие операции не повторяются автоматически.

## JSON-конверт

```json
{
  "version": "1",
  "command": "scan",
  "status": "ok | error | partial | cancelled",
  "data": {},
  "warnings": [],
  "errors": [],
  "duration_ms": 1234
}
```

- `data` — результат команды; схема зависит от команды.
- `warnings` — нефатальные проблемы (например, недоступные каталоги).
- `errors` — фатальные и частичные ошибки с полями `code`, `message`, `path`.
- JSON MUST быть валидным UTF-8 и не содержать служебных сообщений в stdout.

## Команды

### `scan`

```text
freespace scan <path> [--method auto|mft|walk] [--incremental] [--exclude <glob>]...
                      [--snapshot-out <file>] [--json]
```

- `data`: `{ "session_id", "volume", "nodes_count", "total": SizeMetrics, "roots": [...] }`.
- `--incremental` применяется только к NTFS при наличии сохранённого состояния журнала.

### `tree`

```text
freespace tree <path> [--depth <n>] [--min-size <bytes>] [--json]
```

- `data`: иерархический список узлов `{name, path, kind, size{...}, children[]}`.

### `top`

```text
freespace top <path> --limit <n> [--json]
```

- `data`: `{ "items": [{ "path", "size", "modified" }] }`.

### `types`

```text
freespace types <path> [--json]
```

- `data`: `{ "items": [{ "extension", "category", "count", "total_size", "share" }] }`.

### `filter`

```text
freespace filter <path> [--kind <file|dir|link>] [--min-size <bytes>] [--older-than <days>]
                       [--type <ext,...>] [--min-resolution <WxH>] [--min-bitrate <kbps>]
                       [--min-duration <seconds>] [--metadata exif,audio,video,pe]
                       [--owner <account|unknown>] [--json]
```

- `data`: `{ "items": [{ "path", "size", "metadata": {...}, "owner": "..." }] }`.
- `--owner` принимает имя учётной записи, SID или `unknown`; неизвестный владелец
  возвращает пустой список с `matched_owner = false`.
- Метаданные, которые не удалось прочитать, отображаются как `unsupported` или `corrupted` и не
  прерывают команду.

### `duplicates`

```text
freespace duplicates <path> [--min-size <bytes>] [--algorithm sha256|md5]
                              [--keep-rule oldest|newest|shortest|original-dir]
                              [--json]
```

- `data`: `{ "groups": [{ "id", "hash", "size", "reclaimable_bytes", "members": [...] }] }`.
- Поиск не выполняет удаление; для удаления используется `delete`.

### `cleanup`

```text
freespace cleanup suggest <path> [--older-than <days>] [--json]
freespace cleanup empty-dirs <path> [--json]
```

- `data`: `{ "candidates": [{ "path", "category", "reclaimable_bytes", "risk" }] }`.

### `unlock`

```text
freespace unlock <path> [--list] [--soft] [--force] [--yes] [--json]
```

- Без `--soft/--force` работает как `--list`.
- `--force` требует `--yes`; завершает процесс после подтверждения.

### `delete`

```text
freespace delete <path>... --mode recycle|quarantine|permanent [--yes] [--json]
```

- `--mode` обязателен (решение пользователя из UI/CLI).
- Для `quarantine` при нехватке места — ошибка `E_INSUFFICIENT_SPACE` без частичного выполнения.

### `shred`

```text
freespace shred <path>... --passes <n> --pattern zeros|random [--yes] [--json]
```

- `--passes ≥ 1`; на SSD добавляется предупреждение в `warnings` и выполняется TRIM.

### `rename`

```text
freespace rename <path>... --regex <pattern> --replacement <text>
                           [--case lower|upper|preserve] [--preview] [--yes] [--json]
```

- Без `--yes` выполняется только предпросмотр (`status = ok`, `data.preview = true`).
- При конфликтах — `E_CONFLICT`, ничего не изменяется.

### `attrs`

```text
freespace attrs <path>... --set read-only,hidden --clear archive [--yes] [--json]
```

### `acl`

```text
freespace acl <path>... --owner <account> | --reset | --copy-from <path> [--yes] [--json]
```

### `owners`

```text
freespace owners <path> [--json]
```

- `data`: `{ "items": [{ "owner_id", "sid", "display_name", "total_bytes", "file_count", "share" }],
  "count", "total_bytes", "unknown_count", "error_count" }`.
- Объекты без определимого владельца попадают в категорию `unknown`.
- Отчёт использует те же строки, что GUI и раздел `owners` экспорта.

### `snapshot`

```text
freespace snapshot create <path> --out <file.fssnap> [--json]
freespace snapshot diff <base.fssnap> <target.fssnap> [--export <file>] [--json]
```

### `export`

```text
freespace export <path> --format csv|xml|html|xlsx --out <file> [--include tree,top,types,duplicates,owners] [--json]
```

### `archives`

```text
freespace archives <path> [--export <file.csv|json|xml|html>] [--json]
```

- `data`: `{ "items": [{ "archive", "format", "entries": [{ "path", "size", "compressed_size" }] }] }`.
- Поддерживаются ZIP, 7z, TAR, TAR/GZ и ISO; RAR — только при сборке с функцией `rar`.
- `--export` выгружает список содержимого в CSV, JSON, XML или HTML.

### `monitor`

```text
freespace monitor [--threshold <bytes>] [--interval <seconds>] [--iterations <n>]
                  [--root <path>] [--no-etw] [--json]
```

- Потоковый JSON Lines: по одному объекту `DiskEvent` на строку.
- `--iterations 0` работает до прерывания; `--no-etw` включает только резервный режим счётчиков.

### `drives`

```text
freespace drives [--json]
```

### `config`

```text
freespace config get <key> [--json]
freespace config set <key> <value> [--json]
```

### `context-menu`

```text
freespace context-menu install [--machine] [--executable <path>] [--yes]
freespace context-menu uninstall [--machine] [--yes]
```

### `schedule`

```text
freespace schedule export <profile> --out <file.xml>
```

- Создаёт XML-задание для Планировщика заданий Windows (UTF-16LE с BOM).
- Встроенные профили: `daily-scan`, `weekly-duplicates`, `monthly-export`.

## Параллельные запуски

Все команды берут эксклюзивную блокировку `%LOCALAPPDATA%\FreeSpace\cli.lock`.
Второй процесс ждёт `--lock-timeout` секунд (по умолчанию 30) и затем завершается с кодом `10`
(`E_LOCKED`). Блокировка снимается при завершении процесса, в том числе аварийном.

## Коды возврата

| Код | Значение |
|-----|----------|
| 0 | Успех |
| 1 | Общая ошибка |
| 2 | Неверные аргументы |
| 3 | Отказано в доступе |
| 4 | Объект не найден |
| 5 | Операция отменена пользователем или отсутствует подтверждение |
| 6 | Обнаружены конфликты (имена, права, снимки) |
| 7 | Частичный успех (часть объектов обработана) |
| 8 | Неподдерживаемый формат или возможность |
| 9 | Недостаточно свободного места |
| 10 | Объект занят другим процессом |

## Стабильность

- Схемы JSON и коды возврата стабильны в пределах мажорной версии.
- Новые поля могут добавляться без изменения мажорной версии; потребители обязаны игнорировать
  неизвестные поля.
- Удаление или переименование команд — только в мажорной версии.
