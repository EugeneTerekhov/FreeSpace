# План реализации: FreeSpace — единый нативный интерфейс нового поколения

**Branch**: `002-native-ui-redesign` | **Date**: 2026-11-16 | **Spec**: [spec.md](./spec.md)

**Input**: Спецификация функции из `/specs/002-native-ui-redesign/spec.md`

**Замена, а не параллельная поддержка**: старый egui-интерфейс нигде не выпущен. В рамках 002
он полностью заменяется новым клиентом: `apps/freespace-gui` и egui-зависимости удаляются,
второй UI не собирается и не поставляется. Beta-флаг, side-by-side и отдельная cutover-фаза
исключены. Это решение зафиксировано в [spec.md](./spec.md#clarifications) и в ADR
[0001-tauri-react-gui.md](../../docs/adr/0001-tauri-react-gui.md).

## Summary

FreeSpace переводит графический интерфейс на Tauri 2 + React 18 + TypeScript + Vite. Rust
остаётся единственным владельцем доменной логики, WinAPI, сканирования, операций, журнала и
карантина; React отвечает только за представление и ввод. Новый клиент содержит 24 экрана,
единый продуктовый UI-кит, тёмную/светлую/системную тему, виртуализированные таблицы и Canvas
Treemap с порционной передачей данных, безопасные операции через существующий `freespace-ops`,
доступность с клавиатуры и Narrator, DPI 100–200%, локализацию RU/EN и офлайн-поставку.
Старый egui-клиент удаляется в этой же функции; CLI-контракт и ядро не меняются.

## Technical Context

**Language/Version**: Rust 1.85+ (edition 2024), TypeScript 5.6+, Node.js 20.19+, Windows x64

**Primary Dependencies**:

- UI: Tauri 2.x (`tauri`, `tauri-build`), React 18.3, React DOM 18.3, TypeScript 5.6,
  Vite 6.x, `@vitejs/plugin-react` 4.x, `@tauri-apps/api` 2.x, типы `@types/react` и
  `@types/react-dom`.
- Компоненты и иконки: `lucide-react` 0.468 (локальный пакет, без CDN).
- Линт: `eslint` 9.x, `typescript-eslint` 8.x, `eslint-plugin-react-hooks`,
  `eslint-plugin-react-refresh`.
- Таблицы и виртуализация: `@tanstack/react-table` 8.x и `@tanstack/react-virtual` 3.x —
  модель столбцов, видимость/порядок и виртуализация строк; сортировка и фильтрация остаются
  серверными.
- Состояние представления: `zustand` 5.x — небольшие типизированные stores на экран и общий
  shell-store; без Redux и без глобального кеша данных.
- Локализация: существующие `locales/ru.toml` и `locales/en.toml` как единственный источник;
  генератор `ui/scripts/gen-i18n.mjs` с `smol-toml` создаёт типизированные TS-каталоги.
- Тесты: `vitest`, `@testing-library/react`, `@testing-library/user-event`, `jsdom`,
  `@playwright/test`, `@axe-core/playwright`, `tauri-driver` и `webdriverio` на Windows.
- Rust: `tauri`, `tauri-build`, `serde`, `serde_json`, `thiserror`, `tracing`,
  `tracing-subscriber`, существующие `freespace-core`, `freespace-platform`,
  `freespace-scan`, `freespace-ops`, `freespace-analysis`, `freespace-report`,
  `freespace-cli`; асинхронные задачи — через Tauri async runtime, без прямого `tokio`.
- Новый прикладной слой: `crates/freespace-app` — команды, сервисы, DTO, пагинация,
  агрегация Treemap и миграция настроек; не содержит WinAPI и не заменяет ядро.

Обоснование каждой новой зависимости и альтернативы — в [research.md](./research.md).

**Storage**: единственный каталог `data` рядом с исполняемым файлом. Существующие SQLite
(журнал, карантин, кеш хешей), TOML-настройки и снимки `.fssnap` сохраняются. Новые
UI-настройки хранятся в `data/settings.toml` (секция `[ui]`), состояние окна — в
`data/ui-state.toml`; `localStorage`, `IndexedDB` и профиль WebView2 вне `data` запрещены.
Профиль WebView2 принудительно размещается в `data/webview2`. `%APPDATA%`,
`%LOCALAPPDATA%`, `%ProgramData%` и временные каталоги не используются.

**Testing**: `cargo test --workspace` (ядро, CLI, операции, новые команды и валидация),
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`vitest` + React Testing Library для компонентов и экранов, Playwright для Vite-сборки,
`tauri-driver`/WebDriver для Tauri E2E, `@axe-core/playwright` для доступности, визуальная
регрессия Playwright с фиксированными шрифтами/DPI, network guard, административные тесты
только при `FREESPACE_ADMIN_TESTS=1`.

**Target Platform**: Windows 10 21H2+ и Windows 11, x64; WebView2 (системный или поставленный
офлайн); NTFS как основная ФС, non-NTFS fallback сохраняется.

**Project Type**: desktop-app + CLI (Cargo workspace + Tauri frontend)

**Performance Goals**:

- Отклик интерфейса на действия — не более 200 мс при сканировании и навигации.
- Treemap — не ниже 30 кадров/с на контрольном наборе 10 млн узлов.
- Память всего приложения, включая нативный слой и WebView2, — не более 4 ГБ на 10 млн узлов.
- Первый интерактивный экран — не более 3 с; первый экран с данными — не более 5 с после
  готовности данных.
- Полный анализ NTFS-тома 8 ТБ — не более 60 с (существующий бюджет ядра не меняется).
- Прогресс и события — throttle 10–20 Гц; полный JSON дерева на 10 млн узлов запрещён.

**Constraints**:

- Полная офлайн-работа, отсутствие телеметрии, сетевых запросов и remote-ассетов.
- CSP `default-src 'self'`; минимальные capabilities Tauri; без плагинов fs/shell/http;
  devtools выключены в release.
- Все WinAPI-вызовы — только в `crates/freespace-platform`; Tauri-команды и React не вызывают
  WinAPI напрямую.
- Разрушающие операции — только через `freespace-ops`: предпросмотр, подтверждение, выбор
  способа удаления, журнал, карантин/Корзина, undo и защита системных путей.
- Настройки, журнал, карантин, кеш и данные WebView2 — только в `data` рядом с exe.
- Один графический интерфейс: egui удаляется, второй UI не поддерживается.
- CLI-контракт, коды возврата, форматы отчётов и снимков не меняются.
- 24 экрана, RU/EN, тёмная/светлая/системная тема, AA, DPI 100/125/150/200%, reduced motion.
- Имя исполняемого файла `freespace-gui.exe` сохраняется; exe-имя, ярлыки и контекстное меню
  не ломаются.

**Scale/Scope**: 24 экрана, 5 партий экранов, 11 пользовательских историй, 130 FR, 22 SC;
до 10 млн узлов на том, тома до 8 ТБ, 2 языка, 3 режима темы, 4 масштаба DPI; один GUI-клиент
и существующий CLI.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Принцип | Соответствие | Обоснование |
|---------|--------------|-------------|
| I. Безопасность разрушающих операций | PASS | Новый UI только вызывает команды; предпросмотр, подтверждение, выбор способа удаления, журнал, карантин, undo и защита системных путей остаются в `freespace-ops`. UI не получает прямого доступа к ФС. |
| II. Точность учёта места | PASS | Все размеры считает Rust; UI показывает логический/на диске/физический размер с явной подписью, не пересчитывает суммы и не округляет их для итогов. |
| III. Производительность на больших объёмах | PASS | MFT/USN и ядро не меняются; дерево и таблицы — постранично и виртуализированно, Treemap — агрегированные тайлы и Canvas LOD, прогресс throttle 10–20 Гц, бюджеты 200 мс/30 fps/4 ГБ проверяются отдельно. |
| IV. Нативная точность Windows | PASS | WebView2 — компонент Windows; WinAPI остаётся только в `freespace-platform`; Tauri-команды не вызывают WinAPI. Длинные пути, NTFS, права, elevation и контекстное меню сохраняются. |
| V. Тест-первый подход | PASS | Rust-тесты команд/валидации/сериализации, Vitest/RTL, Playwright, tauri-driver E2E, axe и визуальная регрессия; фикстуры — только временные каталоги и тестовые VHD; admin-тесты за `FREESPACE_ADMIN_TESTS=1`. |
| VI. Паритет CLI и GUI | PASS | Одно ядро; CLI-контракт защищён эталонными JSON-проверками; матрица паритета «экран — возможность — CLI-команда — тест — скриншот» заполняется до релиза. |
| VII. Приватность и офлайн | PASS | Нет remote-ассетов, телеметрии и сетевых плагинов; CSP, минимальные capabilities, offline WebView2, все данные в `data`. |
| Tech constraints: UI — egui/eframe, Tauri только с обоснованием | PASS (с ADR) | Tauri разрешён конституцией при обоснованной необходимости веб-технологий и сохранении нативной функциональности. Обоснование — ADR `docs/adr/0001-tauri-react-gui.md`; egui полностью заменяется, второй UI не поддерживается. |

Нарушений, требующих остановки, нет. Отклонение по UI-фреймворку обосновано ADR и не снижает
нативную функциональность. Сложность замены зафиксирована в Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/002-native-ui-redesign/
├── plan.md              # Этот файл
├── research.md          # Phase 0: решения, альтернативы и бюджеты
├── data-model.md        # Phase 1: сущности представления и состояния
├── quickstart.md        # Phase 1: сценарии проверки
├── contracts/
│   ├── ui.md            # Токены, компоненты, 24 экрана, доступность
│   └── ipc.md           # Команды, события, пагинация, Treemap, ошибки
├── checklists/
│   └── requirements.md  # Чек-лист качества требований
└── tasks.md             # Phase 2: задачи (/speckit.tasks)
```

### Source Code (repository root)

```text
Cargo.toml                        # workspace: убрать apps/freespace-gui, добавить новый GUI и freespace-app
rust-toolchain.toml
crates/
├── freespace-core/               # доменная модель, дерево, метрики, настройки, i18n (без изменений по смыслу)
├── freespace-platform/           # WinAPI: тома, MFT/USN, блокировки, ACL, TRIM, реестр, ETW
├── freespace-scan/               # MFT и fallback-обход, инкрементальные сессии
├── freespace-ops/                # операции, предпросмотр, журнал, карантин, шредер
├── freespace-analysis/           # дубликаты, очистка, метаданные, владельцы, архивы
├── freespace-report/             # отчёты, снимки и сравнение
├── freespace-app/                # НОВЫЙ: прикладные сервисы, DTO, пагинация, агрегация Treemap
└── freespace-cli/                # CLI без изменения контракта
apps/
└── freespace-desktop/            # НОВЫЙ единственный GUI; заменяет apps/freespace-gui
    ├── ui/                       # React 18 + TS + Vite
    │   ├── src/
    │   │   ├── app/              # shell, routing state, providers, boot
    │   │   ├── screens/          # 24 экрана
    │   │   ├── components/       # продуктовый UI-кит
    │   │   ├── styles/           # tokens.css, themes.css, global.css
    │   │   ├── api/              # типизированный IPC-клиент
    │   │   ├── stores/           # Zustand stores
    │   │   ├── i18n/             # сгенерированные каталоги и ключи
    │   │   └── treemap/          # Canvas-отрисовка, hit-testing, палитры
    │   ├── tests/                # Vitest/RTL, Playwright, screenshots
    │   ├── scripts/gen-i18n.mjs  # TOML → типизированные TS-каталоги
    │   ├── package.json
    │   └── vite.config.ts
    ├── src-tauri/                # Rust-бинарь Tauri 2
    │   ├── src/
    │   │   ├── commands/         # scan, tree, treemap, dupes, cleanup, snapshots, ops, settings…
    │   │   ├── state.rs          # AppState, задачи, каналы, отмена, прогресс
    │   │   ├── events.rs         # scan-progress, operation-progress, monitor-tick
    │   │   ├── security.rs       # валидация входов, путей, capabilities
    │   │   ├── boot.rs           # тема/язык/окно до первой отрисовки
    │   │   ├── lib.rs
    │   │   └── main.rs
    │   ├── capabilities/default.json
    │   ├── icons/
    │   ├── Cargo.toml
    │   └── tauri.conf.json
    └── build.rs
tests/                            # межкрейтовые тесты; tauri_e2e/ для WebDriver
benches/                          # бенчмарки сканирования и Treemap
docs/
└── adr/0001-tauri-react-gui.md   # обязательный ADR
locales/                          # ru.toml/en.toml — единственный источник строк
installer/                        # заменить/адаптировать под Tauri NSIS, сохранить удаление data и меню
ru/                               # зеркала документации (включая 002)
en/                               # переводы документации (включая 002)
```

**Structure Decision**: один GUI-крейт `apps/freespace-desktop` с бинарём
`freespace-gui.exe`. `crates/freespace-app` — тонкий прикладной слой между Tauri-командами и
существующим ядром, чтобы не дублировать логику 24 egui-views и не превращать Tauri-команды в
бизнес-логику. React не имеет доступа к ФС и WinAPI: только типизированные команды/события.
`apps/freespace-gui` удаляется; его исходники остаются в истории git как источник паритета.
Второй UI, beta-флаг и side-by-side не создаются.

## Delivery Phases

| Этап | Содержание | Результат |
|------|------------|-----------|
| 0. ADR и каркас | ADR, Constitution Check, Tauri-скелет, темы, IPC hello, WebView2/упаковка spike | Решение зафиксировано, риск снят |
| 1. Foundation | Токены, обе темы, shell, UI-кит, i18n, API-клиент, stores, error boundary, тестовая инфраструктура | Галерея компонентов и оболочка |
| 2. Данные | volumes/scan/tree/treemap/ops, пагинация, отмена, прогресс, security caps, миграция настроек | Рабочий мост данных |
| 3. Ядро | Диски, Сканирование, Дерево, Карта | MVP |
| 4. Безопасность операций | Предпросмотр, подтверждение, журнал, карантин, undo | Единый слой для всех очисток |
| 5. Анализ/очистка | Топ, Типы, Дубликаты, Очистка, Снимки | Партия 2 |
| 6. ФС/операции | Журнал, Детали, Длинные пути, Переименование, Права | Партия 3 |
| 7. Инспекция/отчёты | Метаданные, Владельцы, Архивы, Экспорт, Разблокировка | Партия 4 |
| 8. Операции/настройки | Шредер, Мониторинг, Elevation, Настройки, О программе | Партия 5 |
| 9. Полировка | Доступность, DPI, производительность, скриншоты, офлайн-упаковка, удаление egui, документация | Релиз |

## Complexity Tracking

| Violation / Complexity | Why Needed | Simpler Alternative Rejected Because |
|-----------------------|------------|--------------------------------------|
| Tauri 2 + WebView2 вместо egui | Нужен точный перенос нового языка интерфейса и компонентов React, CSS-темы, DOM-доступность и Playwright; конституция допускает Tauri при обоснованной необходимости. | egui не даёт нужного уровня визуального контроля, CSS-тем и DOM-доступности; рисовать 24 экрана заново в egui означало бы дублировать дизайн-систему и потерять заявленный язык макета. |
| Новый крейт `freespace-app` | Прикладные сервисы, DTO, пагинация, агрегация Treemap и миграция настроек общие для команд; иначе логика 24 экранов расползётся по Tauri-командам или будет скопирована из egui. | Команды напрямую над ядром быстрее на старте, но приводят к дублированию и труднее тестируются; перенос всей логики в `freespace-core` размывает домен. |
| Node.js/Vite/TypeScript в Rust-репозитории | Требуется React-сборка, типизация IPC и frontend-тесты. | Отказ от Node означает отказ от выбранного стека; альтернативный UI-фреймворк не даёт нужного дизайна и доступности. |
| IPC-контракт и порционная передача | 10 млн узлов нельзя передавать полным JSON; нужны страницы, агрегаты и события. | Полный JSON/дамп дерева не проходит бюджет памяти и времени; прямой доступ React к ФС запрещён конституцией. |
| Offline WebView2 и размер установщика | Установка без сети обязательна; системный WebView2 не гарантирован на Windows 10. | Требовать системный WebView2 — риск отказа установки; download bootstrapper нарушает офлайн-требование. |
| Полная замена 24 экранов и удаление egui | Продукт ещё не выпущен; поддерживать два UI невыгодно и опасно. | Параллельная поддержка удваивает тесты, доступность и упаковку; beta-флаг оставляет два кодовых пути. |
| Две темы, DPI и локализация на 24 экранах | Это требование продукта и приёмки. | Одна тема/один DPI/один язык сократили бы объём, но не соответствуют спецификации. |
| Настройки и данные WebView2 в `data` | Конституция и FR-093 запрещают `%APPDATA%`/`%LOCALAPPDATA%`; профиль WebView2 по умолчанию пишет в профиль пользователя. | Оставить профиль по умолчанию — нарушение хранения; localStorage/IndexedDB — скрытые данные вне `data`. |

## Post-Design Constitution Re-Check

После Phase 1 (data-model, contracts, quickstart) повторно проверяются:

- все команды проходят через `freespace-ops` и не дают UI прямого доступа к ФС;
- размеры и типы метрик не пересчитываются в TypeScript;
- пагинация, Treemap и события укладываются в бюджеты 200 мс / 30 fps / 4 ГБ;
- CSP и capabilities не содержат сетевых и широких файловых разрешений;
- webview-профиль и настройки размещаются в `data`;
- 24 экрана покрыты матрицей паритета и тестами;
- egui удалён из workspace, второй UI отсутствует.

## Risks and Mitigations

| Риск | Мера |
|------|------|
| WebView2 отсутствует/старый на Windows 10 | Offline fixed runtime в поставке; проверка версии; portable-вариант; spike упаковки. |
| 10 млн узлов и IPC | Пагинация, агрегированные тайлы, Canvas, виртуализация, throttle; spike до партий 3+. |
| Память > 4 ГБ (Rust + WebView) | Бюджет WebView, освобождение данных при уходе с экрана, замеры всего процесса. |
| Дублирование логики egui-views | `freespace-app`, DTO и тесты; egui-код удаляется, а не поддерживается. |
| Безопасность Tauri | CSP `default-src 'self'`, минимальные capabilities, валидация входов, без remote, devtools off, code review. |
| Потеря пользовательских данных при миграции | Резервная копия `settings.toml`, версия миграции, обратимость, тесты на копиях данных. |
| Скриншот-тесты нестабильны | Фиксированные шрифты/DPI, допуски, Windows CI, стабильные фикстуры. |
| Размер установщика из-за WebView2 | Измерить fixed vs offline installer; зафиксировать бюджет и решение в research. |
| Доступность | axe/Playwright, Narrator, клавиатурные сценарии, ранний аудит. |
| Сроки | Партии экранов, MVP (US1) первым, буфер; сокращать партии нельзя без обновления спецификации. |
