# Data Model: Мультитомный снимок

**Feature**: `004-multi-volume-snapshot` | **Date**: 2026-11-20

## 1. Корень сканирования (ScanRoot)

Описывает один выбранный пользователем путь внутри запроса.

| Поле | Тип | Правило |
|---|---|---|
| `path` | строка пути | Абсолютный, непустой; 1..=`MAX_PATH_CHARS` |
| `volume` | `Volume` | Определяется после валидации и канонизации |
| `method` | `ScanMethod` | `mft` для NTFS-корня тома, иначе `walk`; при `auto` — fallback |
| `state` | `ok` / `failed` / `cancelled` | Итог корня |
| `error` | `UiError?` | Заполняется только при `failed` |

Инварианты:

- Пути уникальны по платформенному ключу.
- Ни один путь не является предком другого.
- Количество путей — от 1 до 32.

## 2. Запрос сканирования (ScanRequest)

| Поле | Тип | Изменение |
|---|---|---|
| `paths` | `string[]` | Новое; заменяет `path` |
| `method` | `auto` / `mft` / `walk` | Без изменений |
| `incremental` | bool | Без изменений; для нескольких корней выдаётся предупреждение о полном скане |
| `exclusions` | `string[]` | Без изменений; общие для всех корней |
| `include_ads` | bool | Без изменений; обогащение выполняется один раз после всех корней |
| `metric` | `logical` / `on_disk` / `physical` | Без изменений |

## 3. Задача сканирования (Task)

| Поле | Тип | Изменение |
|---|---|---|
| `task_id` | string | Без изменений |
| `kind` | `scan` | Без изменений |
| `state` | `queued` / `running` / `cancelling` / `cancelled` / `completed` / `partial` / `failed` | `partial` при ошибке хотя бы одного корня |
| `volume_id` | string? | Первый том; сохранён для совместимости |
| `volume_ids` | string[] | Новое; все занятые тома |
| `progress` | `TaskProgress?` | Общий процент, текущий путь |
| `error` | `UiError?` | Ошибка только при полном провале |

Переходы:

```
queued → running → completed
                 ├→ partial   (≥1 корень успешен, ≥1 корень с ошибкой)
                 ├→ cancelled (пользователь отменил)
                 └→ failed    (0 успешных корней)
```

## 4. Результат сканирования (ScanOutcome)

| Поле | Тип | Правило |
|---|---|---|
| `tree` | `FsTree` | Одна арена; несколько корней |
| `roots` | `RootScanOutcome[]` | По одной записи на успешный корень |
| `failures` | `ScanFailure[]` | Пути и ошибки неуспешных корней |
| `warnings` | `string[]` | Включая fallback MFT→walk и ошибки корней |
| `issues` | `ScanIssue[]` | Нефатальные проблемы всех корней |
| `cancelled` | bool | Истина, если отмена прервала скан |
| `ads_streams` | u64 | Обогащение ADS по всей арене |

`RootScanOutcome`:

| Поле | Тип |
|---|---|
| `session` | `ScanSession` (один корень) |
| `method` | `ScanMethod` |
| `volume` | `Volume` |
| `excluded` | u64 |
| `issues` | `ScanIssue[]` |
| `warnings` | `string[]` |
| `cancelled` | bool |

## 5. Снимок (TreeSnapshot)

| Поле | Тип | Изменение |
|---|---|---|
| `tree` | `FsTree` | Без изменений; может иметь несколько корней |
| `generation` | u64 | Без изменений |
| `volumes` | `Volume[]` | Новое; уникальные тома в порядке корней |
| `sessions` | `ScanSession[]` | Новое; по одной на успешный корень |
| `roots` | `NodeId[]` | Без изменений; кеш `tree.roots()` |

Аксессоры:

- `volume()` → `volumes[0]` (совместимость);
- `session()` → `sessions.first()` (совместимость);
- `volumes()` → все тома;
- `sessions()` → все сессии;
- `volume_for_path(path)` → том с самым длинным подходящим mount point;
- `root_id()` → первый корень (совместимость).

Инварианты:

- `volumes` не пуст; `volumes.len() == sessions.len()` для снимка из скана.
- Каждый `NodeId` из `roots` принадлежит `tree`.
- Генерация монотонна и уникальна для снимка.

## 6. Сводка сессии (ScanSessionSummary)

Остаётся одна запись на корень; поля не меняются. `volume_id` — том этого корня, `roots` —
один путь корня. `scan_sessions` возвращает записи всех корней, новые первыми.

## 7. События сканирования

`scan-progress`:

- `progress.processed` — обработанные узлы текущего корня;
- `progress.total` — оценка текущего корня;
- `phase_key` — фаза текущего корня;
- `current_path` — путь текущего корня;
- `bytes_per_second` — скорость текущего корня.

`scan-completed` → `ScanSummary`:

| Поле | Тип | Изменение |
|---|---|---|
| `nodes_count` | u64 | Сумма по корням |
| `method` | string | Единственный метод или `mixed` |
| `methods` | string[] | Новое |
| `roots` | string[] | Все корни |
| `volumes` | string[] | Новое; mount points |
| `warnings` | string[] | Включая ошибки корней |
| `issues_count` | u64 | Сумма |
| `elapsed_ms` | u64 | Общее время |

## 8. Хранение и совместимость

- Настройки, журнал и SQLite не меняются.
- `.fssnap` и отчёты остаются версии 1: основной том = `volumes[0]`, `roots` = все корни.
- `snapshot_volumes(generation)` — новый запрос; `volumes_current` — совместимый, возвращает
  первый том.
