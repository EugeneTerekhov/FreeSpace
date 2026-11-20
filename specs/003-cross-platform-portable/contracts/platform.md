# Contract: Platform Interface

**Feature**: `003-cross-platform-portable` | **Version**: 2.0.0

## 1. Общие правила

- Все платформенные различия находятся в `freespace-platform`.
- Верхние крейты не импортируют `windows`, `winreg`, `ferrisetw`, `rustix` и не содержат
  `cfg(windows)`/`cfg(unix)` в бизнес-логике.
- Трейты object-safe, `Send + Sync`, без generics и без платформенных типов в сигнатурах.
- Ошибки — `freespace_core::Error`; недоступная возможность — `Error::Unsupported`.
- Одна точка сборки: `freespace_platform::platform() -> &'static Platform`.

## 2. `Platform`

```rust
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
```

`Platform` MUST быть доступен без аллокаций и содержать только `'static` ссылки.

## 3. Трейты

| Трейт | Методы (минимум) | Windows | Linux |
|---|---|---|---|
| `PathPolicy` | `normalize`, `display`, `validate_length`, `equivalent_key`, `volume_root`, `join_within_limit`, `ensure_within` | verbatim, `MAX_PATH` | POSIX, `PATH_MAX`, mount-aware |
| `Volumes` | `list`, `describe` | `GetLogicalDrives` + `GetVolumeInformation` | `/proc/self/mounts` + `statvfs` |
| `Trash` | `trash(paths)` | `IFileOperation` | XDG trash |
| `Metadata` | `attributes`, `set_attributes`, `owner`, `allocated_size`, `allocated_ranges`, `streams`, `link_kind` | WinAPI | mode/uid/gid, `st_blocks`, symlinks; запись атрибутов/ACL/ADS — `Unsupported` |
| `MonitorBackend` | `snapshot`, `counters_for` | ETW + process counters | `/proc/<pid>/io` |
| `Elevation` | `is_elevated` | UAC token | `geteuid() == 0` |
| `UnlockService` | `find_locks`, `release_soft`, `terminate`, `delete_on_reboot` | Restart Manager | `Unsupported` |
| `ShredService` | `preview`, `run` | overwrite + TRIM | `Unsupported` |
| `ContextMenuService` | `set_enabled`, `status`, `uninstall` | `winreg` | `Unsupported` |
| `StorageHints` | `kind`, `trim` | DeviceIoControl | `storage_kind` по sysfs; `trim` — `Unsupported` |
| `InstanceLock` | `acquire`, `release` | file lock | `flock` |

## 4. `Capabilities`

```json
{
  "platform": "windows",
  "scan_methods": { "mft": true, "usn": true, "walk": true },
  "trash": true,
  "permanent": true,
  "shred": true,
  "acl": true,
  "attrs": true,
  "ads": true,
  "unlock": true,
  "context_menu": false,
  "monitor": true,
  "elevation": true,
  "long_paths": true,
  "case_sensitive": false,
  "max_path": null
}
```

Linux:

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

Примечание: `shred` в `Capabilities` означает наличие CLI-команды; GUI-экрана шредера нет ни на
одной платформе.

## 5. Контракт ошибок

| Ситуация | Ошибка | Код |
|---|---|---|
| Возможность недоступна на платформе | `Error::Unsupported` | `E_UNSUPPORTED` |
| XDG trash недоступна на томе | `Error::Unsupported`/`Error::Io` | `E_UNSUPPORTED`/`E_IO` |
| Путь длиннее лимита | `Error::InvalidPath` | `E_INVALID_PATH` |
| Защищённый путь без доп. подтверждения | `Error::ProtectedPath` | `E_PROTECTED_PATH` |
| Режим `quarantine` | `Error::InvalidArgument` | `E_INVALID_ARGUMENT` |

## 6. Обязательные тесты

- Контрактный тест для обоих бэкендов: `Capabilities` соответствует фактическим вызовам.
- Windows-only методы на Linux возвращают `Unsupported`, а не паникуют.
- `PathPolicy::equivalent_key` различает регистр на Linux и склеивает на Windows.
- XDG trash: временный том, коллизии имён, `.trashinfo`, недоступный `$topdir`.
- Миграция БД: старый файл с `user_version=1` открывается, журнал читается.
