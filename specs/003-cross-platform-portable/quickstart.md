# Quickstart: FreeSpace 2.0

**Feature**: `003-cross-platform-portable` | **Date**: 2026-11-19

Сценарии выполняются на изолированных данных: временные каталоги, тестовые VHD/loop-образы.
Реальные пользовательские данные не трогать.

## 0. Сборка и тесты

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace --all-targets --no-default-features
```

Windows-only:

```powershell
$env:FREESPACE_ADMIN_TESTS = "0"
cargo test --workspace
```

Linux-зависимости:

```bash
sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
  libxdo-dev build-essential
```

## 1. Linux portable

```bash
tar -xzf freespace-portable-2.0.0-linux-x86_64.tar.gz
cd freespace-portable
./freespace --json capabilities
./freespace scan /tmp/freespace-fixture --json
./freespace tree /tmp/freespace-fixture --json
./freespace delete --mode recycle /tmp/freespace-fixture/old.bin --yes
./freespace delete --mode permanent /tmp/freespace-fixture/junk.bin --yes
./freespace-gui
```

Ожидается:

- `capabilities` показывает `platform: linux`, `mft: false`, `walk: true`, `trash: true`,
  `shred: false`, `acl: false`, `case_sensitive: true`, `max_path: 4096`;
- `scan` явно сообщает метод `directory-walk`;
- удалённый в корзину файл появляется в `~/.local/share/Trash` и восстанавливается файловым
  менеджером;
- permanent удаляет файл;
- данные приложения лежат в `data/` рядом с бинарниками;
- `shred`, `acl`, `attrs`, `unlock` возвращают `E_UNSUPPORTED` (код 8).

Проверка недоступной корзины:

```bash
# read-only или сетевой том
./freespace delete --mode recycle /mnt/ro/file.bin --yes
```

Ожидается: ошибка до изменения данных, предложение permanent, без молчаливого удаления.

## 2. Windows portable

```powershell
Expand-Archive freespace-portable-2.0.0-windows-x64.zip -DestinationPath C:\Temp\FreeSpace
C:\Temp\FreeSpace\freespace.exe --json capabilities
C:\Temp\FreeSpace\freespace.exe scan E:\ --json
C:\Temp\FreeSpace\freespace-gui.exe
```

Ожидается:

- `capabilities` показывает `platform: windows`, `mft: true`, `usn: true`, `trash: true`,
  `shred: true`, `acl: true`, `attrs: true`, `ads: true`, `unlock: true`;
- MFT-сканирование и USN-инкремент работают;
- удаление в Корзину восстанавливается Проводником;
- permanent удаляет файл;
- записей в реестре и установщика нет;
- `data/` рядом с `.exe`.

## 3. Удаление карантина и миграция

1. Подготовить `data/freespace.db` версии 1 с записями журнала (фикстура) и старый
   `settings.toml` с `quarantine_retention_days = 30`.
2. Запустить приложение 2.0.
3. Проверить:
   - `user_version` = 2, таблицы `quarantine` нет;
   - старые записи журнала видны, `restore` недоступен;
   - настройки загрузились, ключ проигнорирован;
   - `data/quarantine` не удалён;
   - `freespace delete --mode quarantine` → `E_INVALID_ARGUMENT`;
   - в GUI нет выбора карантина, в настройках нет срока хранения.

## 4. Экраны

1. Открыть GUI.
2. Проверить сайдбар: 9 экранов, нет «Шредер» и «Файловые операции».
3. Открыть сохранённый `last_screen = "fileops"` (вручную) → должен открыться `tree`;
   `last_screen = "shred"` → `journal`.
4. В дереве вызвать контекстное меню → rename работает.
5. CLI: `freespace shred`, `freespace acl`, `freespace attrs` доступны на Windows; на Linux —
   `E_UNSUPPORTED`.

## 5. Релиз

1. Создать тег `v2.0.0` на коммите с версией 2.0.0 и запушить.
2. GitHub → Actions → Release → Run workflow → tag `v2.0.0`, draft `true`.
3. Дождаться завершения matrix.
4. Проверить черновик релиза:
   - два архива и `SHA256SUMS.txt`;
   - нет установщиков;
   - тело релиза — секция CHANGELOG.
5. Скачать архивы, проверить суммы и запуск.
6. Опубликовать вручную.

Негативные проверки:

- запуск с тегом `v2.0.1` при версии `2.0.0` → падение до сборки;
- обычный `git push` тега без workflow → релиз не создаётся;
- повторный запуск с тем же тегом → черновик обновляется, дубликата нет.

## 6. Офлайн и приватность

- Отключить сеть, выполнить `scan`, `export`, `snapshot`, `delete` — всё работает.
- Убедиться, что в логах нет сетевых вызовов и телеметрии.
- Проверить, что `docs/en/` не содержит кириллицы и ID не изменены.
