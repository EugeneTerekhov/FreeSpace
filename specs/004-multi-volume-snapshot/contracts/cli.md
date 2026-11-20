# CLI Contract: Мультитомный снимок

**Feature**: `004-multi-volume-snapshot` | **Version**: 2

## 1. `freespace scan <PATH>...`

Принимает один или несколько позиционных путей. Один путь сохраняет прежнее поведение; несколько
путей сканируются последовательно в один снимок.

```text
freespace scan C:\ D:\ --json
freespace scan D:\Media --method walk --json
freespace scan C:\ --snapshot-out C:\temp\scan.fssnap --json
```

Правила:

- 1..=32 пути;
- относительные, повторяющиеся и пересекающиеся пути → `E_INVALID_ARGUMENT` / `E_VALIDATION`
  (код возврата 2), скан не запускается;
- `--exclude` применяется ко всем корням;
- `--incremental` для нескольких корней выдаёт предупреждение и выполняет полный скан;
- `--snapshot-out` записывает основной (первый) том и все корни.

### JSON

```json
{
  "session_id": "…",
  "volumes": [
    {
      "id": "…C:",
      "mount_point": "C:\\",
      "label": "System",
      "file_system": "NTFS",
      "total_bytes": 1000000000000,
      "free_bytes": 250000000000
    },
    {
      "id": "…D:",
      "mount_point": "D:\\",
      "label": "Data",
      "file_system": "NTFS",
      "total_bytes": 2000000000000,
      "free_bytes": 500000000000
    }
  ],
  "nodes_count": 1200000,
  "total": {
    "logical": 1200000000000,
    "on_disk": 1250000000000,
    "physical": 1240000000000,
    "ads": 0
  },
  "roots": ["C:\\", "D:\\"],
  "methods": ["mft", "mft"],
  "method": "mft",
  "excluded": 42,
  "issues": []
}
```

- `volume` (единственный) остаётся в выводе для одного пути на один мажорный цикл; новый код
  читает `volumes`.
- `method` равен единственному методу или `mixed`.
- `total` — сумма по всем корням.
- `nodes_count` — сумма по всем корням.
- Коды возврата: `0` — успех или частичный успех; `1` — ошибка/ни один корень не просканирован;
  `2` — ошибка аргументов.

## 2. `freespace tree <PATH>...`

Принимает один или несколько путей и печатает все корни.

```text
freespace tree C:\ D:\ --depth 2 --min-size 104857600 --json
```

### JSON

```json
{
  "method": "mft",
  "session_id": "…",
  "nodes_count": 1200000,
  "total": {
    "logical": 1200000000000,
    "on_disk": 1250000000000,
    "physical": 1240000000000,
    "ads": 0
  },
  "roots": [
    { "name": "C:\\", "path": "C:\\", "kind": "directory", "children": [] },
    { "name": "D:\\", "path": "D:\\", "kind": "directory", "children": [] }
  ]
}
```

- `roots` содержит все корни в порядке аргументов.
- `total` — сумма по всем корням.
- Человекочитаемый вывод печатает корни последовательно.

## 3. Совместимость

- Один путь: прежние поля сохраняются; добавляются `volumes` и `methods`.
- Несколько путей: раньше команда принимала ровно один путь, поэтому поведение расширяется, а не
  ломается.
- `--json` остаётся валидным JSON в обоих режимах.
