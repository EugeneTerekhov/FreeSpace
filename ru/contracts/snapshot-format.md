# Контракт: формат снимка `.fssnap`

**Feature**: `001-freespace-disk-analyzer` | **Version**: 1 | **Date**: 2026-11-15

Снимок — иммутабельный локальный файл с состоянием диска для последующего сравнения
(FR-038…FR-041, SC-008).

## Контейнер

```text
+----------------+---------+-----------------------------+
| Magic "FSSNAP" | u32 ver | zstd-сжатый CBOR-конверт     |
+----------------+---------+-----------------------------+
```

- Magic: 8 байт ASCII `FSSNAP\0\0`.
- `ver`: u32 little-endian, версия контейнера (начинается с `1`).
- Далее — zstd-кадр, внутри — CBOR-документ `SnapshotPayload`.
- Контрольная сумма SHA-256 вычисляется по несжатому CBOR и хранится в метаданных.

## SnapshotPayload (CBOR, схема v1)

```text
SnapshotPayload {
  format_version: u32,          // 1
  app_version: string,
  created_at: string,           // ISO 8601 UTC
  checksum: string,             // SHA-256 несжатого CBOR
  volume: {
    id: string,
    mount_point: string,
    label: string,
    filesystem: string,
    total_bytes: u64,
    free_bytes: u64,
    cluster_size: u32,
    scan_method: "mft" | "usn" | "walk",
    elevated: bool
  },
  roots: [string],
  node_count: u64,
  settings: {
    units: "binary" | "decimal",
    follow_links: bool,
    include_ads: bool
  },
  nodes: ColumnarNodes
}
```

## ColumnarNodes

Колонночное представление дерева для компактности и потокового сравнения. Все массивы имеют длину
`node_count`; индексы — u32.

| Массив | Тип | Описание |
|--------|-----|----------|
| `parent` | [u32] | Индекс родителя; `0xFFFFFFFF` — корень |
| `name_offset` | [u32] | Смещение имени в `name_blob` |
| `name_length` | [u16] | Длина имени в байтах UTF-8 |
| `name_blob` | bytes | Конкатенация имён |
| `kind` | [u8] | 0=file, 1=dir, 2=symlink, 3=junction, 4=hardlink |
| `logical_size` | [u64] | Логический размер |
| `on_disk_size` | [u64] | Размер на диске |
| `physical_size` | [u64] | Физический размер |
| `mtime` | [i64] | Unix-время (UTC, секунды) |
| `attributes` | [u32] | Битовая маска атрибутов |
| `file_ref` | [u64] | FRN NTFS (0, если неизвестно) |
| `access` | [u8] | 0=ok, 1=denied, 2=error |
| `owner_id` | [u32] | Индекс в `owners`; `0xFFFFFFFF` — неизвестно |
| `owners` | [string] | Таблица SID/имён владельцев |

## Правила совместимости

- Снимок читается, если `format_version` поддерживается приложением.
- Сравнение допускается только при совпадении `volume.id`; иначе — ошибка `E_INCOMPATIBLE_SNAPSHOT`.
- Более новые `format_version` отклоняются с понятным сообщением; конвертация форматов не
  выполняется автоматически.
- Повреждение контрольной суммы → ошибка `E_CORRUPTED_SNAPSHOT`, файл не изменяется.
- Снимок MUST NOT содержать содержимое файлов и полные ACL; только агрегированные метаданные.

## Сравнение

- Слияние двух потоков узлов по относительным путям (от корня) без загрузки всех узлов в память.
- Результат: `added`, `removed`, `changed`, `growth_by_directory`.
- Для каталогов прирост считается по `physical_size` (по умолчанию) с возможностью переключения на
  логический размер.
- Отчёт сравнения экспортируется в форматы из `contracts/report-formats.md`.

## Версионирование

- `format_version` увеличивается при несовместимом изменении схемы.
- Добавление новых колонок допускается с сохранением обратной совместимости и повышением
  минорной версии приложения.
