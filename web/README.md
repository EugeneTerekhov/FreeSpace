# FreeSpace — UI mock / reference

An interactive mock of the new FreeSpace interface language. It is a visual
reference for the native utility redesign, not part of the application. It is
intended to move to a separate GitHub Pages project. The Tauri + React product
frontend will reuse the tokens and visual reference; the product UI is a
separate implementation. Every product picture is an authored React scene from
the Stageshow kit, not a screenshot.

## Русская версия

### Что это

Интерактивный макет нового языка интерфейса FreeSpace. Это визуальный
референс для редизайна нативной утилиты, а не часть приложения; его
предполагается вынести в отдельный проект. Продуктовый frontend на Tauri +
React будет использовать эти токены и референс, но сам UI утилиты — отдельная
реализация. Все иллюстрации — React-сцены (`Stage`, `Window`, `Count`,
`Collapse` из Stageshow), поэтому они масштабируются, уважают
`prefers-reduced-motion` и не требуют растровых скриншотов.

Копирайт страницы — на русском. Иконка продукта взята из
`apps/freespace-desktop/src-tauri/icons/icon.png`. Внешних шрифтов, аналитики и сетевых
запросов нет: страница работает офлайн.

### Запуск

```powershell
cd web
npm install
npm run dev        # http://127.0.0.1:5173
npm run typecheck
npm run build      # dist/
```

### Структура

- `src/App.tsx` — страница: шапка, hero, метрики, bento, CLI, сборка, подвал.
- `src/scenes/HeroScene.tsx` / `hero.css` — сцена карты диска и пересканирования
  (три композиции: wide 1120, medium 660, compact 358).
- `src/scenes/BentoTiles.tsx` / `bento.css` — пять плиток: дубликаты, скорость
  сканирования, снимки, CLI и офлайн-утверждение.
- `src/site.css` — страничная типографика, сетка, кнопки, подвал.
- `src/kit/` — копия переносимого Stageshow-кита. В `kit/phone.tsx` убран
  неиспользуемый импорт, чтобы строгий `tsc` хоста проходил.
- `public/docs/` — копии `README.md`, `docs/user-guide.md`, `docs/cli.md`,
  чтобы ссылки со страницы работали в собранной версии.

### Проверка

Скилл Stageshow лежит в `.agents/skills/stageshow`; его скрипты проверяют
сцены в headless Chrome. Из папки скилла:

```powershell
node scripts/film.mjs "http://127.0.0.1:5173/" --select ".hero-stage--wide" --width 1280 --still --reduced --gate
node scripts/film.mjs "http://127.0.0.1:5173/" --select ".hero-stage--compact" --width 390 --still --reduced --gate
node scripts/film.mjs "http://127.0.0.1:5173/" --select ".fs-bento" --width 1280 --still --reduced --gate
node scripts/film.mjs "http://127.0.0.1:5173/" --select ".fs-bento" --width 390 --still --reduced --gate
node scripts/film.mjs "http://127.0.0.1:5173/" --select ".fs-term-stage" --width 1280 --still --gate
node scripts/film.mjs "http://127.0.0.1:5173/" --select ".fs-term-stage" --width 390 --still --gate
node scripts/layout.mjs "http://127.0.0.1:5173/" --select "body" --width 1280 --strict
node scripts/layout.mjs "http://127.0.0.1:5173/" --select "body" --width 390 --strict
```

Результаты, композиционные решения и непроверенные места — в
`REHEARSAL.md`.

## English version

### What it is

An interactive mock of the new FreeSpace interface language. It is a visual
reference for the native utility redesign, not part of the application; it is
intended to move to a separate project. Every product picture is an authored
React scene built with the Stageshow kit (`Stage`, `Window`, `Count`,
`Collapse`), so it scales, honours `prefers-reduced-motion`, and needs no
bitmap screenshots.

The page copy is Russian. The product icon is the real
`apps/freespace-desktop/src-tauri/icons/icon.png`. There are no external fonts, analytics or
network requests: the page runs fully offline.

### Run

```powershell
cd web
npm install
npm run dev        # http://127.0.0.1:5173
npm run typecheck
npm run build      # dist/
```

### Structure

- `src/App.tsx` — the page: header, hero, metrics, bento, CLI, build, footer.
- `src/scenes/HeroScene.tsx` / `hero.css` — the drive-map and rescan scene
  (wide 1120, medium 660, compact 358 compositions).
- `src/scenes/BentoTiles.tsx` / `bento.css` — five tiles: duplicates, scan
  speed, snapshots, CLI and the offline statement.
- `src/site.css` — page typography, grid, buttons and footer.
- `src/kit/` — a copy of the portable Stageshow kit. One unused import was
  removed from `kit/phone.tsx` so the host's strict `tsc` passes.
- `public/docs/` — copies of `README.md`, `docs/user-guide.md` and `docs/cli.md`
  so the page's links work in a production build.

### Verification

The Stageshow skill lives in `.agents/skills/stageshow`; its scripts check the
scenes in headless Chrome. From the skill folder:

```powershell
node scripts/film.mjs "http://127.0.0.1:5173/" --select ".hero-stage--wide" --width 1280 --still --reduced --gate
node scripts/film.mjs "http://127.0.0.1:5173/" --select ".hero-stage--compact" --width 390 --still --reduced --gate
node scripts/film.mjs "http://127.0.0.1:5173/" --select ".fs-bento" --width 1280 --still --reduced --gate
node scripts/film.mjs "http://127.0.0.1:5173/" --select ".fs-bento" --width 390 --still --reduced --gate
node scripts/film.mjs "http://127.0.0.1:5173/" --select ".fs-term-stage" --width 1280 --still --gate
node scripts/film.mjs "http://127.0.0.1:5173/" --select ".fs-term-stage" --width 390 --still --gate
node scripts/layout.mjs "http://127.0.0.1:5173/" --select "body" --width 1280 --strict
node scripts/layout.mjs "http://127.0.0.1:5173/" --select "body" --width 390 --strict
```

The results, the composition decisions and what was not checked are in
`REHEARSAL.md`.
