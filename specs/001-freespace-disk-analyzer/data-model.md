# Модель данных: FreeSpace

**Feature**: `001-freespace-disk-analyzer` | **Date**: 2026-11-15 | **Plan**: [plan.md](./plan.md)

> **Примечание 2.0 (2026-11-20)**: модель 2.0 удаляет `QuarantineItem`, оставляет в `DeleteMode`
> только `RecycleBin` и `Permanent`, а миграция схемы 2 удаляет таблицу карантина. Старые записи
> журнала `quarantine`/`restore` читаются как аудит. Актуальная модель —
> [003/data-model.md](../003-cross-platform-portable/data-model.md).

Модель описывает доменные сущности, их поля (без привязки к физическому представлению) и правила
целостности. Типы указаны концептуально; в реализации используются компактные структуры и арены.

## 1. Volume (Том)

Представляет диск или раздел, доступный для анализа.

| Поле | Тип | Описание |
|------|-----|----------|
| `id` | UUID/строка | Стабильный идентификатор в пределах машины |
| `mount_point` | путь | Буква диска или точка монтирования |
| `label` | строка | Метка тома |
| `file_system` | перечисление | NTFS, FAT32, exFAT, ReFS, иное |
| `total_bytes` | u64 | Общий объём |
| `free_bytes` | u64 | Свободное место |
| `cluster_size` | u32 | Размер кластера (если известен) |
| `scan_method` | перечисление | MFT, USN, directory-walk |
| `is_elevated` | bool | Был ли анализ с повышенными правами |
| `is_removable` | bool | Съёмный носитель |
| `status` | перечисление | available, offline, inaccessible |

**Правила**:
- `free_bytes ≤ total_bytes`.
- `scan_method = MFT` допустим только для NTFS и при наличии прав.
- Для `status = offline` анализ и операции запрещены.

## 2. FsNode (Узел файловой системы)

Файл, каталог или ссылка в дереве анализа.

| Поле | Тип | Описание |
|------|-----|----------|
| `id` | u32 | Индекс в арене узлов |
| `parent_id` | u32? | Родительский узел (null для корня тома) |
| `first_child_id` | u32? | Первый дочерний узел |
| `next_sibling_id` | u32? | Следующий сосед |
| `name` | строка | Имя без пути |
| `kind` | перечисление | file, directory, symlink, junction, hardlink |
| `file_ref` | u64 | NTFS File Reference Number (для дедупликации) |
| `size` | SizeMetrics | Метрики размера (см. ниже) |
| `timestamps` | структура | created, modified, accessed (UTC) |
| `attributes` | битовая маска | read-only, hidden, system, archive, compressed, sparse, reparse |
| `owner_id` | u32? | Индексированный SID владельца |
| `access` | перечисление | ok, denied, error |
| `changed_during_scan` | bool | Объект изменился во время анализа |
| `link_target` | строка? | Цель ссылки (если применимо) |
| `ads` | список | Альтернативные потоки (при наличии) |

**Правила**:
- `kind = file` не имеет детей; `kind = directory` может иметь детей.
- Для `kind = hardlink` `size.logical` учитывается, `size.physical` — только один раз на `file_ref`.
- Для `kind = symlink/junction` цель не обходится без флага «следовать ссылкам».
- `access = denied` не исключает узел из дерева, но исключает его размер из точных сумм (помечается).

## 3. SizeMetrics (Метрики размера)

| Поле | Тип | Описание |
|------|-----|----------|
| `logical` | u64 | Логический размер (сумма длин файлов) |
| `on_disk` | u64 | Фактически занятое место (с учётом сжатия/sparse) |
| `physical` | u64 | Физический вклад с дедупликацией hard links |
| `ads` | u64 | Размер альтернативных потоков |
| `savings` | u64 | Экономия за счёт сжатия/sparse |
| `method` | перечисление | exact, estimated |

**Правила**:
- `physical ≤ logical` для каталогов с жёсткими ссылками.
- `savings = logical − on_disk` (не отрицательно).
- Если `method = estimated`, интерфейс и отчёты обязаны помечать значение как оценочное.

## 4. FsLink (Ссылка)

| Поле | Тип | Описание |
|------|-----|----------|
| `node_id` | u32 | Узел-ссылка |
| `link_type` | перечисление | hard, symlink, junction |
| `target_path` | строка | Цель ссылки |
| `target_file_ref` | u64? | FRN цели (если доступен) |
| `counted` | bool | Учтён ли целевой объект в физическом размере |

## 5. ScanSession (Сессия сканирования)

| Поле | Тип | Описание |
|------|-----|----------|
| `id` | UUID | Идентификатор сессии |
| `volume_id` | UUID | Том |
| `roots` | список путей | Области анализа |
| `method` | перечисление | MFT, USN, directory-walk |
| `started_at` / `finished_at` | время | Границы |
| `status` | перечисление | running, completed, cancelled, failed |
| `progress` | 0..100 | Прогресс |
| `nodes_count` | u64 | Число узлов |
| `errors` | список | Ошибки доступа и прочие |
| `journal_state` | структура? | JournalID и NextUSN для инкремента |
| `exclusions` | список | Исключённые пути |
| `snapshot_id` | UUID? | Снимок, созданный из сессии |

**Переходы состояний**: `running → completed | cancelled | failed`; переход назад запрещён.

## 6. DuplicateGroup (Группа дубликатов)

| Поле | Тип | Описание |
|------|-----|----------|
| `id` | UUID | Идентификатор группы |
| `hash_algorithm` | перечисление | sha256, md5 |
| `hash` | строка | Полный хеш |
| `size` | u64 | Размер файла |
| `members` | список `DuplicateMember` | Участники |
| `keep_node_id` | u32? | Рекомендованная к сохранению копия |
| `reclaimable_bytes` | u64 | Освобождаемый объём |
| `stale` | bool | Файлы изменились во время поиска |

**DuplicateMember**: `node_id`, `path`, `mtime`, `selected_for_deletion`, `verified` (побайтовое
совпадение), `reason`.

**Правила**:
- Минимум два участника.
- Удаление запрещено, если все участники помечены к удалению.
- `stale = true` блокирует удаление до повторной проверки.

## 7. CleanupCandidate (Кандидат на очистку)

| Поле | Тип | Описание |
|------|-----|----------|
| `node_id` | u32 | Объект |
| `category` | перечисление | temp, cache, log, backup, outdated, empty-dir, duplicate, large |
| `reason` | строка | Человекочитаемое объяснение |
| `reclaimable_bytes` | u64 | Потенциальная экономия |
| `risk` | перечисление | low, medium, high |
| `recommended_action` | перечисление | review, delete, keep |
| `selected` | bool | Выбран пользователем |

## 8. Snapshot (Снимок)

| Поле | Тип | Описание |
|------|-----|----------|
| `format_version` | u32 | Версия схемы |
| `id` | UUID | Идентификатор |
| `volume` | Volume-снимок | Метаданные тома |
| `created_at` | время | Дата создания |
| `method` | перечисление | Метод сканирования |
| `node_count` | u64 | Число узлов |
| `roots` | список | Корни анализа |
| `checksum` | строка | Контрольная сумма содержимого |
| `path` | строка | Путь к файлу `.fssnap` |

**Правила**:
- Несовпадение `format_version` или `volume.id` делает сравнение недопустимым.
- Снимок иммутабелен после создания.

## 9. SnapshotDiff (Дельта снимков)

| Поле | Тип | Описание |
|------|-----|----------|
| `base_snapshot_id` / `target_snapshot_id` | UUID | Сравниваемые снимки |
| `added` / `removed` / `changed` | список `DiffEntry` | Изменения |
| `growth_by_directory` | список `GrowthEntry` | Прирост по каталогам |
| `total_growth_bytes` | i64 | Суммарное изменение |

**DiffEntry**: `path`, `kind`, `size_delta`, `mtime_delta`.

## 10. FileLock (Блокировка файла)

| Поле | Тип | Описание |
|------|-----|----------|
| `node_id` | u32 | Файл |
| `process_id` | u32 | PID |
| `process_name` | строка | Имя процесса |
| `handle_type` | перечисление | file, directory, unknown |
| `access_mask` | u32 | Тип доступа |
| `can_soft_close` | bool | Доступно грациозное закрытие |
| `requires_reboot` | bool | Требуется удаление при перезагрузке |

## 11. Operation (Операция)

| Поле | Тип | Описание |
|------|-----|----------|
| `id` | UUID | Идентификатор |
| `kind` | перечисление | delete, shred, move, rename, permissions, attributes, restore, archive-extract, delete-on-reboot |
| `targets` | список `OperationTarget` | Объекты |
| `delete_mode` | перечисление? | recycle-bin, quarantine, permanent |
| `params` | структура | Параметры (метод перезаписи, проходы, правила и т.п.) |
| `status` | перечисление | planned, confirmed, running, completed, partially-completed, failed, cancelled, rolled-back |
| `requires_elevation` | bool | Требуются ли права |
| `created_at` / `finished_at` | время | Границы |
| `result_summary` | структура | Успехи/ошибки/объём |

**OperationTarget**: `node_id`, `path`, `size`, `status`, `error`.

**Переходы**: `planned → confirmed → running → completed | partially-completed | failed`;
`running → cancelled`; `completed | partially-completed → rolled-back` (если обратимо).

**Правила**:
- Переход в `running` невозможен без `confirmed`.
- Для `kind = shred` `delete_mode` всегда `permanent`.
- Для `kind = delete` `delete_mode` обязателен (выбор пользователя).

## 12. JournalEntry (Запись журнала)

| Поле | Тип | Описание |
|------|-----|----------|
| `id` | u64 | Автоинкремент |
| `operation_id` | UUID | Операция |
| `timestamp` | время | Когда |
| `user` | строка | Учётная запись |
| `summary` | строка | Краткое описание |
| `affected_count` | u32 | Число объектов |
| `affected_bytes` | u64 | Объём |
| `result` | перечисление | success, partial, error |
| `undo_available` | bool | Доступна ли отмена |
| `details` | JSON | Детали |

## 13. QuarantineItem (Элемент карантина)

| Поле | Тип | Описание |
|------|-----|----------|
| `id` | UUID | Идентификатор |
| `original_path` | строка | Исходный путь |
| `stored_path` | строка | Путь в карантине |
| `volume_id` | UUID | Исходный том |
| `size` | u64 | Размер |
| `deleted_at` | время | Дата помещения |
| `expires_at` | время | Срок хранения |
| `reason` | строка | Причина |
| `status` | перечисление | stored, restored, purged, corrupted |
| `checksum` | строка | Контрольная сумма для проверки целостности |

## 14. Report (Отчёт)

| Поле | Тип | Описание |
|------|-----|----------|
| `id` | UUID | Идентификатор |
| `format` | перечисление | csv, xml, html, xlsx |
| `scope` | структура | Что включено (дерево, топ, типы, дубликаты, владельцы) |
| `source_session_id` | UUID | Сессия-источник |
| `path` | строка | Файл отчёта |
| `created_at` | время | Дата |
| `row_count` | u64 | Число строк |
| `total_bytes` | u64 | Суммарный объём |

## 15. AutomationProfile (Профиль автоматизации)

| Поле | Тип | Описание |
|------|-----|----------|
| `id` | UUID | Идентификатор |
| `name` | строка | Имя профиля |
| `command` | строка | Команда CLI |
| `args` | список | Аргументы |
| `schedule` | структура? | Описание расписания (для экспорта в Планировщик) |
| `last_run` | время? | Последний запуск |
| `last_exit_code` | i32? | Код возврата |
| `log_path` | строка | Путь к журналу |

## 16. DiskEvent (Событие мониторинга)

| Поле | Тип | Описание |
|------|-----|----------|
| `timestamp` | время | Когда |
| `process_id` | u32? | PID |
| `process_name` | строка? | Имя процесса |
| `path` | строка? | Путь записи |
| `bytes_written` | u64 | Объём |
| `rate_bytes_per_sec` | u64 | Скорость |
| `source` | перечисление | etw, io-counters, usn |

## 17. MetadataRecord (Метаданные)

| Поле | Тип | Описание |
|------|-----|----------|
| `node_id` | u32 | Файл |
| `kind` | перечисление | exif, audio, video, pe |
| `tags` | карта строк | Теги и значения |
| `read_at` | время | Когда прочитано |
| `status` | перечисление | ok, unsupported, corrupted |

## 18. OwnerSummary (Сводка по владельцу)

| Поле | Тип | Описание |
|------|-----|----------|
| `owner_id` | u32 | Индекс SID |
| `sid` | строка | SID |
| `display_name` | строка | Имя учётной записи |
| `total_bytes` | u64 | Суммарный объём |
| `file_count` | u64 | Количество файлов |

## 19. ArchiveEntry (Элемент архива)

| Поле | Тип | Описание |
|------|-----|----------|
| `archive_node_id` | u32 | Узел архива |
| `internal_path` | строка | Путь внутри архива |
| `size` | u64 | Размер |
| `compressed_size` | u64? | Сжатый размер |
| `is_directory` | bool | Каталог |
| `encrypted` | bool | Зашифрован |
| `share` | f64 | Доля в содержимом архива |

## 20. Settings (Настройки)

| Поле | Тип | Описание |
|------|-----|----------|
| `language` | перечисление | ru, en |
| `theme` | перечисление | light, dark, system |
| `units` | перечисление | binary, decimal |
| `exclusions` | список масок | Исключаемые пути |
| `duplicate_keep_rule` | перечисление | oldest, newest, shortest-path, original-dir |
| `outdated_threshold_days` | u32 | Порог устаревания |
| `monitor_threshold` | структура | Порог и интервал уведомлений |
| `quarantine_retention_days` | u32 | Срок хранения карантина |
| `context_menu_enabled` | bool | Включить пункты Проводника (по умолчанию false) |
| `confirm_destructive_actions` | bool | Всегда подтверждать (по умолчанию true) |

Каталог данных не является настройкой: он всегда вычисляется как `<exe_dir>\data`, поэтому
`%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` и временные каталоги не участвуют в модели.

## Связи между сущностями

```text
Volume 1 ─── * ScanSession 1 ─── * FsNode
ScanSession 1 ─── 0..1 Snapshot 1 ─── * SnapshotDiff
FsNode 1 ─── * FsLink
FsNode 1 ─── * FileLock
FsNode * ─── * DuplicateGroup (через DuplicateMember)
FsNode 1 ─── * CleanupCandidate
FsNode 1 ─── * MetadataRecord
FsNode 1 ─── * ArchiveEntry
Volume 1 ─── * OwnerSummary
Operation 1 ─── * JournalEntry
Operation 1 ─── * QuarantineItem
AutomationProfile 1 ─── * Report
ScanSession 1 ─── * Report
DiskEvent * ─── 0..1 FsNode (по пути)
Settings 1 ─── 1 приложение
```

## Правила целостности и валидации

- Сумма `SizeMetrics` дочерних узлов MUST быть равна метрикам родителя с учётом исключённых и
  недоступных объектов (для `method = exact`).
- Операция не может быть выполнена над узлом с `access = denied` без повышения прав.
- Карантин не может занимать больше свободного места, чем доступно; при нехватке места операция
  требует выбора пользователя.
- Все временные метки хранятся в UTC и отображаются в локальном времени.
- Пути хранятся в полном виде (verbatim при необходимости) и не усекаются в журнале.
- `Snapshot` с несовпадающим `volume.id` не участвует в сравнении.
- `Settings.confirm_destructive_actions` не может быть отключён для шредера и смены прав.
