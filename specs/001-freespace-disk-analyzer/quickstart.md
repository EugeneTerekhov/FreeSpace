# Quickstart: проверка FreeSpace

**Feature**: `001-freespace-disk-analyzer` | **Date**: 2026-11-15

Руководство по сборке и сквозной проверке после реализации. Сценарии соответствуют пользовательским
историям и критериям успеха из [spec.md](./spec.md).

## Предварительные требования

- Windows 10 21H2+ или Windows 11, x64.
- Rust stable 1.85+ (edition 2024), компонент `clippy` и `rustfmt`.
- Для административных сценариев — терминал, запущенный от имени администратора.
- Свободный тестовый NTFS-том или VHD (рекомендуется ≥ 2 ГБ) для проверки MFT, прав и карантина.

## Сборка и запуск

```powershell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo run -p freespace-gui
```

CLI:

```powershell
cargo run -p freespace-cli -- --help
cargo run -p freespace-cli -- drives --json
```

## Сценарии проверки

### 1. Сканирование и дерево (US1)

```powershell
cargo run -p freespace-cli -- scan C:\ --json
cargo run -p freespace-cli -- tree C:\ --depth 2 --json
```

Ожидается: валидный JSON, сумма размеров корня согласована с дочерними узлами, недоступные
каталоги помечены, время анализа тома до 8 ТБ — не более 60 с (на SSD).

### 2. Treemap (US2)

Запустить GUI, открыть карту, проверить:

- площадь прямоугольников пропорциональна размерам;
- зум и «хлебные крошки» работают;
- подсказки содержат имя, путь и долю;
- раскраска по типам совпадает с легендой;
- на тестовом томе с 1 млн файлов интерфейс остаётся отзывчивым.

### 3. Разблокировка (US3)

Подготовка: запустить процесс, удерживающий файл (например, открыть файл в редакторе или
использовать тестовый помощник).

```powershell
cargo run -p freespace-cli -- unlock <file> --json
cargo run -p freespace-cli -- unlock <file> --soft --yes --json
```

Ожидается: список процессов и дескрипторов; после мягкого закрытия файл доступен; запись в журнале.

### 4. Топ файлов и типы (US4)

```powershell
cargo run -p freespace-cli -- top C:\ --limit 100 --json
cargo run -p freespace-cli -- types C:\ --json
```

Ожидается: 100 крупнейших файлов, суммы по расширениям совпадают с деревом.

### 5. Дубликаты (US5)

```powershell
cargo run -p freespace-cli -- duplicates <test-dir> --algorithm sha256 --json
```

Ожидается: группы только с побайтово одинаковыми файлами; ложных совпадений нет; одна копия
всегда остаётся.

### 6. Шредер и удаление при перезагрузке (US6)

```powershell
cargo run -p freespace-cli -- shred <test-dir>\secret.txt --passes 3 --pattern random --yes --json
```

Ожидается: предупреждение о необратимости; запись в журнале; восстановление независимым
инструментом невозможно (HDD). На SSD — предупреждение и TRIM.

### 7. Устаревшие файлы и пустые папки (US7)

```powershell
cargo run -p freespace-cli -- cleanup suggest <test-dir> --older-than 180 --json
cargo run -p freespace-cli -- cleanup empty-dirs <test-dir> --json
```

Ожидается: только объекты, у которых максимум из дат доступа/изменения старше порога; пустые
каталоги перечислены полностью.

### 8. Снимки (US8)

```powershell
cargo run -p freespace-cli -- snapshot create C:\ --out before.fssnap --json
# ... изменения на диске ...
cargo run -p freespace-cli -- snapshot create C:\ --out after.fssnap --json
cargo run -p freespace-cli -- snapshot diff before.fssnap after.fssnap --json
```

Ожидается: отчёт содержит все добавленные/удалённые/изменённые объекты и прирост по каталогам.

### 9. Безопасные операции (US9)

Проверить в GUI и CLI:

- предпросмотр с выбором «Корзина / Карантин / Безвозвратно»;
- удаление в карантин при нехватке места — предупреждение и выбор;
- восстановление из карантина;
- запись в журнале и отмена, где применимо.

### 10. Точный учёт места (US10)

На тестовом наборе с hard links, junction, сжатыми и разреженными файлами и ADS:

```powershell
cargo run -p freespace-cli -- scan <test-dir> --json
```

Ожидается: физический размер с дедупликацией, логический — по числу ссылок; ADS учтены.

### 11. Длинные пути (US11)

Создать путь длиной > 260 символов (например, вложенными каталогами) и выполнить:

```powershell
cargo run -p freespace-cli -- rename <long-path> --regex "a" --replacement "b" --yes --json
```

Ожидается: успешное переименование без включения `LongPathsEnabled`.

### 12. Массовое переименование (US12)

```powershell
cargo run -p freespace-cli -- rename <test-dir>\* --regex "IMG_(\d+)" --replacement "Photo_$1" --preview --json
```

Ожидается: предпросмотр, обнаружение конфликтов, откат после применения.

### 13. Атрибуты и права (US13)

```powershell
cargo run -p freespace-cli -- attrs <test-dir> --clear read-only --yes --json
cargo run -p freespace-cli -- acl <test-dir> --owner <account> --yes --json
```

Ожидается: отчёт об успешных и неуспешных объектах; при нехватке прав — запрос повышения.

### 14. Метаданные и владельцы (US14, US15)

```powershell
cargo run -p freespace-cli -- scan <media-dir> --json
cargo run -p freespace-cli -- export <media-dir> --format csv --out report.csv --include owners
```

Ожидается: корректные EXIF/битрейт/разрешение/версия PE; суммы по владельцам совпадают с деревом.

### 15. Архивы (US16)

```powershell
cargo run -p freespace-cli -- archives <test-dir> --json
```

Ожидается: содержимое ZIP/7z/TAR/ISO без распаковки; корректная обработка паролей и повреждений.

### 16. Экспорт (US17)

```powershell
cargo run -p freespace-cli -- export C:\ --format csv --out tree.csv --json
cargo run -p freespace-cli -- export C:\ --format xml --out tree.xml --json
cargo run -p freespace-cli -- export C:\ --format html --out tree.html --json
cargo run -p freespace-cli -- export C:\ --format xlsx --out tree.xlsx --json
```

Ожидается: все файлы открываются целевыми инструментами, суммы совпадают.

### 17. CLI и расписание (US18)

```powershell
cargo run -p freespace-cli -- scan C:\ --json | ConvertFrom-Json
cargo run -p freespace-cli -- schedule export default --out task.xml
```

Ожидается: валидный JSON, документированные коды возврата, XML-задание принимается Планировщиком.

### 18. Контекстное меню (US19)

```powershell
cargo run -p freespace-cli -- config get context_menu_enabled
cargo run -p freespace-cli -- config set context_menu_enabled true
```

Проверить пункты меню для файла, папки и диска; затем выключить и очистить обе области:

```powershell
cargo run -p freespace-cli -- config set context_menu_enabled false
cargo run -p freespace-cli -- context-menu uninstall --all --yes
```

Ожидается: после установки пунктов нет; включение настройки добавляет их, выключение убирает;
`uninstall --all` идемпотентен. Портативная сборка (`cargo build --release --no-default-features
-p freespace-cli -p freespace-gui`) не содержит команды `context-menu`.

### 19. Мониторинг (US20)

```powershell
cargo run -p freespace-cli -- monitor --threshold 1073741824 --json
```

Ожидается: поток событий с процессом, путём и скоростью; уведомление при превышении порога.

### 20. Системные каталоги (US21)

Запустить GUI от администратора, открыть `C:\System Volume Information`, убедиться в доступе;
запустить без прав — проверить корректную пометку недоступности.

### 21. Дашборд, настройки, локализация (US22, US23)

```powershell
cargo run -p freespace-cli -- drives --json
cargo run -p freespace-cli -- config set language ru
cargo run -p freespace-cli -- config set language en
```

Ожидается: все тома в сводке; полная локализация; настройки, журнал и карантин находятся в
`<папка приложения>\data`, а `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` и временные каталоги
не создаются. Проверить удаление: после деинсталляции каталог `data` и пункты меню отсутствуют.

## Критерии готовности к релизу

- `cargo test --workspace` — зелёный.
- `cargo clippy --workspace --all-targets -- -D warnings` — без предупреждений.
- Все сценарии выше выполнены на чистой машине Windows 10 и Windows 11.
- Административные сценарии (`FREESPACE_ADMIN_TESTS=1`) пройдены на тестовом NTFS-томе.
- Контрольные суммы отчётов совпадают с эталонными.
