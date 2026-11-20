# Contract: CLI 2.0

**Feature**: `003-cross-platform-portable` | **Version**: 2.0.0

Формат JSON-конверта, коды возврата и стабильность в пределах мажорной версии сохраняются.
Меняются: набор режимов удаления, доступность команд по платформам и новая команда
`capabilities`.

## 1. Изменения команд

| Команда | Изменение |
|---|---|
| `delete --mode` | Только `recycle`/`recycle-bin` и `permanent`; `quarantine` → `E_INVALID_ARGUMENT` |
| `journal` | `restore` удалён; `undo` только для rename-записей |
| `shred` | Windows only; вне Windows — `E_UNSUPPORTED`; GUI-команды нет |
| `rename` | Без изменений, доступна на обеих платформах |
| `acl`, `attrs` | Windows only; вне Windows — `E_UNSUPPORTED` |
| `config` | Ключ `quarantine_retention_days` удалён; старый конфиг читается |
| `capabilities` | Новая команда, JSON-описание платформы |

## 2. `capabilities --json`

```json
{
  "version": "1",
  "command": "capabilities",
  "status": "ok",
  "data": {
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
}
```

`capabilities` доступна на обеих платформах и не требует сканирования.

## 3. Удаление

```bash
freespace delete --mode recycle   <paths...>
freespace delete --mode permanent <paths...>
freespace delete --mode quarantine <paths...> # E_INVALID_ARGUMENT
```

- JSON-ответ `delete` не содержит `quarantine_id`.
- `undoable` для удаления всегда `false`.
- В человекочитаемом выводе — только «Корзина»/«permanent», без упоминания карантина.

## 4. Коды возврата

Без изменений для существующих кодов. Для неподдерживаемой возможности — `8` (`E_UNSUPPORTED`),
как уже определено в `freespace-core`.

Windows-only команды на Linux:

```bash
freespace shred file.bin   # exit 8, E_UNSUPPORTED
freespace acl ...          # exit 8, E_UNSUPPORTED
freespace attrs ...        # exit 8, E_UNSUPPORTED
```

`--help` помечает такие команды `[Windows only]`.

## 5. Паритет

- Каждая возможность GUI MUST иметь CLI-команду: `drives`, `scan`, `tree`, `top`, `types`,
  `filter`, `duplicates`, `cleanup`, `export`, `snapshot`, `delete`, `journal`, `config`,
  `monitor`, `rename` (контекстный rename — `rename`).
- CLI MAY иметь дополнительные команды: `shred`, `acl`, `attrs`, `unlock`, `schedule`,
  `context-menu`.
- SC-010/SC-003: GUI-возможности ⊆ CLI-возможности; обратное не требуется.

## 6. Совместимость

- JSON-ключи, кроме удалённых `quarantine_ids`/`quarantine_id`, не переименовываются.
- Удаление ключей — breaking change 2.0.0; документируется в CHANGELOG `Removed`.
- Старые скрипты с `--mode quarantine` получают явную ошибку валидации, а не тихий permanent.
