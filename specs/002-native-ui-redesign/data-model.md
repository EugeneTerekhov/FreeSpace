# Модель данных: FreeSpace — единый нативный интерфейс нового поколения

**Feature**: `002-native-ui-redesign` | **Date**: 2026-11-16

Модель описывает сущности представления и границы IPC. Доменные сущности (`Volume`, `FsNode`,
`SizeMetrics`, `ScanSession`, `DuplicateGroup`, `Snapshot`, `Operation`, `JournalEntry`,
`QuarantineItem`, `Settings`) остаются в `freespace-core`/`freespace-ops` и не дублируются в
TypeScript. Rust — единственный источник истины; UI хранит только проекции и состояние
представления.

## 1. Сущности интерфейса

### Screen

Экран приложения.

| Поле | Тип | Обязательность | Правила |
|------|-----|----------------|---------|
| `id` | enum (`overview`, `tree`, `analysis`, `duplicates`, `cleanup`, `journal`, `shred`, `monitor`, `inspect`, `fileops`, `settings`) | обязательно | Ровно 11 значений; неизвестный id — ошибка навигации |
| `group` | enum (`overview`, `analysis`, `cleanup`, `operations`, `tools`, `system`) | обязательно | Группа сайдбара, определяет порядок в навигации |
| `title_key` | `MessageKey` | обязательно | Ключ `nav.*` |
| `requires_volume` | bool | обязательно | Если true и том не выбран — пустое состояние с выбором тома |
| `requires_scan` | bool | обязательно | Если true и нет данных — предложить сканирование |
| `capabilities` | массив enum | обязательно | Действия, доступные на экране |
| `cli_analog` | строка или null | обязательно | Команда CLI для матрицы паритета |

Связи: `Screen 1 — 0..1 ScreenState`; `Screen 1 — 0..* ColumnState`; `Screen 1 — 0..* ScreenshotBaseline`;
`Screen 1 — 1 ParityMatrix`.

### ScreenState

Состояние конкретного экрана.

| Поле | Тип | Правила |
|------|-----|---------|
| `screen_id` | `Screen.id` | Обязательно |
| `phase` | enum (`idle`, `loading`, `ready`, `empty`, `partial`, `error`, `refreshing`) | Обязательно |
| `error` | `UiError?` | Обязателен при `phase = error`/`partial` |
| `volume_id` | `VolumeId?` | Обязателен для экранов тома |
| `selection` | `SelectionModel` | Пустой по умолчанию |
| `filters` | `FilterSet` | Пустой по умолчанию |
| `sort` | `SortSpec?` | Один активный ключ |
| `page` | `PageCursor?` | Сбрасывается при смене фильтра/сортировки/поколения |
| `scroll_anchor` | строка или null | Стабильный id строки, а не пиксель |
| `last_updated` | timestamp | Для индикатора актуальности |

Переходы:

```text
idle → loading → ready | empty | partial | error
ready → refreshing → ready | partial | error
ready | partial | error → loading   (смена тома/фильтра)
any → idle                           (смена экрана без сохранения данных)
```

### ScanTask

Задача сканирования/анализа, видимая в оболочке.

| Поле | Тип | Правила |
|------|-----|---------|
| `task_id` | UUID | Уникален в сессии |
| `kind` | enum (`scan`, `duplicates`, `snapshot_compare`, `archive_scan`, `monitor`) | Обязательно |
| `state` | enum (`queued`, `running`, `cancelling`, `cancelled`, `completed`, `partial`, `failed`) | Обязательно |
| `volume_id` | `VolumeId?` | Для задач тома |
| `progress` | `TaskProgress?` | Обязателен при `running` |
| `started_at` / `finished_at` | timestamp | `finished_at ≥ started_at` |
| `cancelable` | bool | false после точки невозврата |
| `error` | `UiError?` | При `failed`/`partial` |

Переходы: `queued → running → completed | partial | failed`; `running → cancelling →
cancelled`; запрещено `completed → running`.

### TaskProgress

| Поле | Тип | Правила |
|------|-----|---------|
| `processed` | u64 | Монотонно не убывает |
| `total` | u64? | Может быть неизвестен |
| `current_path` | строка? | Обрезается в UI, но хранится полностью |
| `bytes_per_second` | u64? | Не отрицательно |
| `items_per_second` | u64? | Не отрицательно |
| `phase_key` | `MessageKey` | Локализованная фаза |
| `updated_at` | timestamp | Не чаще 20 Гц |

### TreePage

Страница дерева/таблицы.

| Поле | Тип | Правила |
|------|-----|---------|
| `generation` | u64 | Растёт при новом сканировании/инвалидации |
| `parent_id` | `NodeId?` | null для корня/плоского списка |
| `cursor` | строка? | Непрозрачный, ≤ 512 символов |
| `next_cursor` | строка? | null — конец |
| `rows` | `NodeRow[]` | 0..2000 |
| `total_estimate` | u64? | Оценка, не точный счётчик |
| `metric` | enum (`logical`, `on_disk`, `physical`) | Обязательно |
| `sort` | `SortSpec` | Обязательно |
| `stale_cursor` | bool | true — UI перезапрашивает первую страницу |

Правила: `rows.length ≤ page_size`; `next_cursor` не равен `cursor`; при `stale_cursor = true`
остальные поля могут быть пустыми.

### NodeRow

Проекция `FsNode` для строки.

| Поле | Тип | Правила |
|------|-----|---------|
| `id` | `NodeId` (u64/строка) | Уникален в поколении |
| `parent_id` | `NodeId?` | Для корня null |
| `name` | строка | ≤ 32 000 символов, Unicode |
| `path` | строка | Полный путь, ≤ 32 000 символов |
| `kind` | enum (`file`, `directory`, `link`, `other`) | Обязательно |
| `logical_size` | u64 | ≥ 0 |
| `on_disk_size` | u64 | ≥ 0 |
| `physical_size` | u64? | Для hard links/ADS, если известно |
| `allocated_size` | u64? | Если известно |
| `child_count` | u32 | Для каталогов |
| `file_count` | u64? | Для агрегатов |
| `modified_at` | timestamp? | Может отсутствовать |
| `accessed_at` | timestamp? | Может отсутствовать; помечать недостоверность |
| `attributes` | набор флагов | read-only, hidden, system, compressed, sparse, reparse |
| `accessible` | bool | false — пометка «нет доступа» |
| `link_target` | строка? | Для ссылок |
| `ads_size` | u64? | Суммарный размер ADS |

Правило: UI не пересчитывает `logical_size`/`on_disk_size` и не суммирует строки для итогов;
итоги приходят из Rust.

### NodeProgress

Встроенный прогресс-бар узла дерева.

| Поле | Тип | Правила |
|------|-----|---------|
| `node_id` | `NodeId` | Обязательно |
| `share_of_parent` | f32 | 0..1, доля размера узла от родителя |
| `share_of_volume` | f32 | 0..1, доля размера узла от всего тома |

Правила: доли вычисляются Rust и входят в `NodeRow`; UI использует их только для
полупрозрачной заливки строки дерева и не пересчитывает суммы.

### FilterSet

| Поле | Тип | Правила |
|------|-----|---------|
| `text` | строка | ≤ 256 символов |
| `extensions` | массив строк | Нормализованные, без точки |
| `categories` | массив enum | Типы файлов |
| `min_size` / `max_size` | u64? | `min ≤ max` |
| `modified_after` / `modified_before` | дата? | `after ≤ before` |
| `accessible_only` | bool | По умолчанию false |
| `exclude_paths` | массив строк | Verbatim-совместимые |
| `name` | строка? | Имя сохранённого набора, ≤ 64 |

### SelectionModel

| Поле | Тип | Правила |
|------|-----|---------|
| `mode` | enum (`none`, `single`, `multiple`, `range`) | Обязательно |
| `ids` | `Set<NodeId>` | Только видимые/загруженные объекты |
| `anchor_id` | `NodeId?` | Для range |
| `select_all_in_filter` | bool | Явный режим, не путать с `ids` |
| `excluded_ids` | `Set<NodeId>` | Для «все, кроме» |

Правило: массовое действие всегда показывает фактическое количество и объём; UI не
подставляет «все» молча.

### ColumnState

| Поле | Тип | Правила |
|------|-----|---------|
| `id` | строка | Уникален на экран |
| `visible` | bool | Обязательно |
| `width_px` | u16 | 40..1200 |
| `order` | u16 | Уникален |
| `sortable` | bool | Из схемы экрана |

### OperationPreview

| Поле | Тип | Правила |
|------|-----|---------|
| `preview_id` | UUID | Одноразовый, TTL 10 минут |
| `kind` | enum (`delete`, `move`, `rename`, `permissions`, `shred`, `restore`) | Обязательно |
| `targets` | `OperationTarget[]` | Не пусто |
| `total_size` | u64 | ≥ 0, сумма целей |
| `delete_mode` | enum (`recycle_bin`, `quarantine`, `permanent`) | Обязателен для delete/shred |
| `reversible` | bool | Определяется Rust |
| `requires_elevation` | bool | Системные/защищённые пути |
| `warnings` | `MessageKey[]` | SSD, системный путь, большой объём |
| `expires_at` | timestamp | TTL |
| `confirmation_token` | строка | Одноразовый, привязан к preview |

Правила: нельзя выполнить операцию без `preview_id` + `confirmation_token`; при истечении —
новый предпросмотр; `targets` не могут выходить за пределы выбранной области.

### OperationProgress

| Поле | Тип | Правила |
|------|-----|---------|
| `operation_id` | UUID | Обязательно |
| `state` | enum (`preparing`, `running`, `cancelling`, `completed`, `partial`, `failed`, `cancelled`) | Обязательно |
| `processed` / `total` | u64 | `processed ≤ total`, если total известен |
| `current_target` | строка? | Полный путь |
| `bytes_per_second` | u64? | ≥ 0 |
| `errors` | `UiError[]` | До 100 в UI, полный список в журнале |
| `undo_available` | bool | Только для обратимых |
| `journal_entry_id` | UUID? | Ссылка на журнал |

### OperationTarget

| Поле | Тип | Правила |
|------|-----|---------|
| `path` | строка | Полный путь, ≤ 32 000 |
| `kind` | enum | file/directory/link |
| `size` | u64 | ≥ 0 |
| `accessible` | bool | false — пометить и не выполнять молча |
| `is_system` | bool | Требует elevation/подтверждения |

### JournalEntryView

| Поле | Тип | Правила |
|------|-----|---------|
| `id` | UUID | Обязательно |
| `occurred_at` | timestamp | Обязательно |
| `user` | строка | Локальный пользователь |
| `kind` | enum | Совпадает с операцией |
| `targets_count` | u32 | ≥ 0 |
| `total_size` | u64 | ≥ 0 |
| `result` | enum (`success`, `partial`, `failed`, `cancelled`) | Обязательно |
| `reversible` | bool | Для undo/restore |
| `quarantine_ids` | UUID[] | Для восстановления |
| `error` | `UiError?` | При неуспехе |

### UiSettings

| Поле | Тип | По умолчанию | Правила |
|------|-----|--------------|---------|
| `theme` | enum (`system`, `dark`, `light`) | `system` | Системная — первый запуск |
| `language` | enum (`ru`, `en`) | `ru` | Или язык ОС, если поддерживается |
| `density` | enum (`comfortable`, `compact`) | `comfortable` | Влияет на высоту строк |
| `units` | enum (`binary`, `decimal`) | `binary` | KiB/MiB vs KB/MB |
| `reduced_motion` | enum (`system`, `on`, `off`) | `system` | Системное уважается |
| `last_screen` | `Screen.id?` | null | Не влияет на запуск: всегда «Обзор» |
| `sidebar_collapsed` | bool | false | — |
| `column_layouts` | map<Screen.id, ColumnState[]> | {} | Версия схемы |
| `saved_filters` | map<Screen.id, FilterSet[]> | {} | ≤ 20 на экран |
| `schema_version` | u16 | 1 | Миграция |

### WindowState

| Поле | Тип | Правила |
|------|-----|---------|
| `width` / `height` | u32 | ≥ 800×600, ≤ 16 384 |
| `x` / `y` | i32 | В пределах виртуального экрана; иначе центр |
| `maximized` | bool | — |
| `monitor_id` | строка? | Для multi-monitor |

### UiError

| Поле | Тип | Правила |
|------|-----|---------|
| `code` | строка | `E_*`, стабильный |
| `category` | enum (`validation`, `access`, `io`, `operation`, `internal`, `cancel`, `stale`) | Обязательно |
| `message_key` | `MessageKey` | Локализуемый |
| `details` | строка? | Без секретов; ≤ 4 КиБ |
| `retryable` | bool | Влияет на кнопку «Повторить» |
| `task_id` / `journal_entry_id` | UUID? | Связь с задачей/журналом |

### Toast

| Поле | Тип | Правила |
|------|-----|---------|
| `id` | UUID | — |
| `severity` | enum (`info`, `success`, `warning`, `error`) | — |
| `message_key` | `MessageKey` | — |
| `action` | enum (`none`, `open_journal`, `undo`, `retry`) | — |
| `persistent` | bool | true для ошибок, требующих решения |

### Message / MessageKey

| Поле | Тип | Правила |
|------|-----|---------|
| `key` | строка | `dot.case`, уникален |
| `ru` | строка | Не пустая |
| `en` | строка | Не пустая, без кириллицы |
| `placeholders` | массив строк | Совпадают в RU/EN |
| `context` | строка? | Комментарий переводчика |

Правило: ключи в RU и EN совпадают; генератор создаёт union `MessageKey`; missing-key — ошибка
сборки/теста.

### DesignToken / Theme

| Поле `DesignToken` | Тип | Правила |
|--------------------|-----|---------|
| `name` | строка | `--fs-*` |
| `category` | enum (`color`, `space`, `radius`, `type`, `motion`, `shadow`, `border`) | — |
| `dark` / `light` | строка | Валидное CSS-значение |
| `usage` | строка | Назначение |

`Theme` = `resolved` (`dark`/`light`) + `preference` (`system`/`dark`/`light`) + набор токенов
и палитра графиков и прогресс-баров дерева. Правило: продуктовые компоненты используют только
`var(--fs-*)`.

### ScreenshotBaseline

| Поле | Тип | Правила |
|------|-----|---------|
| `id` | UUID | — |
| `screen_id` | `Screen.id` | — |
| `theme` | enum (`dark`, `light`) | — |
| `dpi` | enum (`100`, `125`, `150`, `200`) | — |
| `language` | enum (`ru`, `en`) | — |
| `viewport` | `{w,h}` | Фиксированный |
| `image_path` | строка | В репозитории/LFS |
| `tolerance` | f32 | 0..1, по умолчанию 0.01 |
| `updated_at` | дата | — |

### ParityMatrix

| Поле | Тип | Правила |
|------|-----|---------|
| `screen_id` | `Screen.id` | — |
| `capability` | строка | Пользовательская возможность |
| `cli_command` | строка | Существующая CLI-команда |
| `tests` | строка[] | Unit/E2E/visual |
| `screenshot_id` | UUID? | Для экрана |
| `status` | enum (`todo`, `ported`, `verified`, `blocked`) | verified только после приёмки |

## 2. Связи

```text
Screen 1 ─── 1 ScreenState
Screen 1 ─── * ColumnState
Screen 1 ─── * ScreenshotBaseline
Screen 1 ─── 1 ParityMatrix

ScanTask 1 ─── 1 TaskProgress
ScanTask 1 ─── * TreePage (generation)

TreePage 1 ─── * NodeRow
NodeRow 1 ─── 0..1 NodeProgress

OperationPreview 1 ─── * OperationTarget
OperationPreview 1 ─── 1 OperationProgress (после подтверждения)
OperationProgress 1 ─── 0..1 JournalEntryView
JournalEntryView 1 ─── * QuarantineItemView

UiSettings 1 ─── * ColumnState
UiSettings 1 ─── * FilterSet
WindowState 1 ─── 1 UiSettings (отдельный файл рядом)
```

## 3. Правила целостности

1. **Один источник истины.** Размеры, суммы, результаты операций и журнал приходят из Rust;
   TypeScript не пересчитывает их и не «дополняет» отсутствующие значения.
2. **Поколения.** Любая страница привязана к `generation`; после нового сканирования старые
   страницы инвалидируются, а `stale_cursor` заставляет UI начать с первой страницы.
3. **Курсоры.** Курсор непрозрачен для UI, ограничен 512 символами и не смешивается с
   курсором другой сортировки/фильтра/поколения.
4. **Пагинация.** `page_size` 1..2000 (по умолчанию 500); `max_tiles` 100..10 000; полный
   набор строк/узлов никогда не передаётся одним ответом.
5. **Операции.** Нет выполнения без `preview_id` + `confirmation_token`; предпросмотр
   истекает; цели не выходят за выбранную область; системные пути требуют elevation.
6. **Выделение.** `select_all_in_filter` — отдельный режим; массовое действие всегда
   показывает фактическое количество и суммарный объём.
7. **Локализация.** Набор ключей RU/EN совпадает; плейсхолдеры совпадают; missing-key —
   ошибка теста/сборки.
8. **Темы.** Компоненты используют только токены; `resolved` тема вычисляется из preference и
   системной настройки; смена применяется без перезапуска.
9. **Хранение.** `UiSettings` и `WindowState` пишутся только Rust в `data`; UI не пишет в
   `localStorage`, `IndexedDB` или профиль WebView2.
10. **Окно.** Невалидные координаты/размеры корректируются до видимой области; состояние
    сохраняется после успешного изменения, а не на каждый кадр.
11. **Ошибки.** Любая ошибка имеет `code`, `category`, `message_key` и `retryable`; UI не
    показывает сырой текст Rust как единственное объяснение.
12. **Скриншоты.** Эталон существует для каждого экрана × темы × DPI × языка; без эталона
    визуальный тест не считается пройденным.

## 4. Состояния и переходы

### Сканирование

```text
queued → running → completed
                 ├→ partial   (ошибки доступа/отмена с частичными данными)
                 ├→ failed
                 └→ cancelling → cancelled
```

### Операция

```text
draft → previewing → awaiting_confirmation → running → completed
                                             ├→ partial
                                             ├→ failed
                                             └→ cancelling → cancelled
completed (reversible) → undo_available → undone
completed (quarantine) → restore_available → restored
```

### Тема

```text
preference = system | dark | light
resolved = preference == system ? OS theme : preference
OS theme changes → resolved recomputed → tokens swapped (no restart)
```

### Экран

```text
idle → loading → ready | empty | partial | error
ready → refreshing → ready | partial | error
ready/partial/error → loading (new volume/filter/generation)
```

### Язык

```text
ru ↔ en
state preserved; messages re-rendered; generated catalogs keyed identically
```

## 5. Ограничения и валидация на границе IPC

- Пути: полный путь ≤ 32 000 символов, канонизация, запрет выхода за выбранную область.
- Идентификаторы: `NodeId` в пределах текущего поколения; чужие id → `E_STALE_NODE`.
- Перечисления: неизвестное значение → `E_VALIDATION`.
- Размеры: u64, неотрицательные; переполнение → `E_VALIDATION`.
- Строки: имена ≤ 32 000, фильтры ≤ 256, курсоры ≤ 512, ключи сообщений ≤ 128.
- Дерево: `page_size` ≤ 2000, `max_depth` ≤ 12; доли узлов (`share_of_parent`/`share_of_volume`) в
  диапазоне 0..1.
- Предпросмотр: TTL, одноразовый токен, непустой список целей, `total_size` = сумма целей.
- Настройки: `schema_version` поддерживается; неизвестные поля сохраняются, но не ломают
  чтение; миграция идемпотентна.
- Окно: минимальный размер 800×600; координаты вне видимой области корректируются.
