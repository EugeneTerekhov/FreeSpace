# Rehearsal: FreeSpace UI mock

## Русская версия

**Gold:** `bento` (сетка: одна hero-плитка, разные виды соседей, Stage 1:1 в
каждой плитке), `hero-demo` (один курсорный жест, именованные цели, пауза перед
действием), `dark-bento` (глубина на тёмном через приподнятую поверхность,
реальная иконка продукта). Структура сохранена; палитра, контент и сцена —
свои.

**Beats:** покой (готовая карта) → нажатие «Пересканировать» → прогресс и
счётчик файлов → «Готово за 38 с» → открывается панель «Можно освободить
312 ГБ» → курсор уходит в покой. `play="once"`, постер — кадр с результатом.
Повтор — кнопкой «Повторить сцену» в хосте (ремоунт сцены), не автолуп.

**Captured:**
- 1280: hero wide (1120), bento, CLI-терминал; reduced-motion постеры,
  контактный лист лупа (800 мс × 11) и лист состояний (`--states`, 8 кадров).
- 850: hero medium (660), reduced-motion постер.
- 390: hero compact (358), bento, CLI-терминал; reduced-motion постеры и
  контактный лист лупа (800 мс × 12).
- `layout.mjs` на `body` при 1280 и 390: `no-issues-detected`.
- Сборка: `npm run build` (tsc + Vite) — зелёная.

**Gate:**
```
rehearsal passed (necessary, not sufficient: references/verify.md and the fresh eye in review.md still stand)
```
на всех семи still-проверках (wide, medium, compact, bento 1280/390,
терминал 1280/390), на обоих контактных листах лупа и на листе состояний.
У листов покрытие `inconclusive`: съёмка шла раз в 800 мс (500 мс у
`--states`), поэтому промежутки длиннее 40 мс не судились.

**Film lines:**
- `lopsided: span.ss-button "Сканирование"` (4 px слева, 8 справа) — это
  штатная асимметрия kit-кнопки: сторона с глифом паддится туже, чтобы иконка
  читалась на одном оптическом отступе с текстом. Кнопка остаётся kit-кнопкой;
  геометрию не переопределял.
- `half-pixel glyph` у иконки статуса (14 px в 97×16) — ширина строки статуса
  нечётная из-за текста; сам глиф чётный и не прыгает при движении. Не гонялся
  за субпикселем: слот статуса не фиксированной ширины.
- `appears, then moves` / `handoff shift` у `States` и `Roll` — снято на 800 мс,
  то есть посреди кроссфейда/переезда ширины. На осевших кадрах (reduced-motion
  и финальные still) метка и статус стоят на месте.
- `appears with no entrance` у `span.ss-count` — это полосы цифр `Count`,
  которые всегда смонтированы; смена значения идёт прокруткой, а не всплытием.
  Кит-поведение, визуально проверено на контактном листе.
- `same-box content change` у `Chart` — линия рисуется по `show`; проверил
  глазами на кадре после раскрытия.
- `jump: fs-usage-copy / ss-meter +7.5px` на переходе в done (первый
  `--states`) — реальный скачок: «файла» менялось на «файлов» и сдвигало
  метр. Исправлено: счётчик называет «объектов», форма не меняется; повторный
  `--states` эту строку больше не выдаёт.
- `settle snap` 34.8px у `States`/`Count` (второй `--states`) — это
  пере-домашнивание полосы цифр скрытого scan-счётчика и ширина `States` по
  самому широкому состоянию; на листе состояний текст «Готово» стоит на своём
  левом крае. Кит-поведение `Count`/`States`.
- `handoff shift` 1–3px у `Roll` — подпись кнопки «Сканирование ↔
  Пересканировать» меняется на той же кнопке; кроссфейд kit-`Roll`.
- `jump: ss-spinner ±2.8px` при смене иконки — Spinner и RefreshCw живут в
  одном слоте `States`; скачок меньше пикселя текста, визуально не читается.
- `appears with no entrance` у `span.ss-count` — полосы цифр `Count`, которые
  всегда смонтированы; смена значения идёт прокруткой, а не всплытием.
  Кит-поведение, визуально проверено на контактном листе.

**Review (fresh eye):** первым читается карта и её блок «Видео»; результат
«312 ГБ» — второй по силе; панель рекомендации не спорит с картой. В bento
одна hero-плитка (дубликаты), рядом — число, диаграмма, компонент и
утверждение; два окна подряд нет. Ни одна кнопка и ни одно слово не срезаны
кропом на осевших кадрах. На 390 сцена пересобрана (358), а не сжата.

**Assumptions:** до этой задачи веб-витрины в репозитории не было; страница
создана с нуля. Копирайт — русский. Иконка — реальный
`apps/freespace-desktop/src-tauri/icons/icon.png`, не нарисованный знак. Кит скопирован в
`src/kit`; в `kit/phone.tsx` убран неиспользуемый импорт под строгий `tsc`.

**Not checked:** покадровая съёмка 40–80 мс каждого beat (тайм-аут полного
листа на 340 кадров; `--states` снят на 500 мс и его покрытие тоже
`inconclusive`); touch-эмуляция и реальное устройство; Firefox/Safari; шов
лупа (сцена `once`, шва нет); 100% масштаб Windows и HiDPI-скриншоты.

## English summary

The hero is a single 10-second authored story: a finished drive map, a rescan
press, a live file count and progress line, a done state, and a recommendation
bar that opens with 312 GB reclaimable. It plays once and has a real replay
button in the host. The bento has one hero tile (byte-verified duplicates), a
stat, a snapshot chart, a CLI component and an offline statement; no two
neighbours are the same kind and one window chrome is used on the page hero.
The page was verified with `film.mjs --gate` on the wide, medium, compact,
bento (1280/390) and terminal (1280/390) stages, and `layout.mjs --strict` on
the host at 1280 and 390. The remaining film lines are kit-level candidates
(icon-side button padding, an odd-width status text box, mid-transition States
sampling, the Count digit strips); no never-intentional line stands. Not
checked: a 40–80 ms sheet for every beat, touch and non-Chromium browsers.
