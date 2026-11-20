# Contract: Portable Release

**Feature**: `003-cross-platform-portable` | **Version**: 2.0.0

## 1. Триггер

```yaml
on:
  workflow_dispatch:
    inputs:
      tag:
        description: Existing tag (vX.Y.Z)
        required: true
      draft:
        description: Create a draft release
        type: boolean
        default: true
```

Правила:

- workflow НЕ запускается на `push: tags`;
- тег MUST существовать на remote;
- тег MUST совпадать с `v<workspace.package.version>`;
- при несовпадении — падение до сборки.

## 2. Артефакты

| Платформа | Файл | Состав |
|---|---|---|
| Windows x64 | `freespace-portable-<ver>-windows-x64.zip` | `freespace-gui.exe`, `freespace.exe`, `docs/`, `data/README.txt`, опционально `webview2/` |
| Linux x86_64 | `freespace-portable-<ver>-linux-x86_64.tar.gz` | `freespace-gui`, `freespace`, `docs/`, `data/README.txt`, `run.sh` |
| Все | `SHA256SUMS.txt` | SHA-256 по каждому архиву |

Запрещено:

- NSIS/MSI/deb/rpm/AppImage/Flatpak/Snap;
- подпись кода (сертификата нет);
- публикация без ручного подтверждения.

## 3. Сборка

Windows:

```powershell
npm --prefix apps/freespace-desktop/ui ci
npm --prefix apps/freespace-desktop/ui run build
pwsh -File installer/portable.ps1
```

Linux:

```bash
npm --prefix apps/freespace-desktop/ui ci
npm --prefix apps/freespace-desktop/ui run build
bash installer/portable.sh
```

Оба скрипта читают версию из `Cargo.toml` и собирают GUI+CLI без инсталлятора.

## 4. GitHub Release

- создаётся/обновляется **черновик** с `tag_name`, названием `FreeSpace <tag>` и артефактами;
- тело релиза — секция CHANGELOG для версии;
- повторный запуск для того же тега обновляет существующий черновик;
- публикация — вручную в GitHub UI;
- при падении любого matrix-джоба финальный job не создаёт черновик.

## 5. Версии

- Источник истины: `Cargo.toml` → `[workspace.package].version`.
- `scripts/version.mjs check` проверяет:
  - `Cargo.toml`;
  - `apps/freespace-desktop/src-tauri/tauri.conf.json` (если поле есть);
  - `apps/freespace-desktop/ui/package.json` и `package-lock.json`;
  - входной тег.
- `scripts/version.mjs set <semver>` синхронизирует перечисленные файлы.
- Portable-скрипты не содержат хардкода версии.

## 6. Проверки после релиза

- скачать оба архива, распаковать в чистый каталог;
- Windows: GUI запускается, `freespace --version` совпадает, данные в `data/`;
- Linux: GUI запускается при установленных WebKitGTK/GTK3, `freespace --json capabilities`
  показывает `platform: linux`;
- `SHA256SUMS.txt` совпадает;
- установщики отсутствуют;
- в релизе нет подписи, и это указано в примечаниях.
