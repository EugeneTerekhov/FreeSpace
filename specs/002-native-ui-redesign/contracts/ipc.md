# Контракт IPC: команды, события, пагинация и безопасность

**Feature**: `002-native-ui-redesign` | **Date**: 2026-11-16

Контракт описывает границу между React и Rust. Команды — единственный способ получить данные
или выполнить действие; события передают прогресс и инвалидацию, но не заменяют страницы
данных. Все схемы типизированы и проверяются golden-фикстурами.

## 1. Общие правила

- `IPC_VERSION = 1`. Аддитивные изменения совместимы; удаление/переименование поля или
  изменение смысла требует новой версии и обновления UI.
- Имена команд — `snake_case`; события — `kebab-case`.
- Вход и выход — JSON-совместимые DTO из [data-model.md](../data-model.md); большие страницы
  могут использовать бинарный `tauri::ipc::Response`.
- Ошибка команды — `Result<T, UiError>`; UI не показывает сырой текст Rust как единственное
  объяснение.
- Каждая команда валидирует вход до обращения к ФС; неизвестные поля игнорируются, неизвестные
  enum-значения отклоняются.
- Длительные команды возвращают `task_id`/`operation_id` и продолжают работу в фоне; отмена —
  отдельная команда.
- События throttle 10–20 Гц; `sequence` монотонно растёт в рамках задачи, пропуски допустимы,
  дубликаты отбрасываются по `sequence`.
- UI не вызывает команды, связанные с ФС, напрямую в обход `freespace-ops`.

## 2. Конверт ошибки

```json
{
  "code": "E_ACCESS_DENIED",
  "category": "access",
  "message_key": "error.access_denied",
  "details": "C:\\System Volume Information",
  "retryable": false,
  "task_id": null,
  "journal_entry_id": null
}
```

| Код | Категория | Когда |
|-----|-----------|-------|
| `E_VALIDATION` | validation | Неверный вход, enum, размер, путь |
| `E_ACCESS_DENIED` | access | Нет прав/доступа |
| `E_NOT_FOUND` | io | Объект/том/снимок не найден |
| `E_STALE_CURSOR` | stale | Курсор устарел; UI начинает с первой страницы |
| `E_STALE_NODE` | stale | Узел вне текущего поколения |
| `E_BUSY` | operation | Уже выполняется конфликтующая операция |
| `E_CANCELLED` | cancel | Задача отменена |
| `E_PREVIEW_EXPIRED` | validation | Предпросмотр истёк/токен неверен |
| `E_ELEVATION_REQUIRED` | access | Нужны права администратора |
| `E_ELEVATION_DENIED` | access | Пользователь отказался от повышения |
| `E_OPERATION_PARTIAL` | operation | Часть целей не выполнена |
| `E_IO` | io | Ошибка файловой системы |
| `E_UNSUPPORTED` | validation | ФС/формат/операция не поддерживается |
| `E_OFFLINE` | internal | Попытка сетевого действия запрещена |
| `E_INTERNAL` | internal | Внутренняя ошибка |

## 3. Приложение и оболочка

| Команда | Вход | Выход | Ошибки |
|---------|------|-------|--------|
| `app_bootstrap` | `{}` | `{ ipc_version, app_version, os, webview_version, settings, window_state, capabilities, about }` | `E_INTERNAL` |
| `app_settings_get` | `{}` | `UiSettings` | `E_INTERNAL` |
| `app_settings_set` | `{ patch: Partial<UiSettings> }` | `UiSettings` | `E_VALIDATION` |
| `app_window_state_get` | `{}` | `WindowState` | `E_INTERNAL` |
| `app_window_state_set` | `{ state: WindowState }` | `{}` | `E_VALIDATION` |
| `app_navigation_request` | `{ screen, node_id?, path? }` | `{}` | `E_VALIDATION`, `E_NOT_FOUND` |
| `app_open_logs` | `{}` | `{}` | `E_IO` |
| `app_about_info` | `{}` | `{ version, build, webview_version, licenses, data_dir }` | `E_INTERNAL` |
| `app_elevation_status` | `{}` | `{ elevated, can_elevate, reason_key }` | `E_INTERNAL` |
| `app_elevation_request` | `{ reason_key }` | `{ elevated }` | `E_ELEVATION_DENIED`, `E_INTERNAL` |
| `app_capabilities` | `{}` | `{ commands, events, csp_version }` | `E_INTERNAL` |

`app_bootstrap` вызывается один раз до первого экрана; `settings` и `window_state` — уже
мигрированные значения. `app_capabilities` нужен для матрицы паритета и диагностики.

## 4. Тома и сканирование

| Команда | Вход | Выход | Ошибки |
|---------|------|-------|--------|
| `volumes_list` | `{}` | `VolumeSummary[]` | `E_INTERNAL` |
| `volumes_summary` | `{ volume_id }` | `VolumeDetail` | `E_NOT_FOUND` |
| `volumes_refresh` | `{}` | `VolumeSummary[]` | `E_INTERNAL` |
| `scan_start` | `{ path, method: auto\|mft\|walk, incremental, exclusions, include_ads, metric }` | `{ task_id, generation }` | `E_VALIDATION`, `E_ACCESS_DENIED`, `E_BUSY` |
| `scan_status` | `{ task_id }` | `ScanTask` | `E_NOT_FOUND` |
| `scan_cancel` | `{ task_id }` | `{}` | `E_NOT_FOUND` |
| `scan_sessions` | `{}` | `ScanSessionSummary[]` | `E_INTERNAL` |

Правила: одновременно один активный `scan` на том; повторный запуск возвращает `E_BUSY`.
`generation` растёт после каждого завершённого/частичного сканирования.

## 5. Дерево, поиск и доли узлов

| Команда | Вход | Выход | Ошибки |
|---------|------|-------|--------|
| `tree_root` | `{ generation, metric, sort, page_size }` | `TreePage` | `E_STALE_CURSOR`, `E_NOT_FOUND` |
| `tree_children` | `{ generation, parent_id, metric, sort, filter, cursor?, page_size }` | `TreePage` | `E_STALE_NODE`, `E_STALE_CURSOR` |
| `tree_search` | `{ generation, query, filter, sort, cursor?, page_size }` | `TreePage` | `E_VALIDATION`, `E_STALE_CURSOR` |
| `tree_node` | `{ generation, node_id }` | `NodeRow` | `E_STALE_NODE`, `E_NOT_FOUND` |
| `tree_path` | `{ generation, node_id }` | `NodeRow[]` (предки) | `E_STALE_NODE` |

`TreePage` и `NodeRow` — по [data-model.md](../data-model.md). Доля узла от родителя и от тома
вычисляется нативным слоем и входит в `NodeRow`, поэтому встроенные прогресс-бары дерева не
требуют отдельных команд. `size_details` (раздел 7) отдаёт детализацию для карточки по наведению.

## 6. Анализ и очистка

| Команда | Вход | Выход | Ошибки |
|---------|------|-------|--------|
| `top_files_page` | `{ generation, limit, filter, cursor?, page_size }` | `TreePage` | `E_STALE_CURSOR` |
| `types_summary` | `{ generation, filter }` | `TypeSummary[]` | `E_STALE_CURSOR` |
| `types_files_page` | `{ generation, type, filter, cursor?, page_size }` | `TreePage` | `E_STALE_CURSOR` |
| `duplicates_start` | `{ path, min_size, algorithm, keep_rule, exclusions }` | `{ task_id }` | `E_VALIDATION`, `E_BUSY` |
| `duplicates_status` | `{ task_id }` | `DuplicateTask` | `E_NOT_FOUND` |
| `duplicates_groups_page` | `{ task_id, cursor?, page_size }` | `DuplicateGroupPage` | `E_STALE_CURSOR` |
| `duplicates_group` | `{ task_id, group_id }` | `DuplicateGroup` | `E_NOT_FOUND` |
| `duplicates_cancel` | `{ task_id }` | `{}` | `E_NOT_FOUND` |
| `cleanup_candidates` | `{ generation, older_than_days, filter }` | `CleanupCandidate[]` | `E_STALE_CURSOR` |
| `cleanup_empty_dirs_page` | `{ generation, cursor?, page_size }` | `TreePage` | `E_STALE_CURSOR` |
| `snapshots_list` | `{ volume_id? }` | `SnapshotSummary[]` | `E_IO`, `E_UNSUPPORTED` |
| `snapshots_create` | `{ path, out }` | `{ task_id, snapshot_id }` | `E_VALIDATION`, `E_IO` |
| `snapshots_compare` | `{ base_id, target_id }` | `{ task_id }` | `E_UNSUPPORTED`, `E_VALIDATION` |
| `snapshots_diff_page` | `{ task_id, cursor?, page_size, sort }` | `SnapshotDiffPage` | `E_STALE_CURSOR` |
| `snapshots_delete` | `{ snapshot_id }` | `{}` | `E_IO`, `E_NOT_FOUND` |
| `snapshots_status` | `{ task_id }` | `TaskStatus` | `E_NOT_FOUND` |

Правила: `duplicates_start` и `snapshots_compare` не блокируют UI; их прогресс приходит
событиями. `cleanup_candidates` и `cleanup_empty_dirs_page` только читают данные; удаление
идёт через `operation_preview`.

`types_summary` принимает необязательный `group_by` (`extension` по умолчанию или
`category`); поля `extension`/`category` в ответе заполняются соответственно. `journal_page`
принимает необязательный `filter` (`text`, `kinds`, `results`); фильтрация выполняется Rust.

## 7. Журнал, карантин и файловые операции

| Команда | Вход | Выход | Ошибки |
|---------|------|-------|--------|
| `journal_page` | `{ filter, cursor?, page_size }` | `JournalPage` | `E_STALE_CURSOR` |
| `journal_entry` | `{ entry_id }` | `JournalEntryView` | `E_NOT_FOUND` |
| `journal_undo` | `{ entry_id }` | `{ operation_id }` | `E_BUSY`, `E_UNSUPPORTED` |
| `journal_restore` | `{ entry_id, quarantine_ids? }` | `{ operation_id }` | `E_BUSY`, `E_IO` |
| `quarantine_items_page` | `{ cursor?, page_size }` | `QuarantinePage` | `E_STALE_CURSOR` |
| `size_details` | `{ generation, node_id }` | `SizeDetails` | `E_STALE_NODE` |
| `long_paths_page` | `{ generation, filter, cursor?, page_size }` | `TreePage` | `E_STALE_CURSOR` |
| `long_paths_validate` | `{ path }` | `{ valid, reason_key?, normalized? }` | `E_VALIDATION` |
| `rename_preview` | `{ targets, regex, replacement, scope }` | `RenamePreview` | `E_VALIDATION`, `E_BUSY` |
| `rename_apply` | `{ preview_id, confirmation_token }` | `{ operation_id }` | `E_PREVIEW_EXPIRED`, `E_BUSY` |
| `reveal_in_explorer` | `{ path }` | `{}` | `E_ACCESS_DENIED`, `E_NOT_FOUND` |
| `permissions_get` | `{ path }` | `PermissionsView` | `E_ACCESS_DENIED` |
| `permissions_preview` | `{ path, changes }` | `PermissionsPreview` | `E_VALIDATION`, `E_ACCESS_DENIED` |
| `permissions_apply` | `{ preview_id, confirmation_token }` | `{ operation_id }` | `E_PREVIEW_EXPIRED`, `E_ELEVATION_REQUIRED` |
| `attributes_preview` | `{ targets, changes }` | `OperationPreview` | `E_VALIDATION` |
| `attributes_apply` | `{ preview_id, confirmation_token }` | `{ operation_id }` | `E_PREVIEW_EXPIRED` |

`SizeDetails` содержит `logical_size`, `on_disk_size`, `physical_size`, `allocated_size`,
`hardlink_contribution`, `ads_size`, `compression_savings`, `sparse_savings`, `link_target`,
`metric_label`. UI не пересчитывает эти значения.

`reveal_in_explorer` только открывает каталог в Проводнике или показывает файл выбранным в родительском
каталоге. Команда ничего не меняет, принимает путь лишь внутри просканированной области и не имеет
CLI-аналога: это навигационное действие оболочки, а не операция над данными. Удаление из дерева идёт
через `operation_preview`/`operation_confirm` с `delete_mode = recycle_bin`, переименование — через
`rename_preview`/`rename_apply`.

## 8. Инспекция, отчёты и разблокировка

| Команда | Вход | Выход | Ошибки |
|---------|------|-------|--------|
| `metadata_get` | `{ generation, node_id }` | `MetadataView` | `E_STALE_NODE`, `E_UNSUPPORTED` |
| `owners_page` | `{ generation, filter, cursor?, page_size }` | `OwnerSummaryPage` | `E_STALE_CURSOR` |
| `owners_files_page` | `{ generation, owner, cursor?, page_size }` | `TreePage` | `E_STALE_CURSOR` |
| `archives_entries_page` | `{ task_id?, path, filter, cursor?, page_size }` | `ArchiveEntryPage` | `E_UNSUPPORTED`, `E_ACCESS_DENIED` |
| `archives_export_list` | `{ path, format, out }` | `ExportResult` | `E_UNSUPPORTED`, `E_IO` |
| `export_preview` | `{ scope, format, composition }` | `ExportPreview` | `E_VALIDATION` |
| `export_start` | `{ preview_id }` | `{ task_id }` | `E_PREVIEW_EXPIRED`, `E_BUSY` |
| `export_status` | `{ task_id }` | `TaskStatus` | `E_NOT_FOUND` |
| `export_cancel` | `{ task_id }` | `{}` | `E_NOT_FOUND` |
| `unlock_locks` | `{ path }` | `FileLock[]` | `E_NOT_FOUND`, `E_ACCESS_DENIED` |
| `unlock_release` | `{ path, mode: soft\|force, process_ids }` | `{ operation_id }` | `E_BUSY`, `E_ACCESS_DENIED` |
| `unlock_schedule_reboot` | `{ path }` | `{ operation_id }` | `E_ACCESS_DENIED` |

Архивы не распаковываются на диск; `archives_export_list` экспортирует только список
содержимого. Парольные/повреждённые архивы возвращают `E_UNSUPPORTED` с понятным ключом.

## 9. Операции, шредер и мониторинг

| Команда | Вход | Выход | Ошибки |
|---------|------|-------|--------|
| `operation_preview` | `{ kind, targets, options }` | `OperationPreview` | `E_VALIDATION`, `E_ACCESS_DENIED` |
| `operation_confirm` | `{ preview_id, confirmation_token, delete_mode? }` | `{ operation_id }` | `E_PREVIEW_EXPIRED`, `E_BUSY`, `E_ELEVATION_REQUIRED` |
| `operation_cancel` | `{ operation_id }` | `{}` | `E_NOT_FOUND` |
| `operation_status` | `{ operation_id }` | `OperationProgress` | `E_NOT_FOUND` |
| `operation_undo` | `{ operation_id }` | `{ operation_id }` | `E_UNSUPPORTED`, `E_BUSY` |
| `operation_restore` | `{ quarantine_ids }` | `{ operation_id }` | `E_IO`, `E_BUSY` |
| `shred_preview` | `{ targets, passes, pattern: zeros\|random }` | `OperationPreview` | `E_VALIDATION` |
| `shred_start` | `{ preview_id, confirmation_token }` | `{ operation_id }` | `E_PREVIEW_EXPIRED`, `E_BUSY` |
| `shred_status` | `{ operation_id }` | `OperationProgress` | `E_NOT_FOUND` |
| `shred_cancel` | `{ operation_id }` | `{}` | `E_NOT_FOUND` |
| `monitor_start` | `{ threshold, interval, exclusions }` | `{ task_id }` | `E_VALIDATION`, `E_BUSY` |
| `monitor_stop` | `{ task_id }` | `{}` | `E_NOT_FOUND` |
| `monitor_status` | `{ task_id }` | `MonitorStatus` | `E_NOT_FOUND` |
| `monitor_events_page` | `{ task_id, filter, cursor?, page_size }` | `MonitorEventPage` | `E_STALE_CURSOR` |
| `monitor_set_exclusions` | `{ exclusions }` | `{}` | `E_VALIDATION` |

Правила: `operation_confirm` без действующего `preview_id` + `confirmation_token` запрещён;
`shred` всегда безвозвратен и требует отдельного подтверждения; `operation_cancel` может
вернуть `E_UNSUPPORTED`, если операция прошла точку невозврата.

## 10. Настройки, elevation и «О программе»

| Команда | Вход | Выход | Ошибки |
|---------|------|-------|--------|
| `settings_get` | `{}` | `SettingsView` | `E_INTERNAL` |
| `settings_set` | `{ patch }` | `SettingsView` | `E_VALIDATION`, `E_IO` |
| `settings_validate_paths` | `{ paths }` | `PathValidation[]` | `E_VALIDATION` |
| `settings_reset` | `{ section }` | `SettingsView` | `E_VALIDATION` |
| `about_info` | `{}` | `AboutInfo` | `E_INTERNAL` |
| `about_licenses` | `{}` | `LicenseInfo[]` | `E_INTERNAL` |

`SettingsView` = существующие настройки 001 + `[ui]` из `UiSettings`. Секция `ui` не должна
ломать CLI `config get/set`; CLI видит только известные ему ключи.

## 11. События

| Событие | Payload | Частота | Назначение |
|---------|---------|---------|------------|
| `scan-progress` | `{ task_id, sequence, progress, phase_key, current_path? }` | 10–20 Гц | Прогресс сканирования |
| `scan-completed` | `{ task_id, state, generation, summary?, error? }` | однократно | Завершение/частичный результат |
| `duplicates-progress` | `{ task_id, sequence, progress, phase_key }` | 10–20 Гц | Поиск дубликатов |
| `duplicates-completed` | `{ task_id, state, groups_count, error? }` | однократно | Завершение поиска |
| `operation-progress` | `{ operation_id, sequence, state, processed, total, current_target?, errors? }` | 10–20 Гц | Прогресс операции |
| `operation-completed` | `{ operation_id, state, journal_entry_id?, undo_available, error? }` | однократно | Завершение операции |
| `monitor-tick` | `{ task_id, sequence, events }` | 1–2 Гц | Мониторинг записи |
| `task-queue-changed` | `{ tasks: ScanTask[] }` | по изменению | Оболочка/статус |
| `volume-changed` | `{ volumes: VolumeSummary[] }` | по событию Windows | Появление/удаление тома |
| `settings-changed` | `{ settings: SettingsView }` | по изменению | Синхронизация экранов |
| `navigation-request` | `{ screen, node_id?, path? }` | по событию | Глубокие ссылки, single instance |

Конверт события:

```json
{
  "version": 1,
  "event": "scan-progress",
  "sequence": 42,
  "task_id": "…",
  "emitted_at": "2026-11-16T12:00:00Z",
  "payload": { "progress": { "processed": 1000, "total": 100000 } }
}
```

Правила: `sequence` монотонен в рамках задачи; UI отбрасывает `sequence ≤ last_seen`;
прогресс не чаще 20 Гц; ошибки не дублируются в каждом событии, а приходят в
`*-completed`/`UiError`.

## 12. Пагинация, курсоры и поколения

- `page_size` — 1..2000, по умолчанию 500; `max_tiles` — 100..10 000.
- `cursor` — непрозрачная строка ≤ 512 символов; UI не парсит её.
- `next_cursor = null` — конец; `stale_cursor = true` — начать с первой страницы.
- `generation` — монотонный номер снапшота данных; страницы другого поколения отклоняются.
- Смена сортировки/фильтра/поколения сбрасывает курсор.
- `total_estimate` — оценка, не точный счётчик; UI не использует её для итоговых сумм.

## 13. Предпросмотр и подтверждение операции

```text
operation_preview
  → preview_id, confirmation_token, expires_at, targets, total_size, warnings
operation_confirm(preview_id, token, delete_mode?)
  → operation_id
operation_status / operation-progress / operation-completed
  → journal_entry_id, undo_available
```

Правила:

- `preview_id` и `confirmation_token` одноразовые, TTL 10 минут, привязаны к пользователю и
  набору целей.
- Повторное подтверждение — `E_PREVIEW_EXPIRED`.
- UI обязан показать `warnings` (SSD, системный путь, большой объём) до подтверждения.
- Безвозвратное удаление и шредер требуют отдельного подтверждения, отличного от обычного.
- Операция не начинается, если `requires_elevation = true`, а прав нет.
- При частичном успехе UI показывает `partial` и ссылку в журнал, а не «успех».

## 14. Безопасность и возможности

- Capabilities: `core:default` + точечные `core:window:*`, `core:event:*`,
  `single-instance:default`; плагины `fs`, `shell`, `http`, `process` не подключаются.
- CSP: `default-src 'self'; connect-src ipc: http://ipc.localhost; img-src 'self' data:;
  style-src 'self' 'unsafe-inline'; script-src 'self'`; remote URL запрещены.
- `withGlobalTauri: false`; devtools выключены в release; `dangerousDisableAssetCspModification`
  не используется.
- Все пути канонизируются; выход за выбранную область — `E_ACCESS_DENIED`.
- Команды не принимают произвольные shell-команды, URL или SQL.
- Сетевые вызовы запрещены; фоновая сеть WebView2 отключена аргументами и проверяется
  network guard в E2E.
- Логи — только локально в `data/logs`; без содержимого файлов и секретов.

## 15. Проверка контракта

- Для каждой команды есть golden-фикстура успеха и хотя бы одной ошибки в
  `specs/002-native-ui-redesign/contracts/fixtures/` (или `tests/contract/`).
- Тест сверяет список команд frontend-клиента с `app_capabilities`; лишняя/пропавшая
  команда — падение.
- Тест проверяет, что `sequence` монотонен, а `stale_cursor` обрабатывается.
- Тест проверяет, что UI не вызывает запрещённые плагины/сетевые API.
- Все изменения контракта сопровождаются обновлением `IPC_VERSION` (при необходимости) и
  матрицы паритета.
