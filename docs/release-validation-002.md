# Приёмка релиза 002 — FreeSpace 1.2.0 (единый нативный интерфейс)

**Дата**: 2026-11-16
**Фича**: `002-native-ui-redesign`
**Среда**: Windows 11 23H2 (22631), x64; Rust stable 1.93 (edition 2024); Node.js 24.13;
Chromium 159 (Playwright); AMD Ryzen 7 6800H.

Отчёт фиксирует, что проверено автоматически, что подтверждено эталонами, а что требует ручного
прогона на чистой машине. Все команды запускались в этом репозитории без сети.

## 1. Автоматические гейты

| Гейт | Команда | Результат |
|------|---------|-----------|
| Форматирование | `cargo fmt --all -- --check` | PASS |
| Линты | `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| Rust-тесты | `cargo test --workspace` | PASS — 709 тестов в 52 наборах (unit, contract, integration) |
| Типы | `npm run typecheck` | PASS |
| Линты UI | `npm run lint` | PASS |
| Генерация локализации | `npm run check:i18n` | PASS (diff отсутствует) |
| Жёсткие цвета | `npm run check:tokens` | PASS (0 нарушений) |
| Сеть и CSP | `npm run check:network` | PASS (0 удалённых URL, минимальные permissions) |
| Юнит/компонентные тесты | `npm run test:unit` | PASS — 171 тест в 30 файлах |
| E2E | `npm run test:e2e` | PASS — 46 сценариев |
| Доступность | `npm run test:a11y` | PASS — 36 тестов (24 экрана × 2 темы + ядро) |
| Визуальная регрессия | `npm run test:visual` | PASS — 90 тестов, 450 эталонов |
| PR-набор визуала | `npm run test:visual:pr` | PASS — 9 тестов, 88 сравнений |
| Стабильность эталонов | повторный `npm run test:visual` без обновления | PASS (эталоны не «плывут») |

## 2. Сценарии быстрого старта

| # | Сценарий | Проверка | Результат |
|---|----------|----------|-----------|
| 1 | Предварительные требования | rustc/cargo/node, `.specify/feature.json` → 002 | PASS |
| 2 | Установка зависимостей | `npm ci`, `npm run gen:i18n` | PASS |
| 3 | Среда разработки | `npm run dev`, `npm run tauri dev` (конфиг) | PASS |
| 4 | Сборка | `npm run build`, `cargo build --workspace` | PASS |
| 5 | Быстрая проверка | fmt, clippy, test, typecheck, lint | PASS |
| 6 | Ядро (US1) | сканирование, дерево, карта, отмена | PASS (E2E + эталоны) |
| 7 | Анализ и очистка (US2) | топ, типы, дубликаты, очистка, снимки | PASS |
| 8 | Безопасные операции (US3) | preview → confirm → journal → undo | PASS |
| 9 | ФС и операции (US4) | длинные пути, права, переименование | PASS |
| 10 | Инспекция (US5) | метаданные, владельцы, архивы, экспорт | PASS |
| 11 | Операции и настройки (US6) | шредер, мониторинг, elevation, настройки | PASS |
| 12 | Темы, DPI и motion (US7) | 100–200%, системная тема, no-flash, reduced motion | PASS |
| 13 | Доступность (US8) | клавиатура, фокус, axe, не-цветовые статусы | PASS (axe/E2E); Narrator — вручную, см. §6 |
| 14 | Локализация (US9) | RU/EN, нет missing-key и кириллицы в EN | PASS |
| 15 | Визуальная регрессия (US10) | полная матрица и PR-набор | PASS |
| 16 | 10 млн узлов | backend + Canvas по отчёту производительности | PASS |
| 17 | Поставка (US11) | NSIS, portable, офлайн WebView2, `data` | Конфигурация и скрипты готовы; чистая машина — вручную, §6 |

## 3. Производительность

- Backend на 10 039 063 узлах: сборка дерева 6,1 с, `tree_root` 1 мс, `treemap_tiles` 102 мс,
  оценка памяти дерева ≈ 2,0 ГБ при бюджете 4 ГБ — [performance-002.md](performance-002.md).
- Canvas: 2 000 плиток, медиана 0,9 мс, p95 9,0 мс, максимум 14,3 мс на кадр (бюджет 33 мс).
- Запуск production-сборки через `vite preview` (Chromium, 1440×900): first paint 48 мс, оболочка
  211 мс, данные дашборда 216 мс — бюджеты 3 с / 5 с соблюдены с запасом.
- Память интерфейса на дашборде: 205 DOM-узлов, JS heap ≈ 10 МБ; страницы ограничены 2 000 строк,
  тайлы — `max_tiles`, состояние экрана освобождается при размонтировании.

## 4. Визуальная регрессия

- Полная матрица: **384** эталона (24 экрана × 2 темы × 4 DPI × 2 языка) в
  `apps/freespace-desktop/ui/tests/visual/baselines/`.
- Курируемые состояния доставки: **66** дополнительных эталонов (диалоги предпросмотра и прогресса,
  журнал, шредер, темы, узкий viewport, DPI 200%).
- PR-набор: 8 критических экранов × 2 темы × 100/150% × 2 языка + все 24 экрана в тёмной теме,
  100%, RU; сравнивается с теми же эталонами.
- Галерея для ревью: `apps/freespace-desktop/ui/tests/visual/gallery/index.html` (`npm run gallery`).
- Тест `tests/visual/coverage.test.ts` падает, если у нового экрана нет всех 16 ячеек матрицы.

## 5. Офлайн, безопасность и хранение

- CSP: `default-src 'self'`, `connect-src ipc: http://ipc.localhost`; `withGlobalTauri: false`;
  capabilities без `fs`/`shell`/`http`.
- `WEBVIEW2_BROWSER_ARGS` отключает фоновое сетевое взаимодействие; профиль WebView2 — в
  `data/webview2`; `check:network` проверяет исходники и конфиг.
- Установщик Tauri NSIS использует `offlineInstaller` WebView2 и минимальную версию
  `109.0.1518.78`; при старте версия проверяется и пишется предупреждение.
- Portable-сборка принимает `-WebView2Runtime` и кладёт fixed runtime в `webview2/`; без него
  используется системный Evergreen-рантайм.
- `tests/tauri_e2e/specs/storage.spec.ts` проверяет, что настройки, БД, профиль WebView2 лежат в
  `data` рядом с exe, а `%APPDATA%`/`%LOCALAPPDATA%` не содержат каталогов приложения.
- `tests/tauri_e2e/specs/offline.spec.ts` проверяет отсутствие удалённых ресурсов и блокировку
  `fetch` к внешнему origin в реальном окне.

## 6. Открытые ручные проверки

| Пункт | Причина | Как закрыть |
|-------|---------|-------------|
| Narrator-прогон десяти сценариев (T150) | В среде агента Narrator недоступен | Windows 11, `ui/tests/a11y/README`-сценарии из [accessibility-002.md](accessibility-002.md) §6 |
| Полный прогон GitHub Actions на Windows (T168) | Нет доступа к CI из агентской среды; локальный полный прогон зелёный | Push/PR в GitHub; конфигурация CI готова, Tauri E2E сделан блокирующим |
| Установка/удаление на чистой Windows без сети (T176) | Нужна отдельная машина/VM и подписанный установщик | Собрать `npm run tauri build`, прогнать §17 quickstart, записать результат сюда |
| Подпись установщика | Требует сертификата | Release-машина |

Эти пункты не влияют на автоматические гейты и не являются регрессией: они требуют оборудования и
учётных данных, недоступных в агентской среде.

## 7. Итог

- 24/24 экрана реализованы; второй UI и beta-флаг отсутствуют; egui удалён из workspace.
- 0 сетевых вызовов; CSP и capabilities минимальны; всё состояние — в `data`.
- Бюджеты 200 мс / 30 fps / 4 ГБ подтверждены отчётом и замерами запуска.
- AA в обеих темах подтверждён axe; DPI 100–200% и reduced motion покрыты E2E и эталонами.
- RU/EN без missing-key и без кириллицы в EN; плюрализация и `Intl`-форматирование покрыты тестами.
- 100% разрушающих действий проходят preview/confirm/journal; аудит — [operations-audit-002.md](operations-audit-002.md).
- Матрица паритета GUI/CLI заполнена: [parity.md](../specs/002-native-ui-redesign/contracts/parity.md).

---

## English summary

FreeSpace 1.2.0 ships the single Tauri 2 + React interface with 24 screens, RU/EN, dark/light/system
themes, DPI 100–200%, reduced motion, keyboard and Narrator semantics, and the shared safe-operation
flow. Automated gates are green: Rust fmt/clippy/tests, TypeScript typecheck and lint, 171 unit
tests, 46 E2E scenarios, 36 axe tests, a 384-cell visual matrix plus 66 curated references, and a
9-test PR subset. Startup measures 48 ms first paint, 211 ms to the shell and 216 ms to dashboard
data on the production build; the 10-million-node backend stays within 2 GB and 102 ms per treemap
request. Manual Narrator verification, a GitHub Actions run and a clean-machine installer run remain
open and are listed in §6.
