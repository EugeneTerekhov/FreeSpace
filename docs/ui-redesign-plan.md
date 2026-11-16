# План: интерфейс FreeSpace на Tauri + React

**Статус:** **реализовано** в фиче 002 (`specs/002-native-ui-redesign/`). Итоговая приёмка —
[docs/release-validation-002.md](release-validation-002.md); матрица экранов и CLI —
[contracts/parity.md](../specs/002-native-ui-redesign/contracts/parity.md). Стек и решения ниже
сохранены как история планирования; актуальные требования — в спецификации 002.

**Дата:** 2026-11-16

**Обновление 2026-11-16 (после clarify 002):** старый интерфейс нигде не выпущен.
Решено не поддерживать два UI параллельно: спецификация 002 полностью заменяет
egui-интерфейс новым Tauri-клиентом в том же релизе. Beta-флаг, side-by-side и
отдельная cutover-фаза исключены. Пункты 5, 8, 11 и оценки ниже читать с этой
поправкой.

## 0. Принятые решения

1. **GUI переводится на Tauri 2 + React 18 + TypeScript + Vite.** Rust
   остаётся владельцем всей логики и файловых операций; React — только
   представление и ввод.
2. **Обе темы:** тёмная (флагманская, из макета) и светлая. Плюс режим
   «системная». Оба набора — из одних токенов, не два разных дизайна.
3. **Весь GUI** — 24 экрана, оболочка, диалоги, таблицы, treemap,
   состояния, доступность, DPI, RU/EN.
4. **Пункт 1 (GitHub Pages) — отдельный проект.** В спецификацию 002 не
   входит. `web/` остаётся визуальным референсом; его можно вынести в
   отдельный репозиторий. Продуктовый frontend — не `web/` и не Stageshow-кит:
   это новый UI-слой приложения.
5. **Старый egui-интерфейс** не выпускался. 002 полностью заменяет его новым
   Tauri-клиентом: egui-код и зависимости удаляются, второй UI не собирается и
   не поставляется. Параллельная поддержка и beta-флаг не создаются.

### Почему Tauri допустим

Конституция: «UI — egui/eframe как основной вариант; Tauri допускается только
при обоснованной необходимости веб-технологий и не должен снижать нативную
функциональность». Обоснование для ADR:

- нужен точный перенос нового языка интерфейса и компонентов React;
- веб-слой даёт готовую доступность, клавиатуру, CSS-темы, Playwright-тесты;
- вся нативная функциональность остаётся в Rust: NTFS, MFT/USN, длинные
  пути, права, elevation, операции, журнал;
- производительность и офлайн-требования проверяются отдельными бюджетами и
  не ослабляются.

**Обязательный шаг 0:** ADR `docs/adr/0001-tauri-react-gui.md` +
Constitution Check в плане 002 + Complexity Tracking. Без этого код не
начинается.

## 1. Что уже есть

- `web/` — интерактивный макет нового языка: тёмная тема, treemap,
  bento-сетка, hero-сцена с курсором. Это референс, не приложение.
- `apps/freespace-gui` — текущий egui/eframe 0.32 GUI, 24 экрана, старый UI.
- Ядро и CLI (`freespace`, существующие `crates/*`) не меняются по смыслу;
  CLI-паритет сохраняется.
- Сборка, installer, иконка, splash, локализация RU/EN — существуют.
- Плана редизайна/спецификации 002 ещё нет.

## 2. Целевая архитектура

```
apps/freespace-desktop/
  ui/                 React 18 + TS + Vite
    src/
      app/            shell, routing state, providers
      screens/        24 экрана (порт текущих views)
      components/     продуктовый UI-кит
      styles/         токены, темы, глобальные стили
      api/            типизированный IPC-клиент Tauri
      stores/         состояние представления
    tests/            Vitest + RTL + Playwright
  src-tauri/          Rust-бинарь Tauri 2
    src/
      commands/       scan, tree, treemap, dupes, cleanup, snapshots, ops…
      state.rs        AppState, каналы, отмена, прогресс
      events.rs       scan-progress, operation-progress, monitor-tick
      security.rs     валидация входов, пути, capabilities
  tauri.conf.json
  package.json
```

- `freespace-core`, `freespace-platform`, `freespace-ops`,
  `freespace-report` и остальные крейты используются как есть. WinAPI —
  только в `freespace-platform`.
- Новый app-крейт (при необходимости) — `freespace-app`: прикладные
  сервисы между Tauri-командами и core, чтобы не дублировать логику из
  старых egui-views. Решение — в research 002.
- **React не читает файлы и не вызывает WinAPI.** Только команды/события
  Tauri. Опасные операции — через существующий `freespace-ops` с preview,
  подтверждением, журналом и undo (принцип I).
- **Данные не гоняются целиком.** Дерево/таблица — постранично и
  виртуализированно; treemap — агрегированные тайлы/бинарный payload;
  прогресс — throttle 10–20 Гц. Полный JSON на 10 млн узлов запрещён.
- **Безопасность:** CSP `default-src 'self'`, только локальные ассеты, без
  remote URL; минимальные capabilities; без широких fs/shell/http-плагинов;
  типизированные команды; канонизация путей; devtools выключены в release.
- **Окно:** нативное окно Windows; WebView2; splash до готовности UI;
  `data` рядом с exe сохраняется (не `%APPDATA%`).
- **Темы:** CSS-переменные `:root[data-theme="dark"|"light"]`; системная —
  через `prefers-color-scheme` + сохранённую настройку; без вспышки темы при
  старте. Treemap/графики имеют палитры для обеих тем.
- **Шрифты/иконки:** системный Segoe UI Variable/Text + Cascadia Mono;
  lucide-react (уже в макете) или локальный набор; без CDN и сети.
- **Зависимости frontend** (черновик, требуют обоснования в research):
  React 18, TypeScript, Vite, `@tauri-apps/api`, `lucide-react`; для таблиц —
  TanStack Table/Virtual или собственный виртуализатор (spike); состояние —
  React context/useReducer или Zustand (spike); роутинг — состояние экрана,
  без URL-роутера (десктоп).
- **Пакетирование:** Tauri bundler (NSIS/MSI) + существующий Inno Installer
  как обёртка (решение spike). WebView2: системный или offline fixed runtime
  в составе установщика; сеть при установке запрещена. Портативная сборка и
  exe-имя `freespace-gui.exe` сохраняются при замене интерфейса.

## 3. Как язык макета переносится в утилиту

- Это не лендинг: в приложении нет hero-сцены, маркетинговой bento-сетки и
  курсора-истории.
- Переносятся: палитра, типографика, плотность, поверхности, радиусы,
  статус-цвета, treemap, summary-карточки, графики, таблицы, состояния,
  функциональный motion.
- Главный экран — «Карта» (treemap) с боковой панелью деталей; «Диски» —
  обзорный дашборд. Дальше — рабочие экраны по партиям.
- Bento-язык используется только как сетка summary-карточек на дашборде, не
  как декор на каждом экране.

## 4. Дизайн-система

Токены берутся из `web/src/site.css`/`hero.css` и переносятся в
`ui/src/styles/tokens.css`:

- фон `#07060C`, поверхность `#110E1B`, приподнятая `#171327`/`#1F1930`,
  линии `rgba(255,255,255,.075)`, текст `#F4EFE9`/`#A9A1B6`/`#736C82`,
  акцент `#FF8A3D`/`#FFB56B`, второй `#A06BFF`, успех `#7EE8A9`, ошибка
  `#F87171`, предупреждение `#FBBF24`;
- светлая тема — производные токены с контрастом AA, без второго дизайна;
- отступы 4/8/12/16/20/24/32/40, радиусы 6/8/10/14/20, фокус 2 px;
- motion: hover 120 мс, панели 180 мс, диалоги 240 мс; reduced motion
  отключает необязательные переходы;
- компоненты: shell/sidebar/header/status, buttons, fields, select,
  segmented/tabs, toggle/checkbox/radio, chips/badges, tooltip, progress,
  table, tree, list, dialogs, toasts, treemap, charts, empty/loading/error.
- **Stageshow-кит из `web/` в продукт не переносится.** Он нужен только для
  иллюстраций; продуктовые компоненты — свои, с теми же токенами.

## 5. Экраны и партии

| Партия | Экраны |
|---|---|
| Ядро | Диски, Сканирование, Дерево, Карта |
| Анализ/очистка | Крупнейшие файлы, Типы, Дубликаты, Очистка, Снимки |
| ФС и операции | Журнал, Детали размера, Длинные пути, Переименование, Права |
| Инспекция/отчёты | Метаданные, Владельцы, Архивы, Экспорт, Разблокировка |
| Операции/настройки | Шредер, Мониторинг записи, Elevation, Настройки, О программе |

Для каждого: загрузка/пусто/ошибка/долгая операция/отмена, клавиатура,
обе темы, RU/EN, 100/125/150/200% DPI, скриншот-тест.

## 6. Этапы и оценка

**Вариант с двумя инженерами** (1 Rust/backend + 1 frontend, дизайнер 30%):
**12–16 недель**. Один full-stack инженер: **22–28 недель**. Буфер 20%.

| Этап | Недели | Содержание | Параллельно |
|---|---|---|---|
| 0. ADR, spec 002, spike | 2 | Constitution Check, ADR, Tauri-скелет, 2 экрана, IPC, 10 млн узлов, темы, WebView2/installer | дизайн-токены |
| 1. Foundation | 3 | React/Vite/TS, токены, обе темы, shell, UI-кит, ошибки, IPC-клиент, галерея, тесты | Rust commands/state/events |
| 2. Мост данных | 2.5 | volumes/scan/tree/treemap/operations, paging, отмена, прогресс, security caps | продолжение shell/кита |
| 3. Ядро | 2.5 | Диски, Сканирование, Дерево, Карта | polish кита |
| 4. Анализ/очистка | 2.5 | Топ, Типы, Дубликаты, Очистка, Снимки | тесты/скриншоты |
| 5. ФС/операции | 2.5 | Журнал, Детали, Длинные пути, Переименование, Права | доступность |
| 6. Инспекция/отчёты | 2 | Метаданные, Владельцы, Архивы, Экспорт, Разблокировка | производительность |
| 7. Операции/настройки | 2 | Шредер, Мониторинг, Elevation, Настройки, О программе | installer/WebView2 |
| 8. Полировка/замена | 2.5 | доступность, DPI, производительность, скриншоты, документация, удаление egui-UI | релиз |

Критический путь: IPC/treemap производительность → партия 3 → партии 4–7 →
упаковка/замена. Если spike 10 млн узлов не проходит бюджет, сначала
оптимизировать пейджинг/Canvas, не начинать партии 3+.

## 7. Тестирование и критерии

- **Frontend:** Vitest + React Testing Library; Playwright на Vite-сборке;
  скриншоты ключевых экранов в обеих темах и на 100/150/200% DPI.
- **Tauri E2E:** `tauri-driver`/WebDriver на Windows: запуск, навигация,
  диалоги, отмена, опасные операции (с тестовыми VHD/временными каталогами).
- **Rust:** существующие тесты ядра/CLI остаются зелёными; новые тесты
  команд/валидации/сериализации.
- **Доступность:** axe/Playwright, клавиатура, фокус, ARIA, Narrator.
- **Производительность:** 10 млн узлов, виртуализация, ≤200 мс отклик,
  30 fps treemap, память ≤4 ГБ суммарно; отдельный бюджет WebView.
- **Офлайн:** network guard в тестах; CSP; ни одного remote-запроса.
- **Установщик:** установка без сети, WebView2, portable `data`, запуск,
  удаление, контекстное меню off by default, подпись.
- **Приёмка:** 24/24 экрана, 0 хардкод-цветов, 0 хардкод-строк, AA-контраст,
  RU/EN, обе темы, старый UI удалён, CLI-паритет не нарушен.

## 8. Constitution Check (черновик)

| Принцип | Как выполняется |
|---|---|
| I. Безопасные разрушающие операции | React только вызывает команды; preview/confirm/undo/journal остаются в `freespace-ops` |
| II. Точный учёт места | Все размеры считает Rust; UI показывает результат и тип размера |
| III. Производительность | Rust-ядро, пейджинг, виртуализация, Canvas-treemap, бюджеты и бенчмарки |
| IV. Нативная точность Windows | WebView2 — компонент Windows; WinAPI только в `freespace-platform`; long paths, NTFS, elevation |
| V. Тест-первый подход | Rust-тесты + frontend/E2E/скриншоты; фикстуры только временные/VHD |
| VI. Паритет CLI/GUI | Одно ядро; CLI-контракт не меняется |
| VII. Офлайн/приватность | Нет remote-ассетов/телеметрии; CSP; fixed WebView2 offline |
| Tech constraints | Tauri разрешён с обоснованием; ADR + Complexity Tracking |

**Complexity Tracking:** единовременная замена 24 экранов и удаление
egui-зависимостей; WebView2 runtime; IPC-контракт; Node-сборка в CI; миграция
состояния 24 экранов; двойные темы; размер установщика.

## 9. Черновик спецификации 002

**Истории:** единый язык на 24 экранах; treemap как главный рабочий экран;
плотные таблицы/фильтры/массовые действия; безопасные операции с preview;
обе темы/системная/reduced motion; клавиатура и доступность; RU/EN;
скриншот-галерея и тесты как Definition of Done.

**FR-UI (черновик):**
- Tauri 2 + React/TS frontend; Rust — единственный владелец логики/ФС.
- Все 24 экрана на продуктовом UI-ките; токены вместо хардкода.
- Оболочка: sidebar/header/status/toasts/dialogs.
- Обе темы + системная; AA-контраст.
- Treemap/таблицы: пейджинг, виртуализация, бюджеты 200 мс/30 fps.
- IPC: типизированные команды, CSP, минимальные capabilities, без remote.
- Доступность: клавиатура, фокус, ARIA/accesskit-эквивалент, не только цвет.
- DPI 100–200%, reduced motion.
- Локализация RU/EN.
- Установщик: offline WebView2, portable `data`, exe-имя `freespace-gui.exe`.
- Тесты: Rust, Vitest/RTL, Playwright, Tauri E2E, скриншоты.
- Замена: паритет, удаление egui-UI, обновление installer/CI/docs.

**SC-UI (черновик):** 24/24 экрана; 0 сетевых вызовов; ≤200 мс; 30 fps;
AA; 100–200% DPI; обе темы; зелёные тесты; CLI-контракт без изменений;
установка без сети.

**Артефакты:** `specs/002-native-ui-redesign/{spec,plan,research,quickstart,tasks}.md`,
`contracts/ui.md` (токены/IPC/компоненты), чек-листы, зеркала `ru/`/`en/`;
ADR `docs/adr/0001-tauri-react-gui.md`.

## 10. Риски

| Риск | Мера |
|---|---|
| WebView2 отсутствует/старый на Windows 10 | offline fixed runtime в установщике; проверка версии; portable-вариант |
| 10 млн узлов и IPC | пейджинг, бинарные payload, Canvas-treemap, виртуализация; spike до партий |
| Память > 4 ГБ (Rust + WebView) | бюджет WebView, ленивые экраны, освобождение данных, замеры |
| Дублирование логики из egui-views | `freespace-app`/команды, перенос логики в Rust, тесты |
| Безопасность Tauri | CSP, capabilities, валидация, без remote, code review |
| Миграция 24 экранов | партии, полная замена, без beta и второго UI |
| Скриншот-тесты нестабильны | фиксированные шрифты/DPI, допуск, CI Windows |
| Размер установщика из-за WebView2 | решение: system vs fixed; измерить; документация |
| Доступность | axe/Playwright, Narrator, клавиатура; ранний аудит |
| Двойная тема | один набор токенов, один UI-кит |
| Сроки | 2 инженера/буфер; сокращать партии нельзя без спеки |

## 11. Передача в следующую сессию

**Сначала прочитать:** `AGENTS.md`, конституцию, `specs/001.../spec.md`,
`specs/001.../plan.md`, этот план, `web/` (референс), `app.rs` и `views/*`
текущего GUI, `Cargo.toml`/`installer/`.

**Первые задачи следующей сессии:**
1. `/speckit.specify` для `specs/002-native-ui-redesign/` по черновику выше;
   clarify; plan; tasks; чек-листы; зеркала ru/en.
2. `docs/adr/0001-tauri-react-gui.md`: решение, альтернативы, последствия,
   Constitution Check, WebView2/offline, безопасность, полная замена egui.
3. Скелет `apps/freespace-desktop` (Tauri 2 + Vite/React/TS) с hello-окном,
   темами и одним экраном «Диски» на моках.
4. Spike IPC: `volumes` + `scan` + `tree` page + `treemap` tiles; Canvas
   treemap; замеры на 10 млн узлов; отчёт в `research.md`.
5. Spike упаковки: Tauri bundler, WebView2 fixed/system, portable `data`,
   exe-имя, Inno, размер, подпись.
6. Выбрать table/virtualization/state-зависимости; зафиксировать в research.
7. Заморозить токены/скриншоты `web/`; вынести Pages-проект отдельно
   (вне 002).

**Команды-ориентиры:**
```powershell
# текущий продукт
cargo test --workspace
# макет (референс)
cd web; npm install; npm run dev; npm run build
# будущий Tauri-клиент (после скелета)
cd apps/freespace-desktop/ui; npm install; npm run build
cd ../src-tauri; cargo check
```

**Открытые решения для 002 (закрываются в research):** WebView2 fixed vs
system; Tauri bundler vs Inno; TanStack vs свой виртуализатор; Zustand vs
context. Уже решено: exe-имя `freespace-gui.exe` сохраняется; egui-приложение
удаляется из workspace; beta-флага и параллельной поддержки нет.

---

## English summary

The UI stack decision is now locked: Tauri 2 + React 18 + TypeScript + Vite,
with both dark and light themes (dark flagship). Rust remains the only owner of
NTFS, operations and native Windows code; React owns presentation and input.
The separate GitHub Pages illustration is out of scope for this project's
specification. The existing egui GUI was never released. Feature 002 replaces
it outright with the Tauri client; no parallel support, beta flag, or separate
cutover phase is created. The old UI and egui dependencies are removed.

The plan covers the design tokens carried from the `web/` mock, a product UI
kit (the Stageshow illustration kit is not shipped), the Tauri/Rust command and
event architecture, secure IPC and CSP, paged/virtualized data and a Canvas
treemap for 10M nodes, offline WebView2 packaging, accessibility, DPI, both
themes and RU/EN. Estimate: 12–16 weeks with two engineers (one Rust, one
frontend) or 22–28 weeks for one full-stack engineer. The next session starts
with an ADR/constitution check, the Spec Kit feature 002, a Tauri shell and a
10M-node IPC/treemap packaging spike. The plan is the handoff document
(`docs/ui-redesign-plan.md`).
