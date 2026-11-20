# Контракт CLI FreeSpace

**Версия контракта**: 1  
**Относится к**: FreeSpace 2.0.x  
**Источник истины**: `specs/001-freespace-disk-analyzer/contracts/cli.md` и
`specs/003-cross-platform-portable/contracts/`

CLI использует те же основные крейты, что и GUI. У каждой возможности GUI есть аналог в CLI с
машиночитаемым выводом; CLI может предоставлять больше возможностей, чем GUI. Интерфейс полностью
офлайн и никогда не отправляет телеметрию.

## Вызов

```text
freespace <команда> [подкоманда] [аргументы] [опции]
```

Глобальные опции:

| Опция | Значение |
|-------|----------|
| `--json` | Вывести JSON-конверт вместо человекочитаемого текста |
| `--quiet` | Подавить человекочитаемый вывод |
| `--yes` | Подтвердить разрушающую операцию неинтерактивно |
| `--no-color` | Отключить ANSI-цвета |
| `--lang <ru\|en>` | Язык интерфейса |
| `--config <path>` | Использовать явный файл настроек |
| `--log-level <error\|warn\|info\|debug>` | Уровень диагностики |
| `--lock-timeout <секунды>` | Ожидание другого процесса FreeSpace (по умолчанию 30) |

Человекочитаемый текст выводится в stdout, предупреждения и ошибки — в stderr. С `--json` stdout
содержит ровно один корректный UTF-8 JSON-документ, кроме `monitor`, который потоково печатает
JSON Lines, как описано ниже.

## JSON-конверт

```json
{
  "version": "1",
  "command": "scan",
  "status": "ok",
  "data": {},
  "warnings": [],
  "errors": [],
  "duration_ms": 1234
}
```

`status` принимает значения `ok`, `error`, `partial` или `cancelled`. Каждая ошибка содержит
стабильный `code` (`E_*`), `message` и, когда применимо, `path`. Новые поля могут появляться в
пределах мажорной версии; потребители обязаны игнорировать неизвестные поля.

## Коды возврата

| Код | Значение |
|-----|----------|
| 0 | Успех |
| 1 | Общий сбой (I/O, конфигурация, повреждённый снимок) |
| 2 | Неверный аргумент или путь |
| 3 | Доступ запрещён или защищённый системный путь |
| 4 | Объект не найден |
| 5 | Нет подтверждения или операция отменена |
| 6 | Конфликт (имена, права, снимки) |
| 7 | Частичный успех |
| 8 | Неподдерживаемый формат или возможность (`E_UNSUPPORTED`) |
| 9 | Недостаточно свободного места |
| 10 | Объект или запуск заблокированы другим процессом |

Разрушающие команды требуют `--yes` в неинтерактивном режиме. Без него команда завершается с
кодом `5` и ничего не меняет.

## Команды

### `drives`

```text
freespace drives [--json]
```

`data`: `{ "items": [{ "mount_point", "label", "file_system", "total_bytes", "free_bytes", "used_bytes", "cluster_size", "serial_number", "drive_type", "is_removable", "status" }] }`.

На Windows тома перечисляются через WinAPI, на Linux — через таблицу монтирования и `statvfs`.
Недоступные тома попадают в вывод со `"status": "inaccessible"`, а не отбрасываются.

### `capabilities`

```text
freespace capabilities [--json]
```

Описывает возможности текущей платформы. `data` — объект `Capabilities`:

```json
{
  "platform": "linux",
  "scan_methods": { "mft": false, "usn": false, "walk": true },
  "trash": true,
  "permanent": true,
  "shred": false,
  "acl": false,
  "attrs": false,
  "ads": false,
  "unlock": false,
  "context_menu": false,
  "monitor": true,
  "elevation": true,
  "long_paths": false,
  "case_sensitive": true,
  "max_path": 4096
}
```

| Поле | Значение |
|------|----------|
| `platform` | `windows`, `linux` или `other` |
| `scan_methods` | Доступные методы: `mft`, `usn`, `walk` |
| `trash` | Системная корзина: Recycle Bin или XDG trash |
| `permanent` | Безвозвратное удаление |
| `shred` | Затирание, только Windows CLI |
| `acl` | Владелец и права NTFS |
| `attrs` | Атрибуты Windows |
| `ads` | Альтернативные потоки NTFS |
| `unlock` | Снятие блокировок Restart Manager |
| `context_menu` | Интеграция с контекстным меню Проводника |
| `monitor` | Мониторинг записи |
| `elevation` | Определение и запрос повышения прав |
| `long_paths` | Поддержка длинных путей |
| `case_sensitive` | Имена файлов учитывают регистр |
| `max_path` | Лимит длины пути в символах или `null` |

Windows сообщает `mft: true`, `usn: true`, `shred: true`, `acl: true`, `attrs: true`,
`ads: true`, `unlock: true`, `long_paths: true`, `case_sensitive: false`, `max_path: null`.
Linux сообщает только `walk`, `trash`, `permanent`, `monitor`, `elevation`, `case_sensitive: true`
и `max_path: 4096`; остальные возможности — `false`. Неподдерживаемая команда возвращает
`E_UNSUPPORTED` и код возврата `8`.

### `config`

```text
freespace config get <key> [--json]
freespace config set <key> <value> [--json]
```

Поддерживаемые ключи: `language`, `theme`, `units`, `duplicate_keep_rule`,
`outdated_threshold_days`, `confirm_destructive_actions`, `monitor_threshold_bytes`,
`monitor_interval_secs`, `exclusions`, `context_menu_enabled`. Устаревший ключ
`quarantine_retention_days` игнорируется, если старый `settings.toml` всё ещё его содержит.

`context_menu_enabled` по умолчанию `false`. Команда `config set context_menu_enabled true`
сразу регистрирует пункты Проводника, `false` — удаляет их. В портативной сборке
(`--no-default-features`) ключ остаётся читаемым, но включить его нельзя, а команды
`context-menu` нет.

### `scan`

```text
freespace scan <path> [--method auto|mft|walk] [--incremental]
                      [--exclude <glob>]... [--snapshot-out <file>]
                      [--include-ads] [--size-mode logical|on-disk|physical]
                      [--json]
```

`data`: `{ "session_id", "volume", "nodes_count", "total": SizeMetrics, "roots": [...] }`.

На Windows доступны `auto`, `mft` и `walk`, а `--incremental` обновляет предыдущий скан NTFS через
журнал USN. На Linux используется только обход дерева; `--method mft` и `--incremental`
возвращают `E_UNSUPPORTED`, а метод сканирования явно указывается в результате.

### `tree`

```text
freespace tree <path> [--depth <n>] [--min-size <bytes>] [--json]
```

`data`: иерархические узлы `{ "name", "path", "kind", "size": SizeMetrics, "children": [] }`.

### `top`

```text
freespace top <path> --limit <n> [--json]
```

`data`: `{ "items": [{ "path", "size", "modified" }] }`.

### `types`

```text
freespace types <path> [--json]
```

`data`: `{ "items": [{ "extension", "category", "count", "total_size", "share" }] }`.

### `filter`

```text
freespace filter <path> [--kind file|dir|link] [--min-size <bytes>]
                       [--older-than <days>] [--type <ext,...>]
                       [--owner <account|unknown>]
                       [--min-resolution <WxH>] [--min-bitrate <kbps>]
                       [--min-duration <seconds>]
                       [--metadata exif,audio,video,pe] [--json]
```

`data`:

```json
{
  "items": [{
    "path": "C:\\Videos\\clip.mp4",
    "kind": "file",
    "size": 123456,
    "modified": "2026-11-16T10:00:00Z",
    "owner": "PC\\Alice",
    "metadata": {
      "kind": "video",
      "status": "ok",
      "tags": { "Width": "3840", "Height": "2160", "Codec": "h264" },
      "read_at": 1763290000000
    }
  }],
  "count": 1
}
```

`--owner` принимает имя учётной записи, SID или `unknown`/`неизвестно`. Команда завершается
успешно с `count: 0` и `matched_owner: false`, если учётной записи нет. Ограничения по метаданным
исключают файлы с неподдерживаемыми или повреждёнными метаданными; без опций метаданных
возвращаются все подходящие объекты.

### `duplicates`

```text
freespace duplicates <path> [--min-size <bytes>] [--algorithm sha256|md5]
                              [--keep-rule oldest|newest|shortest|original-dir]
                              [--json]
```

`data`: `{ "groups": [{ "id", "hash", "size", "reclaimable_bytes", "members": [...] }] }`.
Поиск никогда ничего не удаляет.

### `cleanup`

```text
freespace cleanup suggest <path> [--older-than <days>] [--json]
freespace cleanup empty-dirs <path> [--json]
```

`data`: `{ "candidates": [{ "path", "category", "reclaimable_bytes", "risk" }] }`.

### `unlock`

```text
freespace unlock <path> [--list] [--soft] [--force] [--yes] [--json]
```

Без `--soft`/`--force` команда перечисляет процессы, удерживающие файл. `--force` требует
`--yes`. Команда доступна только на Windows (Restart Manager); на Linux возвращает
`E_UNSUPPORTED`.

### `delete`

```text
freespace delete <path>... --mode recycle|permanent [--yes] [--json]
```

`--mode` обязателен. Допустимы ровно два режима:

- `recycle` — системная корзина: Recycle Bin на Windows, XDG trash на Linux;
- `permanent` — безвозвратное удаление.

Удалённое значение `quarantine` завершается ошибкой `E_INVALID_ARGUMENT` и никогда не трактуется
как `permanent`. Если корзина на томе недоступна, команда возвращает ошибку до изменения данных;
permanent предлагается отдельно и требует собственного подтверждения. Любое удаление проходит
предпросмотр, подтверждение и запись в журнал.

### `shred`

```text
freespace shred <path>... --passes <n> --pattern zeros|random [--yes] [--json]
```

Команда доступна только на Windows; на Linux и других платформах возвращает `E_UNSUPPORTED`
(код `8`). На SSD в результате появляется предупреждение и запрашивается TRIM.

### `rename`

```text
freespace rename <path>... --regex <pattern> --replacement <text>
                           [--case lower|upper|preserve] [--preview] [--yes]
                           [--json]
```

Без `--yes` выполняется только предпросмотр. Конфликты возвращают `E_CONFLICT` и ничего не
меняют. Пакетное переименование доступно только в CLI; контекстное переименование одного файла
остаётся в дереве GUI.

### `attrs` и `acl`

```text
freespace attrs <path>... --set read-only,hidden --clear archive [--yes] [--json]
freespace acl <path>... --owner <account> | --reset | --copy-from <path> [--yes] [--json]
```

Обе команды работают только на Windows. На Linux они возвращают `E_UNSUPPORTED`; права POSIX
через CLI не изменяются.

### `owners`

```text
freespace owners <path> [--json]
```

Строит отчёт по владельцам. `data`:

```json
{
  "items": [{
    "owner_id": 0,
    "sid": "S-1-5-21-…",
    "display_name": "PC\\Alice",
    "total_bytes": 123456,
    "file_count": 42,
    "share": 0.31
  }],
  "count": 1,
  "total_bytes": 123456,
  "unknown_count": 0,
  "error_count": 0
}
```

Объекты, владельца которых определить не удалось, попадают в `unknown`. На Linux владелец
определяется по `uid`/`gid` и `/etc/passwd`.

### `snapshot`

```text
freespace snapshot create <path> --out <file.fssnap> [--json]
freespace snapshot diff <base.fssnap> <target.fssnap> [--export <file>] [--json]
```

### `export`

```text
freespace export <path> --format csv|xml|html|xlsx --out <file>
                        [--include tree,top,types,duplicates,owners] [--json]
```

`data`: `{ "path", "format", "sections", "row_count", "total_bytes" }`.

Все форматы строятся из одной модели отчёта, поэтому заголовки, порядок и итоги совпадают.
CSV — UTF-8 с BOM и блоком `# section: <name>` на каждую секцию; XML следует задокументированным
именам элементов; HTML самодостаточен и экранирует пользовательский текст; XLSX содержит листы
`Summary`, `Tree`, `TopFiles`, `Types`, `Duplicates` и `Owners`. См.
`specs/001-freespace-disk-analyzer/contracts/report-formats.md`.

### `archives`

```text
freespace archives <path> [--export <file.csv|json|xml|html>] [--json]
```

`path` может быть одним архивом или каталогом. Поддерживаемые контейнеры: ZIP, 7z, TAR, TAR/GZ и
ISO. RAR требует необязательной функции сборки `rar` и совместимой библиотеки; иначе команда
возвращает `E_UNSUPPORTED` без падения. Защищённые паролем и повреждённые архивы дают понятную
ошибку.

`data`:

```json
{
  "items": [{
    "archive": "C:\\Downloads\\archive.zip",
    "format": "zip",
    "total_size": 12345,
    "file_count": 2,
    "encrypted_count": 0,
    "entries": [{
      "path": "docs/readme.txt",
      "size": 120,
      "compressed_size": 80,
      "is_directory": false,
      "encrypted": false,
      "share": 0.01
    }]
  }],
  "count": 1
}
```

Списки архивов экспортируются в CSV, JSON, XML и HTML. Ограничения ресурсов отклоняют
архивные бомбы с `E_UNSUPPORTED`.

### `schedule`

```text
freespace schedule export <profile> --out <file.xml> [--json]
```

Встроенные профили: `daily-scan`, `weekly-duplicates`, `monthly-export`. Вывод — определение
Task Scheduler в UTF-16LE с BOM:

```powershell
freespace schedule export daily-scan --out daily-scan.xml
schtasks /create /tn "FreeSpace daily scan" /xml daily-scan.xml /f
```

Профили проверяются перед экспортом. Неизвестный профиль возвращает `E_NOT_FOUND`. Команда
предназначена для Windows.

### `monitor`

```text
freespace monitor [--threshold <bytes>] [--interval <seconds>]
                  [--iterations <n>] [--root <path>] [--no-etw] [--json]
```

Потоково печатает JSON Lines вместо обычного конверта. Каждая строка — один объект:

```json
{"type":"write","timestamp_ms":1700000000000,"pid":1234,"process":"backup.exe","bytes":8388608,"paths":["\\Device\\HarddiskVolume3\\data\\video.mkv"],"threshold_exceeded":false}
{"type":"notification","timestamp_ms":1700000000000,"bytes":2147483648,"window_ms":60000,"threshold_bytes":1073741824}
{"type":"usn","timestamp_ms":1700000000000,"file_ref":42,"parent_ref":5,"name":"changed.bin"}
```

На Windows сессия ETW Kernel-File (процесс + путь) требует прав администратора. Без них команда
переходит на `GetProcessIoCounters` (процесс + байты) и хвост журнала USN (подсказки путей). На
Linux используются счётчики `/proc/<pid>/io`. `--iterations 0` работает до прерывания;
`--interval 0` опрашивает максимально часто (100 мс).

### `context-menu`

```text
freespace context-menu install [--machine] [--executable <path>]
freespace context-menu uninstall [--machine] [--all]
```

Устанавливает пункты *Analyze with FreeSpace* для файлов, папок и дисков, а также *Unlock with
FreeSpace* для файлов и папок. По умолчанию пунктов нет: ни распаковка архива, ни свежая
конфигурация их не регистрируют. Установка или удаление для текущего пользователя также обновляет
`context_menu_enabled`, поэтому экран настроек GUI остаётся синхронизированным. Область по
умолчанию — текущий пользователь (`HKCU\Software\Classes`); `--machine` пишет в `HKLM`, требует
повышения прав и не меняет пользовательскую настройку; `--all` удаляет обе области и отключает
настройку. Очистка идемпотентна.

Пути с пробелами, Unicode и завершающими разделителями корректно экранируются для
`CommandLineToArgvW`. Голый путь, переданный исполняемому файлу, разворачивается в `scan <path>`.
Команда доступна только на Windows; в портативной сборке (`--no-default-features`) она
отсутствует.

## Журнал и отмена

Журнал операций доступен в GUI. Отмена поддерживается только для переименования
(`operation_undo` / `journal_undo`); удаление из приложения не отменяется и не восстанавливается —
объекты из системной корзины восстанавливаются средствами ОС. Старые записи журнала с типами
`quarantine` и `restore` читаются как аудит и не создают новых операций.

## Параллельные запуски

Каждая команда берёт эксклюзивную рекомендательную блокировку на `<exe_dir>/data/cli.lock`. Второй
процесс ждёт `--lock-timeout` секунд, затем завершается с `E_LOCKED` (код `10`). Блокировка
снимается автоматически при выходе процесса, в том числе после сбоя. Команды только для чтения
тоже сериализуются, чтобы JSON-логи и кеш оставались согласованными.

## Хранение данных

Настройки, SQLite-журнал, кеш хешей и блокировка запуска находятся в `<exe_dir>/data`. FreeSpace
никогда не пишет в `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` или временный каталог. Каталог
`data/quarantine`, оставшийся от старой версии, не используется и не удаляется автоматически;
удалите его вручную, убедившись, что в нём нет нужных файлов. Если каталог доступен только для
чтения, настройки живут в памяти на сессию, а команда сообщает об ошибке вместо перехода в
системный каталог.

## Стабильность

JSON-схемы, имена команд и коды возврата стабильны в пределах мажорной версии. Удаление или
переименование команды требует повышения мажорной версии. Новые необязательные поля могут
появляться в любое время.
