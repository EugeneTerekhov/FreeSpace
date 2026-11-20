# Контракт UI: токены, компоненты, экраны и доступность

**Feature**: `002-native-ui-redesign` | **Date**: 2026-11-16

Контракт фиксирует продуктовый язык интерфейса. Он обязателен для 11 экранов и проверяется
тестами: жёсткие цвета, строки вне каталога локализации, отсутствие состояния или скриншота
считаются дефектом.

## 1. Дизайн-токены

Токены — CSS custom properties в `ui/src/styles/tokens.css`. Компоненты используют только
`var(--fs-*)`; значения ниже — стартовые и уточняются визуальными тестами без изменения имён.

### 1.1. Цвета

| Токен | Тёмная тема | Светлая тема | Назначение |
|-------|-------------|--------------|------------|
| `--fs-bg` | `#344b52` | `#e8e1db` | Фон окна |
| `--fs-surface` | `#3a535a` | `#f2ede9` | Основные поверхности |
| `--fs-surface-raised` | `#405d65` | `#f7f3f0` | Карточки, панели |
| `--fs-surface-overlay` | `#4a6b74` | `#dcd5cc` | Меню, поповеры, hover-строки |
| `--fs-line` | `rgb(250 232 218 / 12%)` | `rgb(54 46 35 / 12%)` | Разделители и границы |
| `--fs-line-strong` | `rgb(250 232 218 / 20%)` | `rgb(54 46 35 / 22%)` | Активные границы |
| `--fs-text` | `#fae8da` | `#362e23` | Основной текст |
| `--fs-text-muted` | `#cfd3ca` | `#5d5935` | Вторичный текст |
| `--fs-text-subtle` | `#c6d3d2` | `#636138` | Подписи, подсказки |
| `--fs-accent` | `#fae8da` | `#5d5935` | Заливки, активные элементы |
| `--fs-accent-strong` | `#fae8da` | `#5d5935` | Акцентный текст, иконки, фокус |
| `--fs-accent-ink` | `#344b52` | `#e8e1db` | Текст на акцентной заливке |
| `--fs-success` | `#b8dcc0` | `#2f6b3f` | Успех |
| `--fs-danger` | `#f5c4b4` | `#a8442f` | Ошибка, опасное действие |
| `--fs-warning` | `#edcf92` | `#7a5c12` | Предупреждение |
| `--fs-info` | `#b9d6da` | `#2f5f6b` | Информация |
| `--fs-focus` | `#cfd3ca` | `#6d6c3c` | Кольцо фокуса |
| `--fs-selection` | `rgb(250 232 218 / 16%)` | `rgb(169 172 93 / 22%)` | Выделенная строка |

Палитра — пять якорей на тему (plan.md §2.1.1): тёмная `#344b52 → #fae8da`, светлая
`#362E23 → #E8E1DB`. Производные оттенки — `color-mix(in oklch, …)`; в темах нет чистых
`#000000`/`#FFFFFF` на больших поверхностях.

Правила:

- Любая пара «текст — фон» проверяется автоматическим контраст-тестом (WCAG 2.2 AA:
  4.5:1 для обычного текста, 3:1 для крупного и нетекстовых элементов).
- Статус никогда не передаётся только цветом: добавляются текст, иконка или форма.
- Прозрачные слои не используются для основного текста.
- Прогресс-бар дерева — полупрозрачная заливка строки узла (`--fs-tree-fill`,
  `--fs-tree-fill-strong`), доля которой пропорциональна размеру узла относительно родителя;
  выбранная строка выделяется акцентом и кольцом.
- Декоративные градиенты, «aurora» и подсветки удалены: цвет означает состояние или действие
  (принцип 3).

### 1.2. Типографика

| Токен | Значение |
|-------|----------|
| `--fs-font-mono` | `"JetBrains Mono", "Cascadia Mono", "Consolas", ui-monospace, monospace` |
| `--fs-font-sans` / `--fs-font-display` | те же, что `--fs-font-mono` (единый шрифт) |
| `--fs-text-2xs` | `11px/15px` |
| `--fs-text-xs` | `13px/17px` |
| `--fs-text-sm` | `15px/20px` |
| `--fs-text-base` | `16px/22px` |
| `--fs-text-md` | `19px/26px` |
| `--fs-text-lg` | `23px/30px` |
| `--fs-text-xl` | `27px/34px` |
| `--fs-text-2xl` | `33px/40px` |
| `--fs-text-3xl` | `40px/46px` |
| `--fs-text-4xl` | `48px/54px` |
| `--fs-weight-regular` | `400` |
| `--fs-weight-medium` | `500` |
| `--fs-weight-semibold` | `600` |
| `--fs-weight-bold` | `700` |

Правила: единый моноширинный шрифт с табличными цифрами для путей, размеров и чисел; один `h1`
на экран (`2xl`) — заголовок оболочки; надзаголовки секций — `2xs`, uppercase, letter-spacing
`0.08em`; шрифт поставляется локально или берётся из системного моноширинного стека, без CDN.

### 1.3. Отступы, радиусы, тени, слои

| Группа | Токены |
|--------|--------|
| Отступы | `--fs-space-1: 4px`, `2: 8px`, `3: 12px`, `4: 16px`, `5: 20px`, `6: 24px`, `8: 32px`, `10: 40px` |
| Радиусы | `--fs-radius-sm: 4px`, `md: 6px`, `lg: 8px`, `pill: 999px` |
| Границы | `--fs-border: 1px solid var(--fs-line)`; сильная — `--fs-line-strong` |
| Тени | только функциональные: `--fs-shadow-overlay` для диалогов/оверлеев; декоративные удалены |
| Слои | `--fs-z-sticky: 10`, `dropdown: 100`, `overlay: 200`, `dialog: 300`, `toast: 400`, `tooltip: 500` |
| Плотность | `--fs-row-comfortable: 36px`, `--fs-row-compact: 30px`, `--fs-header: 56px`, `--fs-sidebar: 240px`, `--fs-sidebar-collapsed: 64px`, `--fs-status: 28px` |

### 1.4. Motion

| Токен | Значение |
|-------|----------|
| `--fs-motion-fast` | `120ms` |
| `--fs-motion-base` | `180ms` |
| `--fs-motion-hover` | `120ms` |
| `--fs-motion-panel` | `180ms` |
| `--fs-motion-dialog` | `180ms` |
| `--fs-ease` | `cubic-bezier(.2,.8,.2,1)` |
| `--fs-motion-reduced` | `0ms` |

Правила: переходы только для функциональных изменений состояния (открытие/закрытие, hover,
фокус); никаких каскадных появлений, count-up и «приподнимания» кнопок. При
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
- Дерево и графики перерисовываются с новой палитрой в том же кадре, без мигания.

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
| `DeleteObjectDialog` | open, busy, error | `role="dialog"`, компактный вопрос, имя, тип, размер и дата изменения |
| `Toast` | info, success, warning, error, with-action | `aria-live`, не перекрывает диалог |
| `Banner` | info, warning, error, offline | `role="alert"` для ошибок |
| `EmptyState` | no-data, no-volume, no-scan, no-results | Заголовок, объяснение, действие |
| `ErrorState` | recoverable, fatal, with-details | Код, сообщение, «Повторить»/«Журнал» |
| `Loading` / `Skeleton` | page, section, row | `aria-busy`, не бесконечный индикатор |
| `TreeRow` | collapsed, expanded, selected, loading, error | `role="treegrid"`-строка, иконка типа, `aria-expanded`, встроенный прогресс-бар |
| `Legend` | categorical, size | Связь цветов с подписями |
| `Chart` | loading, ready, empty, error | Текстовая альтернатива/таблица |
| `Breadcrumbs` | default, overflow | `nav`, `aria-current` |
| `Pagination` | cursor, end, loading | «Загрузить ещё»/виртуальный скролл |
| `SplitPane` | default, collapsed | Клавиатурное изменение размера |
| `ContextMenu` | closed, open, disabled-item | `role="menu"`, стрелки, Esc; пункты строки дерева: открыть в Проводнике, скопировать путь, переименовать, удалить в Корзину |
| `CommandPalette` | closed, open, filtered, empty | `role="dialog"`, `aria-activedescendant`, Ctrl+K |
| `Kbd` | default | Визуальная подсказка, не единственный способ |

Правила: компонент без состояния ошибки/пустоты не считается готовым; интерактивные
компоненты имеют `:focus-visible`; минимальная цель нажатия — 24×24 px (рекомендуется 32×32).

## 4. Экраны

Все 11 экранов используют общий `Screen`-каркас: заголовок, контекст, основное содержимое,
панель деталей (если нужна), состояния загрузки/пусто/ошибки/частичного результата. Экраны
сгруппированы в 6 групп сайдбара; поглощённые экраны остаются вкладками/разделами и строками
матрицы паритета.

| ID | Группа | Назначение | Основные данные | Действия | CLI-аналог |
|----|--------|------------|-----------------|----------|------------|
| `overview` | overview | Обзор томов и встроенное сканирование | `volumes_list`, `scan_start/status`, `scan-progress`, снимки | Анализировать, отмена, сравнить | `drives`, `scan` |
| `tree` | analysis | Иерархия размеров + детали размера | `tree_root/children/search`, `size_details` | Раскрыть, фильтр, сортировка, выбрать | `tree`, `filter` |
| `analysis` | analysis | Крупнейшие файлы, типы, владельцы | `top_files_page`, `types_summary`, `owners_page` | N, фильтр, вкладки | `top`, `types`, `owners` |
| `duplicates` | analysis | Группы дубликатов | `duplicates_*` | Поиск, правило, выбрать копии | `duplicates` |
| `cleanup` | cleanup | Рекомендации очистки и снимки | `cleanup_candidates`, `snapshots_*` | Фильтр по причине (И), вкладки, предпросмотр | `cleanup suggest`, `cleanup empty-dirs`, `snapshot create/diff` |
| `journal` | operations | История операций | `journal_page` | Открыть, отменить, восстановить | `delete`/`shred`/`rename`/`acl` (записи) |
| `shred` | operations | Безвозвратное уничтожение | `shred_preview/start/status` | Метод, проходы, подтверждение | `shred` |
| `monitor` | operations | Мониторинг записи | `monitor_*`, `monitor-tick` | Старт/стоп, порог, фильтр, сортировка, открыть папку файла/EXE | `monitor` |
| `inspect` | tools | Метаданные, архивы, разблокировка, длинные пути | `metadata_get`, `archives_entries_page`, `unlock_locks`, `long_paths_page` | Вкладки, фильтр, снять блокировку | `filter --metadata`, `archives`, `unlock`, `rename`, `delete` |
| `fileops` | tools | Переименование и права | `rename_preview/apply`, `permissions_get/preview/apply` | Вкладки, предпросмотр, применить | `rename`, `acl`, `attrs` |
| `settings` | system | Настройки, elevation, о программе | `settings_get/set`, `elevation_status/request`, `about_info` | Тема, язык, единицы, elevation, версия | `config get/set`, `--version` |

Экспорт отчёта (`export`) — глобальное действие, доступное на экранах, порождающих данные
(Дерево, Анализ, Дубликаты, Журнал), а не отдельный экран.

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
| Навигация | `Ctrl+1…9` — первые экраны группы, `Alt+←` — назад, `Alt+→` — вперёд |
| Поиск | `Ctrl+F` — фокус в поиск, `Esc` — очистить/закрыть |
| Дерево | `↑/↓`, `←/→` — свернуть/развернуть, `Home/End`, `Enter` — открыть, `Space` — выбрать, `Shift+F10` — контекстное меню |
| Таблица | `↑/↓`, `PageUp/PageDown`, `Home/End`, `Ctrl+A`, `Shift+↑/↓` — диапазон, `Space` — выбрать |
| Диалог | `Tab`/`Shift+Tab` внутри, `Esc` — закрыть, `Enter` — только на сфокусированной кнопке |
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
- 100/125/150/200% DPI: проверяются все 11 экранов; цели нажатия не меньше 24×24 px.
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

- Полная матрица: 11 экранов × 2 темы × 4 DPI × 2 языка = 176 эталонов.
- PR-набор: 8 критических экранов × 2 темы × 100/150% × 2 языка + все 11 экранов в тёмной
  теме, 100%, RU.
- Viewport: 1440×900 (основной), 1280×800 (узкий), 1920×1080 (широкий); для DPI — эмуляция
  `deviceScaleFactor`.
- Шрифты фиксированы системным стеком; различия шрифтов на CI допускаются только в
  контролируемом образе Windows.
- Допуск по умолчанию 0.01 (1% пикселей); для графиков — 0.02.
- Эталон обновляется только осознанно, с ревью diff; отсутствие эталона — падение теста.
- Для каждого падения сохраняются screenshot, diff и trace.

## 8. Матрица паритета (шаблон)

| Экран | Возможность | CLI-команда | Тест | Скриншот | Статус |
|-------|-------------|-------------|------|----------|--------|
| `overview` | Список томов и встроенное сканирование | `drives`, `scan` | Rust + E2E | `overview-dark-100-ru` | todo |
| `tree` | Иерархия, поиск, фильтр, детали размера | `tree`, `filter` | Vitest + E2E | `tree-dark-100-ru` | todo |
| `analysis` | Крупнейшие файлы, типы, владельцы | `top`, `types`, `owners` | Vitest + E2E | `analysis-dark-100-ru` | todo |
| … | … | … | … | … | todo |

Матрица заполняется до 11 строк экранов; поглощённые экраны (Сканирование, Карта, Детали
размера, Владельцы, Снимки, Экспорт, Elevation, О программе) остаются отдельными строками
возможностей. `verified` ставится только после приёмки экрана, теста, скриншота и сверки
CLI-поведения.
