# Доступность интерфейса 002 — клавиатура, фокус и экранный диктор

**Функция**: `002-native-ui-redesign` · **Дата**: 2026-11-16 · **Область**: US8 (T144–T152)

Документ фиксирует реализацию и проверку доступности нового интерфейса. Он отделяет то, что
проверено автоматически или юнит-тестами, от того, что требует ручного прогона с Narrator на
Windows. Все утверждения ниже проверяемы по указанным файлам и тестам.

## 1. Что реализовано

| Механизм | Файлы | Поведение |
|----------|-------|-----------|
| Ловушка фокуса диалогов | `ui/src/app/focus.ts`, `ui/src/components/Dialog.tsx` | Фокус ставится на `[data-autofocus]` или первый контрол, `Tab`/`Shift+Tab` зациклены, `Esc` закрывает (кроме `busy`), фокус возвращается инициатору |
| Видимый фокус | `focus.ts` (`focusWithRing`, `FOCUS_VISIBLE_ATTRIBUTE`), `DataTable.module.css`, `TreemapCanvas.module.css` | Кольцо 2 px с отрицательным offset на строках грида и канве карты; программный фокус сохраняет кольцо |
| Roving tabindex грида | `ui/src/components/DataTable.tsx` | Одна строка в порядке табуляции; `↑/↓`, `Home/End`, `PageUp/PageDown` двигают фокус, `Space` выбирает, `Enter` открывает |
| Дерево | `DataTable` (`tree`), `ui/src/screens/Tree.tsx` | `aria-level`, `aria-expanded`, `→` раскрывает каталог, `←` поднимается на уровень выше |
| Карта | `ui/src/treemap/keyboard.ts`, `TreemapCanvas.tsx`, `ui/src/screens/Treemap.tsx` | Пространственная навигация стрелками между плитками, `Home/End`, `Enter` — погружение, `Space` — выбор, `Esc`/`Backspace` — на уровень вверх; текстовая сводка выбранной плитки в `aria-live` |
| Skip link и landmark-и | `ui/src/app/AppShell.tsx`, ключи `a11y.*` | Первый Tab — «Перейти к основному содержимому» (`Skip to main content` в EN), затем `navigation`, `banner`, `main`, `contentinfo`; `main` получает фокус по Enter; подписи landmark-ов локализованы |
| Объявления прогресса | `ui/src/app/live.ts`, `OperationProgress.tsx`, `Feedback.tsx`, `AppShell.tsx` | `aria-live="polite"` для прогресса и `assertive` для ошибок; дедупликация, троттлинг 2 с, лимит 8 сообщений / 10 с |
| Не-цветовые статусы | `Feedback.tsx`, `DataTable.tsx`, `Tree.tsx`, `Treemap.tsx` | Иконки `ℹ/⚠/✕` у баннеров и ошибок, маркер `✓` у выбранной строки, текстовый бейдж «Нет доступа», легенда карты и строка выбранной плитки |
| Роли и состояния | `DataTable`, `Dialog`, `Choice`, `Fields`, `Feedback` | `grid`/`rowgroup`/`row`/`gridcell`, `aria-rowcount`, `aria-colcount`, `aria-sort`, `aria-selected`, `aria-busy`, `treegrid`-семантика строк, `switch`, `tablist`/`tab`, `dialog`+`aria-modal`, `progressbar`/`meter`, `aria-current` в навигации |

Порядок фокуса совпадает с DOM-порядком: skip link → навигация → шапка → основной контент →
статусная строка. Строки грида больше не добавляют по одному стопу каждая.

## 2. Политика объявлений экранного диктора

- Прогресс (`priority: 'progress'`) объявляется не чаще одного раза в 2 секунды
  (контракт `contracts/ui.md`, §5.2).
- Терминальные состояния и ошибки (`priority: 'status'`, `urgency: 'assertive'`) не
  отбрасываются троттлингом; ошибка, требующая решения, прерывает речь.
- Повтор идентичного текста не объявляется (дедупликация по последнему сообщению).
- Скользящее окно ограничивает вежливые сообщения 8 событиями за 10 секунд.
- Статусная строка оболочки обновляется визуально каждую секунду, но её `role="status"` имеет
  `aria-live="off"`; озвучивание идёт из отдельного скрытого региона с троттлингом, поэтому
  Narrator не перекрывает собственную речь.
- `OperationProgressView` объявляет фазу и процент, но не каждый кадр: повторные проценты
  отбрасываются.

## 3. Десять ключевых сценариев

Статус `авто` — покрыто автоматическими тестами; `вручную` — требуется прогон с Narrator на
Windows 11 (в этой среде Narrator недоступен, см. §6).

| № | Сценарий | Клавиатура | Авто-проверка | Narrator (вручную) |
|---|----------|-----------|---------------|--------------------|
| 1 | Запуск и переход по оболочке | `Tab`/`Shift+Tab`, skip link | `tests/unit/shell.test.tsx`, `tests/e2e/keyboard.spec.ts` | ожидает прогона |
| 2 | Переключение экранов из навигации | `Tab`, `Enter`, `aria-current` | `tests/unit/shell.test.tsx`, `tests/e2e/keyboard.spec.ts` | ожидает прогона |
| 3 | Палитра команд | `Ctrl+K`, `↑/↓`, `Home/End`, `Enter`, `Esc` | `tests/unit/palette.test.tsx`, `tests/unit/keyboard.test.tsx`, `tests/e2e/keyboard.spec.ts` | ожидает прогона |
| 4 | Сканирование и отмена | `Tab` до кнопок, `Enter`; прогресс в `aria-live` | `tests/unit/screens/core/scan.test.tsx`, `tests/unit/live.test.ts` | ожидает прогона |
| 5 | Дерево: навигация и детали | `↑/↓`, `Home/End`, `Space`, `Enter`, `→/←` | `tests/unit/keyboard.test.tsx`, `tests/e2e/keyboard.spec.ts`, `tests/unit/screens/core/tree.test.tsx` | ожидает прогона |
| 6 | Карта: плитки и погружение | стрелки, `Enter`, `Esc`, `Home/End` | `tests/unit/keyboard.test.tsx`, `tests/e2e/keyboard.spec.ts`, `tests/unit/screens/core/treemap.test.tsx` | ожидает прогона |
| 7 | Предпросмотр разрушающей операции | фокус в диалоге, `Tab`, `Esc`, явное подтверждение | `tests/unit/focus.test.tsx`, `tests/e2e/keyboard.spec.ts`, `tests/unit/operations/operations.test.tsx` | ожидает прогона |
| 8 | Сортировка и выбор в таблице анализа | `Tab` по заголовкам, `Space`, `Enter` | `tests/unit/data-table.test.tsx`, `tests/unit/keyboard.test.tsx` | ожидает прогона |
| 9 | Ошибки и уведомления | `role="alert"`, `aria-live` | `tests/unit/kit-part2.test.tsx`, `tests/unit/live.test.ts` | ожидает прогона |
| 10 | Журнал и смена темы/языка | `Tab`, `Enter`, переключатели | `tests/unit/shell.test.tsx`, `tests/unit/kit-part1.test.tsx` | ожидает прогона |

## 4. Автоматические проверки

- **axe (WCAG 2.1 A/AA)** — `ui/tests/a11y/screens.spec.ts` проверяет все 24 экрана в тёмной и
  светлой темах (48 прогонов) плюс базовые проверки ядра: 36 тестов зелёные. На каждом экране
  дополнительно проверяются landmark-и (локализованное имя навигации), skip link и заголовок `h1`.
  Набор тегов совпадает с общим помощником `tests/a11y/axe.ts`.
- **Юнит-тесты** — `tests/unit/focus.test.tsx` (ловушка, начальный и возвращаемый фокус,
  `rovingIndex`, видимое кольцо), `tests/unit/live.test.ts` (троттлинг, дедупликация, лимит,
  assertive), `tests/unit/keyboard.test.tsx` (грид, дерево, палитра, пространственная навигация
  карты), `tests/unit/i18n.test.ts` (кириллица в EN, плюрализация).
- **E2E клавиатуры** — `ui/tests/e2e/keyboard.spec.ts`: skip link, навигация, ловушка фокуса
  палитры и предпросмотра операции, roving tabindex дерева, стрелки/`Enter`/`Esc` карты,
  `Ctrl+,`, `F1`, `F6`, `Ctrl+F`, `Alt+←/→` и `Ctrl+1…9`.
- **E2E тем/DPI** — `ui/tests/e2e/theme.spec.ts`: отсутствие вспышки (маркер `data-theme-boot`),
  сохранение темы на всех 24 экранах, следование системной теме, раскладка при 100/125/150/200 %.
- **Визуальная регрессия** — полная матрица 384 эталона (24 × 2 темы × 4 DPI × 2 языка) плюс 66
  курируемых состояний в `tests/visual/baselines/` (всего 450 файлов).

## 5. Что осталось за пределами этого прохода

| Пункт | Причина | Куда отнести |
|-------|---------|--------------|
| Ручной Narrator-прогон | Нет доступа к Windows Narrator в среде агента | Приёмка US8 на Windows 11 (§6) |

Локализация служебных подписей оболочки (`skip link`, `Screens`/«Экраны», «Навигационная цепочка»,
«Изменить ширину панелей») выполнена в рамках T153: строки вынесены в `[a11y]` каталога
`locales/*.toml`, тесты используют русские имена по умолчанию и английские при переключении языка.

## 6. Как воспроизвести ручной прогон

1. `cd apps/freespace-desktop/ui && npm run dev`, открыть `http://127.0.0.1:1420`.
2. Включить Narrator: `Ctrl+Win+Enter`; режим «Сканирование» выключен.
3. Пройти сценарии из §3 только клавиатурой; на каждом шаге сверять:
   - имя, роль и состояние элемента (`Narrator`: `Insert+F1` / `CapsLock+F1` для веб-режима);
   - что фокус не уходит за пределы диалога и возвращается на инициатор;
   - что прогресс не повторяется чаще одного раза в 2 секунды;
   - что ошибки объявляются assertive, а статусы — polite.
4. Отдельно проверить, что статус различим без цвета: отключить «Цветовые фильтры» и убедиться,
   что выбранная строка имеет `✓`/`aria-selected`, ошибки — иконку и текст, карта — легенду и
   строку выбранной плитки.

## 7. Критерии приёмки US8

- [x] Фокус виден, ловушка и возврат фокуса реализованы и покрыты тестами.
- [x] Roving tabindex, стрелки, `Home/End`, `Space`, `Enter` работают в гриде, дереве и карте.
- [x] `aria-sort`, `aria-selected`, `aria-expanded`, `aria-busy`, `aria-current` выставлены.
- [x] Статус не передаётся только цветом в таблицах, дереве и карте.
- [x] Skip link, landmark-и и логический порядок Tab в оболочке; подписи локализованы (RU/EN).
- [x] Троттлинг, дедупликация и лимит объявлений реализованы и покрыты тестами.
- [ ] Ручной Narrator-прогон десяти сценариев на Windows 11 — **ожидает**.
- [x] axe для всех 24 экранов в обеих темах — пройдено.

---

## English summary

The US8 pass adds a shared focus trap (`ui/src/app/focus.ts`) used by `Dialog`, a throttled live
announcer (`ui/src/app/live.ts`) used by the status bar and operation progress, roving-tabindex
keyboard navigation for `DataTable`/tree rows (`treegrid` for the hierarchy) and the Treemap canvas,
a shell skip link with the documented landmarks, and non-colour status cues (icons, text badges,
selected-row marker). axe covers all 24 screens in both themes (36 passing tests), theme/DPI E2E
covers the no-flash marker and 100/125/150/200 %, and `tests/e2e/keyboard.spec.ts` covers the
keyboard-only walkthrough including shell shortcuts (`Ctrl+,`, `F1`, `F6`, `Ctrl+F`, `Alt+←/→`,
`Ctrl+1…9`). Shell labels are localized in RU and EN. The full visual matrix (384 references) plus
66 curated states (450 files) guards layout and status cues. Manual Narrator verification on
Windows 11 remains open.
