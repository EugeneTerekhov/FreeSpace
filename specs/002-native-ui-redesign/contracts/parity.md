# Матрица паритета GUI и CLI (002)

**Дата**: 2026-11-18 | **Фича**: `002-native-ui-redesign` | **Задачи**: T100, T101

Каждый экран нового GUI имеет аналог в CLI с JSON-выводом. Экранов **11 в 6 группах**;
поглощённые экраны (Сканирование, Карта, Детали размера, Владельцы, Снимки, Экспорт, Elevation,
О программе) остаются отдельными строками возможностей. Treemap удалён из продукта и строки
паритета не имеет.

Статусы:

- **implemented** — экран/вкладка реализованы, покрыты Rust-контрактом и Vitest-тестами, наборы
  E2E/axe обновлены под новую компоновку.
- **visual pending** — эталон визуальной регрессии пересобирается отдельным прогоном (T100,
  полная матрица 11 × 2 темы × 4 DPI × 2 языка) и пока не зафиксирован.

| Экран | Раздел/вкладка | Возможность | CLI-команда | Rust/unit | E2E/axe | Скриншот | Статус |
|-------|----------------|-------------|-------------|-----------|---------|----------|--------|
| `overview` | — | Список томов, занятое и свободное место, метрика и снимки | `drives` | contract + Vitest | обновлён | T100 | implemented |
| `overview` | Сканирование | Запуск, встроенный прогресс, отмена, ошибки доступа | `scan` | contract + Vitest | обновлён | T100 | implemented |
| `tree` | — | Иерархия, доли узлов, поиск, фильтр, пагинация | `tree`, `filter` | contract + Vitest | обновлён | T100 | implemented |
| `tree` | Детали размера (карточка по наведению) | Логический, занятый и физический размер, доля родителя, ссылки, ADS, сжатие | `tree --json` | contract + Vitest | обновлён | T100 | implemented |
| `tree` | Контекстное меню строки | Открыть в Проводнике и скопировать путь (только UI), переименование с предпросмотром, удаление только в Корзину с подтверждением | `tree`, `rename`, `delete --mode recycle` | contract + Vitest | обновлён | T035 | implemented |
| `analysis` | Крупнейшие файлы | Крупнейшие файлы с фильтрами и N | `top` | contract + Vitest | обновлён | T100 | implemented |
| `analysis` | Типы | Распределение по расширениям и категориям | `types` | contract + Vitest | обновлён | T100 | implemented |
| `analysis` | Владельцы | Место по владельцам и их файлы, экспорт | `owners` | contract + Vitest | обновлён | T100 | implemented |
| `duplicates` | — | Группы дубликатов, правило копии, предпросмотр удаления | `duplicates` | contract + Vitest | обновлён | T100 | implemented |
| `cleanup` | Очистка | Рекомендации, уровень риска, пустые каталоги | `cleanup` | contract + Vitest | обновлён | T100 | implemented |
| `cleanup` | Снимки | Создание снимков и сравнение | `snapshot` | contract + Vitest | обновлён | T100 | implemented |
| `journal` | — | Журнал операций, undo и restore через подтверждение | `journal` | contract + Vitest | обновлён | T100 | implemented |
| `shred` | — | Необратимое затирание с предпросмотром и подтверждением | `shred` | contract + Vitest | обновлён | T100 | implemented |
| `monitor` | — | Мониторинг записи, пороги и фильтры | `monitor` | contract + Vitest | обновлён | T100 | implemented |
| `inspect` | Метаданные | EXIF, медиа и PE-метаданные (только чтение) | `filter --metadata` | contract + Vitest | обновлён | T100 | implemented |
| `inspect` | Архивы | Просмотр архивов без распаковки | `archives` | contract + Vitest | обновлён | T100 | implemented |
| `inspect` | Разблокировка | Блокировки, мягкое и жёсткое снятие | `unlock` | contract + Vitest | обновлён | T100 | implemented |
| `inspect` | Длинные пути | Операции над verbatim-путями | `tree`, `rename` | contract + Vitest | обновлён | T100 | implemented |
| `fileops` | Переименование | Предпросмотр и применение переименования | `rename` | contract + Vitest | обновлён | T100 | implemented |
| `fileops` | Права | Атрибуты, владелец и ACL | `attrs`, `acl` | contract + Vitest | обновлён | T100 | implemented |
| `settings` | Настройки | Настройки, тема, язык, пути, плотность | `config` | contract + Vitest | обновлён | T100 | implemented |
| `settings` | Права доступа | Статус прав и запрос повышения (через подтверждение) | — (`app_elevation_status`) | Vitest | обновлён | T100 | implemented |
| `settings` | О программе | Версия, WebView2, лицензии, `data` | — (`app_about_info`) | contract + Vitest | обновлён | T100 | implemented |
| Экспорт (глобально) | — | CSV, XML, HTML и XLSX; действие на экранах с данными | `export` | contract + Vitest | обновлён | T100 | implemented |

## Как проверялось

- Rust: `tests/contract/{core,analysis,operations,filesystem,inspection,settings}_commands.rs`;
  команды `treemap_*` и соответствующие DTO/фикстуры удалены (T091), контракт CLI `tree` не
  изменялся.
- CLI: `tests/cli_contract.rs` и golden-фикстуры `tests/contract/fixtures/`; команды возвращают те
  же JSON-конверты, что и Tauri-команды.
- Frontend: `npm run test:unit` — Vitest/RTL для 11 экранов и вкладок; `npx tsc --noEmit`,
  `npm run lint`, `check:tokens`, `check:i18n`, `check:network` — зелёные.
- E2E/axe: `tests/e2e/*.spec.ts` и `tests/a11y/*.spec.ts` переписаны под 11 экранов, 6 групп,
  вкладки и глобальный экспорт. Прогон 2026-11-18: 55 E2E-сценариев и 24 axe-теста зелёные
  (найдены и исправлены пять контрастных дефектов тёмной темы).
- Визуальные эталоны: набор пересобирается отдельным прогоном T100; скриншоты в матрице не
  зафиксированы, поэтому статус `visual pending`, а не `verified`.

## Что остаётся

1. Пересборка визуальных эталонов 11 × 2 × 4 × 2 и PR-набора.
2. Ручной Narrator-прогон десяти сценариев (`quickstart.md`).

Разделы `settings`/«Права доступа» и «О программе» не имеют отдельной CLI-команды по
спецификации: повышение прав запускается только из GUI, а сведения о версии CLI печатает в
`--version`. Это зафиксировано в `specs/002-native-ui-redesign/spec.md` (FR-104, FR-107) и не
является пробелом паритета.
