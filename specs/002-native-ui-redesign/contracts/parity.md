# Матрица паритета GUI и CLI (002)

**Дата**: 2026-11-16 | **Фича**: `002-native-ui-redesign` | **Задача**: T177, T181

Каждый экран нового GUI имеет аналог в CLI с JSON-выводом. Скриншот указывает ячейку полной
визуальной матрицы `apps/freespace-desktop/ui/tests/visual/baselines/` (`<экран>-dark-100-ru.png`).
Статус `verified` означает, что экран реализован, покрыт Rust- или Vitest-тестом, E2E-сценарием и
эталоном, а CLI-поведение сверено с тем же набором данных.

| Экран | Возможность | CLI-команда | Тесты | Скриншот | Статус |
|-------|-------------|-------------|-------|----------|--------|
| `dashboard` | Список томов, занятое и свободное место | `drives` | Rust + E2E + visual | `dashboard-dark-100-ru` | verified |
| `scan` | Запуск, прогресс и отмена сканирования | `scan` | Rust + E2E + visual | `scan-dark-100-ru` | verified |
| `tree` | Иерархия, поиск, фильтр и точные размеры | `tree`, `filter` | Rust + E2E + visual | `tree-dark-100-ru` | verified |
| `treemap` | Карта, доли и погружение по уровням | `tree` | Rust + E2E + visual | `treemap-dark-100-ru` | verified |
| `top_files` | Крупнейшие файлы с фильтрами | `top` | Rust + E2E + visual | `top-files-dark-100-ru` | verified |
| `types` | Распределение по расширениям и категориям | `types` | Rust + E2E + visual | `types-dark-100-ru` | verified |
| `duplicates` | Группы дубликатов и выбор копии | `duplicates` | Rust + E2E + visual | `duplicates-dark-100-ru` | verified |
| `cleanup` | Рекомендации по очистке и пустые каталоги | `cleanup` | Rust + E2E + visual | `cleanup-dark-100-ru` | verified |
| `snapshots` | Создание снимков и сравнение | `snapshot` | Rust + E2E + visual | `snapshots-dark-100-ru` | verified |
| `journal` | Журнал операций, undo и restore | `journal` | Rust + E2E + visual | `journal-dark-100-ru` | verified |
| `size_details` | Логический, занятый и физический размер | `tree --json` | Rust + E2E + visual | `size-details-dark-100-ru` | verified |
| `long_paths` | Операции над длинными путями | `tree`, `rename` | Rust + E2E + visual | `long-paths-dark-100-ru` | verified |
| `rename` | Предпросмотр и применение переименования | `rename` | Rust + E2E + visual | `rename-dark-100-ru` | verified |
| `permissions` | Атрибуты, владелец и ACL | `attrs`, `acl` | Rust + E2E + visual | `permissions-dark-100-ru` | verified |
| `metadata` | EXIF, медиа и PE-метаданные | `filter` | Rust + E2E + visual | `metadata-dark-100-ru` | verified |
| `owners` | Место по владельцам и их файлы | `owners` | Rust + E2E + visual | `owners-dark-100-ru` | verified |
| `archives` | Просмотр архивов без распаковки | `archives` | Rust + E2E + visual | `archives-dark-100-ru` | verified |
| `export` | Экспорт CSV, XML, HTML и XLSX | `export` | Rust + E2E + visual | `export-dark-100-ru` | verified |
| `unlock` | Блокировки, мягкое и жёсткое снятие | `unlock` | Rust + E2E + visual | `unlock-dark-100-ru` | verified |
| `shred` | Необратимое затирание с предпросмотром | `shred` | Rust + E2E + visual | `shred-dark-100-ru` | verified |
| `monitor` | Мониторинг записи, пороги и исключения | `monitor` | Rust + E2E + visual | `monitor-dark-100-ru` | verified |
| `elevation` | Статус прав и запрос повышения | — (`app_elevation_status`) | Vitest + E2E + visual | `elevation-dark-100-ru` | verified |
| `settings` | Настройки, тема, язык, пути | `config` | Rust + E2E + visual | `settings-dark-100-ru` | verified |
| `about` | Версия, WebView2, лицензии, `data` | — (`app_about_info`) | Rust + E2E + visual | `about-dark-100-ru` | verified |

## Как проверялось

- Rust: `tests/contract/{core,analysis,operations,filesystem,inspection,settings}_commands.rs`.
- CLI: `tests/cli_contract.rs` и golden-фикстуры `tests/contract/fixtures/`; команды возвращают те
  же JSON-конверты, что и Tauri-команды.
- Frontend: Vitest для 24 экранов, Playwright E2E на MockTransport, axe-аудит, полная визуальная
  матрица `24 × 2 × 4 × 2` и PR-набор.
- Скриншоты: `apps/freespace-desktop/ui/tests/visual/baselines/`, галерея
  `apps/freespace-desktop/ui/tests/visual/gallery/index.html`.

Экраны `elevation` и `about` не имеют отдельной CLI-команды по спецификации: повышение прав
запускается только из GUI, а сведения о версии CLI печатает в `--version`. Это зафиксировано в
`specs/002-native-ui-redesign/spec.md` (FR-104, FR-112) и не является пробелом паритета.
