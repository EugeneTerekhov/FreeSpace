# IPC Contract: Мультитомный снимок

**Feature**: `004-multi-volume-snapshot` | **Version**: IPC 2

Версия `IPC_VERSION` повышается с 1 до 2: запрос `scan_start` больше не принимает `path`.

## 1. `scan_start`

Запускает одно сканирование одного или нескольких корней.

### Request

```json
{
  "request": {
    "paths": ["C:\\", "D:\\"],
    "method": "auto",
    "incremental": false,
    "exclusions": ["**/node_modules"],
    "include_ads": false,
    "metric": "on_disk"
  }
}
```

Правила:

- `paths` — 1..=32 абсолютных путей;
- относительные, пустые, повторяющиеся и пересекающиеся пути → `E_VALIDATION`;
- хотя бы один занятый том → `E_BUSY`;
- новое сканирование не отменяет текущий снимок до успеха.

### Response

```json
{
  "task_id": "6f1c…",
  "generation": 12
}
```

## 2. `scan_status`

Без изменений. `Task.volume_ids` содержит все занятые тома; `volume_id` — первый из них.

```json
{
  "task_id": "6f1c…",
  "kind": "scan",
  "state": "running",
  "volume_id": "…C:",
  "volume_ids": ["…C:", "…D:"],
  "progress": {
    "processed": 120000,
    "total": 480000,
    "current_path": "D:\\Media",
    "bytes_per_second": 125000000,
    "items_per_second": 900,
    "phase_key": "scan.phase.building",
    "updated_at": 1790000000000
  },
  "started_at": 1790000000000,
  "finished_at": null,
  "cancelable": true,
  "error": null
}
```

## 3. `scan_cancel`

Без изменений: отменяет текущий корень и все оставшиеся.

## 4. `scan_sessions`

Возвращает по одной записи на корень, новые первыми. `volume_id` — том корня, `roots` — один
путь. Формат записи не меняется.

## 5. `snapshot_volumes` (новый)

Возвращает тома снимка указанной генерации в порядке корней.

### Request

```json
{ "generation": 12 }
```

### Response

```json
[
  {
    "volume_id": "…C:",
    "mount_point": "C:\\",
    "label": "System",
    "file_system": "NTFS",
    "total_bytes": 1000000000000,
    "free_bytes": 250000000000,
    "used_bytes": 750000000000,
    "is_removable": false,
    "status": "available",
    "scan_method": "mft",
    "is_elevated": true
  },
  {
    "volume_id": "…D:",
    "mount_point": "D:\\",
    "label": "Data",
    "file_system": "NTFS",
    "total_bytes": 2000000000000,
    "free_bytes": 500000000000,
    "used_bytes": 1500000000000,
    "is_removable": false,
    "status": "available",
    "scan_method": "mft",
    "is_elevated": true
  }
]
```

Ошибки: `E_NOT_FOUND` — снимка нет; `E_STALE_CURSOR` — генерация не текущая.

## 6. `volumes_current`

Совместимый: возвращает первый том текущего снимка. Новый код использует `snapshot_volumes`.

## 7. `tree_root`, `tree_children`, `tree_search`, `tree_node`, `tree_path`

Без изменений формата. `tree_root` возвращает страницу всех корней снимка; у каждой строки
`parent_id = null`, `kind = "directory"`, `path` — точка монтирования.

## 8. Событие `scan-progress`

Формат не меняется; `current_path` и счётчики относятся к текущему корню. Общий процент
монотонен и пересчитывается как `(index + local) / roots_count`.

## 9. Событие `scan-completed`

### Payload

```json
{
  "task_id": "6f1c…",
  "state": "partial",
  "generation": 12,
  "summary": {
    "nodes_count": 1200000,
    "method": "mixed",
    "methods": ["mft", "directory-walk"],
    "roots": ["C:\\", "D:\\"],
    "volumes": ["C:\\", "D:\\"],
    "warnings": ["D:\\: access denied"],
    "issues_count": 7,
    "elapsed_ms": 42000
  },
  "error": null
}
```

`state`:

- `completed` — все корни успешны, нет проблем;
- `partial` — хотя бы один корень успешен, но есть ошибки или проблемы;
- `failed` — ни один корень не просканирован; `summary = null`, `error` заполнен;
- `cancelled` — отмена.

## 10. Совместимость

- Клиенты IPC 1 несовместимы: поле `path` удалено.
- `volumes_current` оставлен на один мажорный цикл.
- `Task.volume_id` оставлен; новые клиенты читают `volume_ids`.
