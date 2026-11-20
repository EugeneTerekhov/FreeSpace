# Tasks: FreeSpace 2.0 — кроссплатформенный portable-релиз

**Feature**: `003-cross-platform-portable` | **Input**: [spec.md](./spec.md), [plan.md](./plan.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/)

## Формат

- `[P]` — задача может выполняться параллельно с другими в той же фазе.
- `[USn]` — относится к пользовательской истории из `spec.md`.
- Каждая задача имеет критерий готовности: код + тесты + документация (если применимо).
- Windows-only тесты помечаются `#![cfg(windows)]`, Linux-only — `#![cfg(unix)]`.

## Phase 0 — SDD и конституция

- [x] T001 Обновить `.specify/memory/constitution.md` до 2.0.0: принципы I, III, IV, VI и Sync Impact Report.
- [x] T002 Обновить `docs/en/constitution.md` (без кириллицы, ID сохранены).
- [x] T003 Создать `specs/003-cross-platform-portable/spec.md` с Clarifications и Out of Scope.
- [x] T004 Создать `plan.md`, `research.md`, `data-model.md`, `contracts/`, `quickstart.md`.
- [x] T005 Создать `tasks.md` и зафиксировать зависимости фаз.
- [ ] T006 Проверить, что `.specify/feature.json` указывает на `specs/003-cross-platform-portable`, и закоммитить SDD-артефакты.
- [x] T007 Добавить в `CHANGELOG.md` раздел `[Unreleased]` с `Changed`/`Removed` для 2.0.0.

## Phase 1 — Платформенный интерфейс (US1, US2, US6)

- [x] T008 [P] [US6] Определить `Capabilities`, `PlatformId`, `ScanMethods` в `crates/freespace-platform/src/capabilities.rs` с serde и тестами.
- [x] T009 [P] [US1] Определить трейты `PathPolicy`, `Volumes`, `Trash`, `Metadata`, `MonitorBackend`, `Elevation`, `UnlockService`, `ShredService`, `ContextMenuService`, `StorageHints`, `InstanceLock` в `crates/freespace-platform/src/lib.rs`.
- [x] T010 [US1] Создать `Platform` и `platform() -> &'static Platform` в `lib.rs`; `cfg` только в этой точке.
- [x] T011 [US2] Перенести существующую Windows-логику в `src/windows/` без изменения поведения; `windows`, `winreg`, `ferrisetw` — в `[target.'cfg(windows)'.dependencies]`.
- [x] T012 [US1] Сохранить фасады `paths::*`, `volumes::*`, `security::*`, `recycle::*` как делегаты к `platform()`, чтобы не ломать вызовы.
- [x] T013 [US1] Добавить `src/unix/mod.rs` с `Unsupported`-реализациями для Windows-only трейтов (shred, acl, attrs, ads, unlock, context_menu, trim).
- [x] T014 [US1] Добавить `[target.'cfg(unix)'.dependencies] rustix` (fs/process) и `percent-encoding`; обоснование уже в `research.md`.
- [x] T015 [US1] Добавить контрактный тест: `Capabilities` соответствует фактическим вызовам на текущей ОС.
- [x] T016 [US1] Убедиться, что `cargo check --workspace --all-targets` на Windows зелёный после рефакторинга.

## Phase 2 — Удаление карантина (US3)

- [x] T017 [P] [US3] Удалить `crates/freespace-ops/src/quarantine.rs`, экспорт и тесты; убрать `Quarantine` из `lib.rs`.
- [x] T018 [US3] Переписать `crates/freespace-ops/src/delete.rs`: только `RecycleBin`/`Permanent`, без `Option<&Quarantine>`, `quarantine_id`, `retention_days`, `quarantine_ids`.
- [x] T019 [US3] Переписать `crates/freespace-ops/src/undo.rs`: оставить только откат rename; удалить restore и `affected_paths`.
- [x] T020 [US3] `crates/freespace-ops/src/operation.rs`: `DeleteMode` без `Quarantine`; обновить `as_str`/`parse`, DTO и тесты.
- [x] T021 [US3] `crates/freespace-ops/src/store.rs`: `SCHEMA_VERSION = 2`, MIGRATION_2 с `DROP TABLE quarantine` и индексами; удалить `quarantine_*` и `read_quarantine_item`.
- [x] T022 [US3] Добавить тест миграции: база `user_version=1` с журналом и quarantine открывается, журнал читается, таблица удалена.
- [x] T023 [US3] `crates/freespace-app/src/operations.rs`: убрать `Quarantine` из сервиса/execute/undo; `reversible` считать по `details.renames`.
- [x] T024 [US3] `crates/freespace-app/src/dto`: удалить `DeleteMode::Quarantine`, `quarantine_ids`, `quarantine_retention_days`; обновить `JournalEntryView`.
- [x] T025 [US3] `crates/freespace-core/src/settings.rs`: удалить `quarantine_retention_days` из `CONFIG_KEYS`, `Settings`, `get/set` и тестов; старый TOML должен загружаться.
- [x] T026 [US3] `crates/freespace-cli/src/commands/delete.rs`: только `recycle`/`permanent`; `quarantine` → `E_INVALID_ARGUMENT`; убрать `quarantine_id`.
- [x] T027 [US3] CLI `journal`: убрать `restore`; `undo` — только для rename; обновить `cli.md` и help.
- [x] T028 [US3] Tauri: удалить `operation_restore`, `journal_restore`, `AppState::quarantine`; обновить `lib.rs` и `commands/operations.rs`.
- [x] T029 [US3] UI: `DeleteObjectDialog` — два режима; `Journal` — без restore и счётчика quarantine; `Settings` — без retention.
- [x] T030 [US3] UI DTO/моки: `DeleteMode`, `quarantine_ids`, `OperationProgress`, `JournalEntryView`; обновить `mockTransport`.
- [x] T031 [US3] UI-тесты: удаление показывает только два режима; журнал не предлагает restore; настройки без retention.
- [x] T032 [US3] Документация: описать, что `data/quarantine` больше не используется и не удаляется автоматически.
- [x] T033 [US3] Проверить, что старые записи `quarantine`/`restore` в журнале читаются как legacy-аудит.

## Phase 3 — Shred и сокращение экранов (US4)

- [x] T034 [P] [US4] `crates/freespace-ops/src/shred.rs`: `#[cfg(windows)]`, без изменений поведения на Windows.
- [x] T035 [US4] `crates/freespace-app/src/shred.rs` и Tauri `commands/shred.rs`: удалить; убрать `shred_*` из `lib.rs`.
- [x] T036 [US4] CLI `shred`: Windows-only вызов, вне Windows — `E_UNSUPPORTED`; в help пометить «Windows only».
- [x] T037 [US4] Удалить `apps/freespace-desktop/ui/src/screens/Shred.tsx`, `FileOps.tsx`, `Permissions.tsx`, `Rename.tsx`; `RenameDialog` оставить.
- [x] T038 [US4] `ScreenId`/`SCREENS`/`GROUPS`/`ScreenRouter`/иконки/палитра/hotkeys — 9 экранов.
- [x] T039 [US4] Legacy-алиасы: `shred` → `journal`, `fileops`/`rename`/`permissions` → `tree`.
- [x] T040 [US4] Tauri: удалить `permissions_*` и `attributes_*`; `rename_preview`/`rename_apply` оставить.
- [x] T041 [US4] i18n: удалить ключи удалённых экранов, перегенерировать каталоги, обновить тесты ключей.
- [x] T042 [US4] UI-тесты/e2e: убрать сценарии Shred/FileOps, добавить проверку 9 экранов и legacy-переходов.
- [x] T043 [US4] Контекстный rename в `Tree.tsx` оставить и покрыть тестом (preview + confirm).

## Phase 4 — Linux backend (US1, US6)

- [x] T044 [P] [US1] `unix/paths.rs`: `normalize`, `display`, `validate_length` (`PATH_MAX` 4096), `volume_root`, `equivalent_key` (регистрозависимость).
- [x] T045 [P] [US1] `unix/volumes.rs`: парсинг `/proc/self/mounts`, `statvfs`, `VolumeInfo`; тесты на временных mount-фикстурах/парсере.
- [x] T046 [P] [US1] `unix/metadata.rs`: `st_blocks`, mode/uid/gid, symlink, `allocated_ranges`; владелец через `/etc/passwd` с fallback на uid.
- [x] T047 [US1] `unix/trash.rs`: XDG trash (`$XDG_DATA_HOME/Trash`, `$topdir/.Trash-$uid`), `.trashinfo`, percent-encoding, коллизии; тесты.
- [x] T048 [US1] `unix/monitor.rs`: `/proc/<pid>/io`, перечисление процессов; интеграция с `monitor_fallback`.
- [x] T049 [US1] `unix/elevation.rs`: `geteuid() == 0`; `unix/reveal.rs`: `xdg-open`; `unix/instance_lock.rs`: `flock`.
- [x] T050 [US1] `unix/storage.rs`: `storage_kind` по sysfs; `trim` — `Unsupported`.
- [x] T051 [US1] `unix/unsupported.rs`: `shred`, `acl`, `attrs`, `ads`, `unlock`, `context_menu` → `Error::Unsupported`.
- [x] T052 [US1] Защищённые пути Linux (`/`, `/etc`, `/usr`, `/boot`, `/proc`, `/sys`, `/dev`, …) в `unix/guarded.rs`.
- [x] T053 [US1] `freespace-scan`: `mft_scan.rs`/`usn.rs` под `#[cfg(windows)]`; `MethodChoice`/`session` — через `Capabilities`.
- [x] T054 [US1] `freespace-scan/src/usn.rs`: заменить `std::os::windows::fs::MetadataExt` на платформенный helper (или оставить под Windows).
- [x] T055 [US1] `freespace-app`: permissions/size_details/volumes — через `Capabilities`, без `cfg` в бизнес-логике.
- [x] T056 [US1] `freespace-ops`: ACL/attrs/long paths — `Unsupported` на Linux; delete — `recycle`/`permanent`.
- [x] T057 [US1] Контрактные тесты Linux: `Capabilities`, XDG trash, пути, регистр, `PATH_MAX`, недоступные команды.
- [x] T058 [US1] Windows-тесты `scan_mft`, `unlock`, `elevation`, `permissions`, `context_menu`, `long_paths` — `#![cfg(windows)]`.
- [x] T059 [US6] `capabilities` в bootstrap GUI и CLI `capabilities --json`; UI скрывает недоступные действия.
- [x] T060 [US6] CLI help и `E_UNSUPPORTED` для `shred`/`acl`/`attrs`/`unlock` на Linux.

## Phase 5 — Portable-поставка (US1, US2, US5)

- [x] T061 [US2] `installer/portable.ps1`: версия из `Cargo.toml`, сборка frontend при отсутствии `ui/dist`, копирование `docs/ru|en/user-guide.md`, имя `freespace-portable-<ver>-windows-x64.zip`.
- [x] T062 [US2] Удалить `installer/nsis/hooks.nsh` и NSIS-секцию из `tauri.conf.json`; сборка — `--no-bundle`.
- [x] T063 [US1] `installer/portable.sh`: Linux tar.gz с `freespace-gui`, `freespace`, docs, `data/README.txt`, `run.sh`; версия из `Cargo.toml`.
- [x] T064 [US1] Проверить portable-модель: `data` рядом с exe, никаких записей в системные каталоги/реестр.
- [x] T065 [US1] Добавить в архивы README с runtime-зависимостями Linux (WebKitGTK 4.1/GTK3) и порядком запуска.
- [ ] T066 [US2] Проверить Windows ZIP на чистой Windows 10 21H2 и Windows 11 x64.
- [ ] T067 [US1] Проверить Linux tar.gz на Ubuntu 22.04/24.04 и Fedora.

## Phase 6 — CI, версии и релиз (US5)

- [x] T068 [US5] `.github/workflows/ci.yml`: добавить `ubuntu-22.04` job с GTK/WebKit-зависимостями, clippy/test, `cargo check --no-default-features`.
- [x] T069 [US5] Гейтить Windows-only тесты и добавить Linux-тесты; убедиться, что `cargo test --workspace` зелёный на обеих ОС.
- [x] T070 [US5] `scripts/version.mjs` с `set`/`check`: Cargo.toml, tauri.conf.json, package.json/lock; тест на рассинхрон.
- [x] T071 [US5] Убрать хардкод версии из `installer/portable.ps1`/`portable.sh`, `README.md`, user guide; `mockTransport` — оставить как фикстуру.
- [x] T072 [US5] `.github/workflows/release.yml`: `workflow_dispatch` с `tag`/`draft`, `permissions: contents: write`, matrix windows/ubuntu.
- [x] T073 [US5] Проверка тега: существует на remote и равен `v<workspace.version>`; падение до сборки.
- [x] T074 [US5] Финальный job: скачать артефакты, `SHA256SUMS`, создать/обновить draft через `softprops/action-gh-release@v2`.
- [x] T075 [US5] Тело релиза — секция CHANGELOG; повторный запуск не создаёт дубликат.
- [x] T076 [US5] Проверить, что `push` тега не запускает release-workflow и что падение matrix не публикует частичный релиз.

## Phase 7 — Документация и приёмка

- [x] T077 [P] Обновить `docs/ru/user-guide.md`: платформы, portable-запуск, режимы удаления, 9 экранов, CLI-only функции, миграция `data/quarantine`.
- [x] T078 [P] Обновить `docs/en/user-guide.md` (без кириллицы).
- [x] T079 [P] Обновить `docs/ru/cli.md` и `docs/en/cli.md`: `capabilities`, удаление, Windows-only команды.
- [x] T080 [P] Обновить `README.md`: платформы, portable-артефакты, отсутствие инсталляторов и подписи.
- [x] T081 [P] Обновить `CHANGELOG.md`: `[2.0.0]` с Added/Changed/Removed/Fixed.
- [x] T082 Обновить `specs/001-freespace-disk-analyzer` ссылки на карантин/установщик, если они противоречат 2.0 (без переписывания истории).
- [x] T083 Проверить `docs/en/` на кириллицу и неизменность ID (FR-, SC-, T-, CHK-).
- [ ] T084 Пройти `quickstart.md` на Windows 10/11 и Linux; зафиксировать результаты.
- [ ] T085 Финальный прогон: `fmt`, `clippy -D warnings`, `cargo test --workspace`, portable-архивы, draft-релиз.

## Зависимости фаз

```text
Phase 0 → Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7
```

- Phase 1 обязательна до Phase 4 (Linux backend реализует те же трейты).
- Phase 2 можно вести параллельно с Phase 1 после T008–T012, но миграция БД должна быть
  согласована с DTO.
- Phase 3 зависит от Phase 2 (журнал/удаление) и Phase 1 (Capabilities).
- Phase 5 зависит от Phase 4 и Phase 3 (состав GUI).
- Phase 6 зависит от Phase 5 (артефакты) и Phase 0 (версия 2.0.0).
- Phase 7 — после всех.

## MVP

Минимальный полезный срез: Phase 0–2 + Windows portable (T061–T062) — карантин удалён, Windows
работает, релиз ещё не автоматизирован. Затем Phase 3–4 (Linux) и Phase 5–6.
