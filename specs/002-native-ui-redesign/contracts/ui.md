# Контракт UI: токены, компоненты, экраны и доступность

**Feature**: `002-native-ui-redesign` | **Date**: 2026-11-16

Контракт фиксирует продуктовый язык интерфейса. Он обязателен для 24 экранов и проверяется
тестами: жёсткие цвета, строки вне каталога локализации, отсутствие состояния или скриншота
считаются дефектом.

## 1. Дизайн-токены

Токены — CSS custom properties в `ui/src/styles/tokens.css`. Компоненты используют только
`var(--fs-*)`; значения ниже — стартовые и уточняются визуальными тестами без изменения имён.

### 1.1. Цвета

| Токен | Тёмная тема | Светлая тема | Назначение |
|-------|-------------|--------------|------------|
| `--fs-bg` | `#07060C` | `#F6F2EC` | Фон окна |
| `--fs-surface` | `#110E1B` | `#FFFFFF` | Основные поверхности |
| `--fs-surface-raised` | `#171327` | `#FFFDFA` | Карточки, панели |
| `--fs-surface-overlay` | `#1F1930` | `#F1EAE2` | Меню, поповеры, hover-строки |
| `--fs-line` | `rgba(255,255,255,.075)` | `rgba(17,14,27,.12)` | Разделители и границы |
| `--fs-line-strong` | `rgba(255,255,255,.14)` | `rgba(17,14,27,.22)` | Активные границы |
| `--fs-text` | `#F4EFE9` | `#1B1626` | Основной текст |
| `--fs-text-muted` | `#A9A1B6` | `#5A5364` | Вторичный текст |
| `--fs-text-subtle` | `#736C82` | `#7A7285` | Подписи, подсказки |
| `--fs-accent` | `#FF8A3D` | `#E06A1C` | Заливки, активные элементы |
| `--fs-accent-strong` | `#FFB56B` | `#A8430A` | Акцентный текст, иконки, фокус |
| `--fs-accent-ink` | `#1A1208` | `#1A1208` | Текст на акцентной заливке |
| `--fs-violet` | `#A06BFF` | `#6D3FD1` | Второй акцент, графики |
| `--fs-success` | `#7EE8A9` | `#1F7A4D` | Успех |
| `--fs-danger` | `#F87171` | `#B42318` | Ошибка, опасное действие |
| `--fs-warning` | `#FBBF24` | `#8A5A00` | Предупреждение |
| `--fs-info` | `#7DD3FC` | `#0B6A8A` | Информация |
| `--fs-focus` | `#FFB56B` | `#A8430A` | Кольцо фокуса |
| `--fs-selection` | `rgba(255,138,61,.16)` | `rgba(224,106,28,.14)` | Выделенная строка/плитка |

Правила:

- Любая пара «текст — фон» проверяется автоматическим контраст-тестом (WCAG 2.2 AA:
  4.5:1 для обычного текста, 3:1 для крупного и нетекстовых элементов).
- Статус никогда не передаётся только цветом: добавляются текст, иконка или форма.
- Прозрачные слои не используются для основного текста.
- Палитра Treemap — отдельные токены `--fs-tm-1…--fs-tm-10` для каждой темы; соседние
  категории различимы, подписи читаемы.

### 1.2. Типографика

| Токен | Значение |
|-------|----------|
| `--fs-font-sans` | `"Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif` |
| `--fs-font-display` | `"Segoe UI Variable Display", "Segoe UI", sans-serif` |
| `--fs-font-mono` | `"Cascadia Mono", "Consolas", monospace` |
| `--fs-text-xs` | `11px/16px` |
| `--fs-text-sm` | `12px/18px` |
| `--fs-text-base` | `13px/20px` |
| `--fs-text-md` | `14px/22px` |
| `--fs-text-lg` | `16px/24px` |
| `--fs-text-xl` | `20px/28px` |
| `--fs-text-2xl` | `24px/32px` |
| `--fs-text-3xl` | `32px/40px` |
| `--fs-text-4xl` | `40px/48px` |
| `--fs-weight-regular` | `400` |
| `--fs-weight-medium` | `500` |
| `--fs-weight-semibold` | `600` |
| `--fs-weight-bold` | `700` |

Правила: основной текст — `base`/`md`; заголовки экранов — `xl`/`2xl`; таблицы — `sm`/`base`;
моноширинный — только пути, хеши, размеры в байтах и коды. Никаких веб-шрифтов и CDN.

### 1.3. Отступы, радиусы, тени, слои

| Группа | Токены |
|--------|--------|
| Отступы | `--fs-space-1: 4px`, `2: 8px`, `3: 12px`, `4: 16px`, `5: 20px`, `6: 24px`, `8: 32px`, `10: 40px` |
| Радиусы | `--fs-radius-sm: 6px`, `md: 8px`, `lg: 10px`, `xl: 14px`, `2xl: 20px`, `pill: 999px` |
| Границы | `--fs-border: 1px solid var(--fs-line)`; сильная — `--fs-line-strong` |
| Тени | `--fs-shadow-1: 0 1px 2px rgba(0,0,0,.24)`, `--fs-shadow-2: 0 8px 24px rgba(0,0,0,.32)`, светлая тема — мягче |
| Слои | `--fs-z-sticky: 10`, `dropdown: 100`, `overlay: 200`, `dialog: 300`, `toast: 400`, `tooltip: 500` |
| Плотность | `--fs-row-comfortable: 36px`, `--fs-row-compact: 30px`, `--fs-header: 56px`, `--fs-sidebar: 240px`, `--fs-sidebar-collapsed: 64px`, `--fs-status: 28px` |

### 1.4. Motion

| Токен | Значение |
|-------|----------|
| `--fs-motion-hover` | `120ms` |
| `--fs-motion-panel` | `180ms` |
| `--fs-motion-dialog` | `240ms` |
| `--fs-ease` | `cubic-bezier(.2,.8,.2,1)` |
| `--fs-motion-reduced` | `0ms` |

Правила: анимация не скрывает критическое предупреждение и не блокирует ввод. При
`prefers-reduced-motion: reduce` или настройке `reduced_motion=on` необязательные переходы
отключаются; функциональные изменения состояния остаются мгновенными и понятными.

## 2. Контракт темы

- Preference: `system | dark | light`; default — `system`.
- Resolved theme: `preference === system ? OS theme : preference`.
- Атрибуты на `<html>`: `data-theme="dark|light"`, `data-density="comfortable|compact"`,
  `data-reduced-motion="on|off"`, `lang="ru|en"`, `style="color-scheme: dark|light"`.
- Тема применяется до первой отрисовки: `boot.js` в `<head>` читает
  `window.__FREESPACE_BOOT__` (инъекция Rust) или `prefers-color-scheme`.
- Смена системной темы применяется без перезапуска через `matchMedia` listener; сохранённая
  preference не перезаписывается.
- При смене темы не перезагружаются данные, не сбрасываются фильтры, выделение и прокрутка.
- Treemap и графики перерисовываются с новой палитрой в том же кадре, без мигания.

## 3. Компоненты продуктового кита

| Компонент | Обязательные состояния | Доступность |
|-----------|------------------------|-------------|
| `AppShell` | ready, loading, error | Landmark-роли `banner`, `navigation`, `main`, `contentinfo` |
| `Sidebar` | expanded, collapsed, active, badge | `<nav>`, `aria-current="page"`, клавиатура |
| `Header` | default, with-back, with-actions | `<header>`, заголовок `h1` |
| `StatusBar` | idle, task, error, offline | `role="status"`, `aria-live="polite"` |
| `Button` | default, hover, focus, active, disabled, loading, danger, ghost | `<button>`, `aria-busy`, не только цвет |
| `IconButton` | default, focus, active, disabled | `aria-label`, tooltip |
| `Field` | default, focus, error, disabled, readonly | `<label>`, `aria-describedby`, `aria-invalid` |
| `SearchField` | default, focus, with-clear, loading | `role="searchbox"`, Esc очищает |
| `Select` | default, open, disabled, error | Нативный `select` или combobox с ARIA |
| `Combobox` | collapsed, open, filtered, empty | `aria-expanded`, `aria-controls`, активный потомок |
| `Segmented` | default, selected, disabled | `role="tablist"`/radiogroup, стрелки |
| `Tabs` | default, selected, disabled | `role="tablist"`, `aria-selected`, стрелки |
| `Toggle` | on, off, disabled | `role="switch"`, `aria-checked` |
| `Checkbox` | unchecked, checked, indeterminate, disabled | Нативный input |
| `Radio` | unchecked, checked, disabled | Нативный input + fieldset/legend |
| `Chip` | default, selected, removable | Кнопка/`aria-pressed` |
| `Badge` | neutral, success, warning, danger, info | Текст, не только цвет |
| `Tooltip` | hidden, visible | `aria-describedby`, не единственный носитель смысла |
| `Progress` | determinate, indeterminate, error, cancelled | `role="progressbar"`, `aria-valuenow` |
| `Meter` | normal, warning, danger | `role="meter"`, текстовая подпись |
| `DataTable` | loading, ready, empty, partial, error, selected | `role="grid"`/table, `aria-rowcount`, сортировка |
| `TreeTable` | collapsed, expanded, selected, loading, error | `role="treegrid"`, стрелки, `aria-expanded` |
| `List` | loading, ready, empty, error | `<ul>`/`role="listbox"` по контексту |
| `Card` | default, interactive, selected | Заголовок и описание |
| `StatCard` | default, loading, warning | Число + подпись + тренд |
| `SummaryCard` | default, selected, loading | Bento-сетка только на дашборде |
| `Dialog` | closed, open, busy, error | `role="dialog"`, `aria-modal`, focus trap, Esc |
| `ConfirmDialog` | default, destructive, busy, error | Заголовок, последствия, явная кнопка |
| `Toast` | info, success, warning, error, with-action | `aria-live`, не перекрывает диалог |
| `Banner` | info, warning, error, offline | `role="alert"` для ошибок |
| `EmptyState` | no-data, no-volume, no-scan, no-results | Заголовок, объяснение, действие |
| `ErrorState` | recoverable, fatal, with-details | Код, сообщение, «Повторить»/«Журнал» |
| `Loading` / `Skeleton` | page, section, row | `aria-busy`, не бесконечный индикатор |
| `Treemap` | loading, ready, empty, error, zoomed | `role="img"` + текстовая сводка, клавиатура |
| `Legend` | categorical, size | Связь цветов с подписями |
| `Chart` | loading, ready, empty, error | Текстовая альтернатива/таблица |
| `Breadcrumbs` | default, overflow | `nav`, `aria-current` |
| `Pagination` | cursor, end, loading | «Загрузить ещё»/виртуальный скролл |
| `SplitPane` | default, collapsed | Клавиатурное изменение размера |
| `ContextMenu` | closed, open, disabled-item | `role="menu"`, стрелки, Esc |
| `CommandPalette` | closed, open, filtered, empty | `role="dialog"`, `aria-activedescendant`, Ctrl+K |
| `Kbd` | default | Визуальная подсказка, не единственный способ |

Правила: компонент без состояния ошибки/пустоты не считается готовым; интерактивные
компоненты имеют `:focus-visible`; минимальная цель нажатия — 24×24 px (рекомендуется 32×32).

## 4. Экраны

Все 24 экрана используют общий `Screen`-каркас: заголовок, контекст, основное содержимое,
панель деталей (если нужна), состояния загрузки/пусто/ошибки/частичного результата.

| ID | Партия | Назначение | Основные данные | Действия | CLI-аналог |
|----|--------|------------|-----------------|----------|------------|
| `dashboard` | core | Обзор томов и динамики | `volumes_list`, снимки | Анализировать, открыть том, сравнить | `drives` |
| `scan` | core | Запуск и прогресс анализа | `scan_start/status`, `scan-progress` | Старт, отмена, повтор | `scan` |
| `tree` | core | Иерархия размеров | `tree_root/children/search` | Раскрыть, фильтр, сортировка, выбрать | `tree`, `filter` |
| `treemap` | core | Визуальная карта | `treemap_tiles` | Погрузиться, назад, раскраска, выбрать | `tree` |
| `top_files` | analysis | Крупнейшие файлы | `top_files_page` | N, фильтр, перейти в дерево | `top` |
| `types` | analysis | Распределение по типам | `types_summary` | Выбрать тип, фильтр | `types` |
| `duplicates` | analysis | Группы дубликатов | `duplicates_*` | Поиск, правило, выбрать копии | `duplicates` |
| `cleanup` | analysis | Рекомендации очистки | `cleanup_candidates` | Выбрать категорию, предпросмотр | `cleanup suggest`, `cleanup empty-dirs` |
| `snapshots` | analysis | Снимки и сравнение | `snapshots_*` | Создать, сравнить, удалить | `snapshot create/diff` |
| `journal` | filesystem | История операций | `journal_page` | Открыть, отменить, восстановить | `delete`/`shred`/`rename`/`acl` (записи) |
| `size_details` | filesystem | Точные метрики размера | `size_details` | Сменить тип размера, копировать | `tree`, `filter` |
| `long_paths` | filesystem | Объекты > 260 символов | `long_paths_page` | Открыть, переименовать, удалить | `rename`, `delete`, `tree` |
| `rename` | filesystem | Массовое переименование | `rename_preview/apply` | Шаблон, предпросмотр, применить | `rename` |
| `permissions` | filesystem | Права и атрибуты NTFS | `permissions_get/preview/apply` | Предпросмотр, применить | `acl`, `attrs` |
| `metadata` | inspection | Метаданные файла | `metadata_get` | Выбрать файл, фильтр | `filter --metadata` |
| `owners` | inspection | Распределение по владельцам | `owners_page` | Выбрать владельца, перейти к файлам | `owners` |
| `archives` | inspection | Содержимое архивов | `archives_entries_page` | Открыть, фильтр, экспорт списка | `archives` |
| `export` | inspection | Экспорт отчётов | `export_preview/start/status` | Формат, состав, запуск | `export` |
| `unlock` | inspection | Блокировки файла | `unlock_locks` | Найти, снять, перезагрузка | `unlock` |
| `shred` | settings | Безвозвратное уничтожение | `shred_preview/start/status` | Метод, проходы, подтверждение | `shred` |
| `monitor` | settings | Мониторинг записи | `monitor_*`, `monitor-tick` | Старт/стоп, порог, фильтр | `monitor` |
| `elevation` | settings | Повышение прав | `elevation_status/request` | Запросить, объяснить причину | Системное действие; CLI-флаги elevation |
| `settings` | settings | Настройки приложения | `settings_get/set` | Тема, язык, единицы, плотность | `config get/set` |
| `about` | settings | Версия и лицензии | `about_info` | Копировать версию, открыть лицензии | `--version` |

### 4.1. Обязательные состояния экрана

Каждый экран обязан иметь:

1. `loading` — скелет/прогресс, не пустой экран.
2. `empty` — объяснение и действие (выбрать том, запустить анализ, изменить фильтр).
3. `error` — код, понятное сообщение, «Повторить», ссылка в журнал.
4. `partial` — видимая пометка о неполных данных и причинах (нет доступа, отмена).
5. `busy` — длительная операция с прогрессом и отменой, если отмена доступна.
6. `ready` — данные и действия.

### 4.2. Правила навигации и сохранения состояния

- Переключение экрана сохраняет фильтры, сортировку, выделение и `scroll_anchor`.
- Смена тома сбрасывает данные зависимых экранов и показывает подтверждение, если есть
  несохранённые изменения.
- Возврат на экран не перезагружает данные, если `generation` не изменился.
- Глубокие ссылки открывают экран и объект; если объект исчез — пустое состояние с причиной.

## 5. Контракт доступности

### 5.1. Клавиатура

| Контекст | Клавиши |
|----------|---------|
| Оболочка | `Tab`/`Shift+Tab`, `Ctrl+K` — палитра команд, `Ctrl+,` — настройки, `F1` — справка |
| Навигация | `Ctrl+1…9` — первые экраны партии, `Alt+←` — назад, `Alt+→` — вперёд |
| Поиск | `Ctrl+F` — фокус в поиск, `Esc` — очистить/закрыть |
| Дерево | `↑/↓`, `←/→` — свернуть/развернуть, `Home/End`, `Enter` — открыть, `Space` — выбрать |
| Таблица | `↑/↓`, `PageUp/PageDown`, `Home/End`, `Ctrl+A`, `Shift+↑/↓` — диапазон, `Space` — выбрать |
| Диалог | `Tab`/`Shift+Tab` внутри, `Esc` — закрыть, `Enter` — только на сфокусированной кнопке |
| Карта | `Tab` — фокус, `Enter` — погрузиться, `Backspace`/`Alt+←` — назад, стрелки — соседние плитки |
| Уведомления | `F6` — перейти к уведомлениям, `Esc` — закрыть необязательное |

### 5.2. Фокус и объявления

- Фокус видим на всех фонах (2 px, offset 2 px).
- Диалог удерживает фокус и возвращает его инициатору.
- `aria-live="polite"` — прогресс и статусы; `aria-live="assertive"` — ошибки, требующие
  решения.
- Прогресс объявляется не чаще 1 раза в 2 секунды, чтобы не перекрывать речь.
- Декоративные иконки `aria-hidden="true"`; у иконочных кнопок есть `aria-label`.
- Таблицы имеют заголовки, `aria-rowcount`/`aria-colcount`; дерево — `aria-level`,
  `aria-expanded`, `aria-selected`.
- Цвет не является единственным носителем статуса; ошибки имеют текст и иконку.

### 5.3. Контраст, DPI и движение

- WCAG 2.2 AA: 4.5:1 обычный текст, 3:1 крупный текст и границы интерактивных элементов.
- 100/125/150/200% DPI: проверяются все 24 экрана; цели нажатия не меньше 24×24 px.
- `prefers-reduced-motion` и `prefers-contrast` учитываются; настройка reduced motion
  переопределяет системную только по явному выбору пользователя.

## 6. Контракт локализации

- Ключи: `dot.case`, латиница, например `nav.dashboard`, `scan.progress.current_path`.
- Плоские ключи генерируются из `locales/*.toml`; union `MessageKey` обязателен.
- Плейсхолдеры: `{name}`, `{count}`, `{size}`; набор и типы совпадают в RU/EN.
- Плюрализация: `{count} файлов` через правила языка, без склейки строк.
- Числа, даты и единицы — через `Intl.*`; единицы — `binary` (KiB/MiB) или `decimal`
  (KB/MB) по настройке.
- Запрещено: конкатенация переведённых фрагментов, строки в JSX, сообщения об ошибках
  только на английском.
- Missing key: английский резерв + ошибка теста; пустая строка запрещена.
- Смена языка не перезагружает приложение и не теряет состояние.

## 7. Контракт визуальной регрессии

- Полная матрица: 24 экрана × 2 темы × 4 DPI × 2 языка = 384 эталона.
- PR-набор: 8 критических экранов × 2 темы × 100/150% × 2 языка + все 24 экрана в тёмной
  теме, 100%, RU.
- Viewport: 1440×900 (основной), 1280×800 (узкий), 1920×1080 (широкий); для DPI — эмуляция
  `deviceScaleFactor`.
- Шрифты фиксированы системным стеком; различия шрифтов на CI допускаются только в
  контролируемом образе Windows.
- Допуск по умолчанию 0.01 (1% пикселей); для Treemap/графиков — 0.02.
- Эталон обновляется только осознанно, с ревью diff; отсутствие эталона — падение теста.
- Для каждого падения сохраняются screenshot, diff и trace.

## 8. Матрица паритета (шаблон)

| Экран | Возможность | CLI-команда | Тест | Скриншот | Статус |
|-------|-------------|-------------|------|----------|--------|
| `dashboard` | Список томов и размеров | `drives` | Rust + E2E | `dashboard-dark-100-ru` | todo |
| `scan` | Запуск/отмена/прогресс | `scan` | Rust + E2E | `scan-dark-100-ru` | todo |
| `tree` | Иерархия, поиск, фильтр | `tree`, `filter` | Vitest + E2E | `tree-dark-100-ru` | todo |
| `treemap` | Карта и погружение | `tree` | Vitest + visual | `treemap-dark-100-ru` | todo |
| … | … | … | … | … | todo |

Матрица заполняется до 24 строк; `verified` ставится только после приёмки экрана, теста,
скриншота и сверки CLI-поведения.
