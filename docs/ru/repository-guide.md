# Как устроен репозиторий FreeSpace

## 1. Назначение

Здесь ведётся разработка **FreeSpace** — нативной утилиты для Windows, которая анализирует и
освобождает дисковое пространство. Репозиторий содержит SDD-документацию, Rust-крейты и
Tauri 2 + React клиент с тестами.

Источник истины — канонические артефакты в [`../../specs/`](../../specs/). Общие русские
документы лежат в [`docs/ru/`](.), английские переводы — в [`../en/`](../en/).

## 2. Требования

| Инструмент | Версия | Назначение |
|------------|--------|------------|
| Windows | 10 21H2+ или 11, x64 | Целевая платформа |
| Git | 2.40+ | Версионный контроль |
| Rust | stable 1.85+, edition 2024 | Основной код |
| Node.js | 20.19+ | Интерфейс Tauri 2 + React |
| uv | 0.5+ | Установка Spec Kit CLI (опционально) |
| Python | 3.11+ | Требование Spec Kit CLI (ставится через uv) |

Установка Spec Kit CLI нужна только для работы с SDD-командами:

```powershell
winget install astral-sh.uv
uv python install 3.12
uv tool install specify-cli --python 3.12
```

Убедитесь, что `%USERPROFILE%\.local\bin` добавлен в `PATH`, и проверьте:

```powershell
specify --version
```

## 3. Структура репозитория

```text
FreeSpace/
├── .specify/                          # каркас Spec Kit: шаблоны, скрипты, конституция
│   └── memory/constitution.md         # конституция проекта (обязательна к соблюдению)
├── .opencode/commands/                # команды /speckit.* для OpenCode
├── specs/001-freespace-disk-analyzer/ # канонические артефакты (русский язык)
│   ├── spec.md                        # спецификация
│   ├── plan.md                        # план: стек, архитектура, Constitution Check
│   ├── research.md                    # технические решения и альтернативы
│   ├── data-model.md                  # сущности и правила целостности
│   ├── quickstart.md                  # сценарии сквозной проверки
│   ├── contracts/                     # CLI, форматы отчётов, формат снимков
│   ├── checklists/                    # чек-листы качества требований
│   └── tasks.md                       # задачи по фазам
├── specs/002-native-ui-redesign/      # спецификация нативного интерфейса
├── specs/003-cross-platform-portable/ # кроссплатформенная portable-поставка
├── specs/004-multi-volume-snapshot/   # мультитомный снимок
├── docs/ru/                           # русские общие документы
├── docs/en/                           # английские переводы
├── apps/freespace-desktop/            # Tauri 2 + React клиент
├── crates/                            # Rust-крейты
├── AGENTS.md                          # правила для AI-агентов
└── README.md                          # краткое описание и ссылки
```

## 4. С чего начать

1. [`constitution.md`](../../.specify/memory/constitution.md) — принципы, которые нельзя нарушать.
2. [`spec.md`](../../specs/001-freespace-disk-analyzer/spec.md) — что и зачем строим.
3. [`plan.md`](../../specs/001-freespace-disk-analyzer/plan.md) — как строим: стек и архитектура.
4. [`research.md`](../../specs/001-freespace-disk-analyzer/research.md) — почему выбраны решения.
5. [`data-model.md`](../../specs/001-freespace-disk-analyzer/data-model.md) — сущности и связи.
6. [`contracts/`](../../specs/001-freespace-disk-analyzer/contracts/) — внешние интерфейсы.
7. [`tasks.md`](../../specs/001-freespace-disk-analyzer/tasks.md) — история задач.
8. [`quickstart.md`](../../specs/001-freespace-disk-analyzer/quickstart.md) — как проверять результат.

## 5. Как устроен SDD-процесс

Проект использует [GitHub Spec Kit](https://github.com/github/spec-kit). Команды вызываются в
чате OpenCode (не в терминале) и читают активную фичу из `.specify/feature.json`.

| Этап | Команда | Результат |
|------|---------|-----------|
| Конституция | `/speckit.constitution` | `.specify/memory/constitution.md` |
| Спецификация | `/speckit.specify` | `spec.md` и `checklists/requirements.md` |
| Уточнения | `/speckit.clarify` | раздел `Clarifications` в `spec.md` |
| План | `/speckit.plan` | `plan.md`, `research.md`, `data-model.md`, `contracts/`, `quickstart.md` |
| Чек-лист | `/speckit.checklist` | `checklists/<domain>.md` |
| Задачи | `/speckit.tasks` | `tasks.md` |
| Анализ | `/speckit.analyze` | отчёт о консистентности (без изменений файлов) |
| Реализация | `/speckit.implement` | код по задачам |
| Сходимость | `/speckit.converge` | дополняет `tasks.md` недостающими задачами |

## 6. Двуязычность

- Канонические артефакты Spec Kit пишутся **на русском** в `specs/<feature>/`.
- Общие русские документы лежат в `docs/ru/`.
- Английские переводы — в `docs/en/`; отдельное зеркало `ru/` не создаётся.
- Любое изменение канонического файла нужно перевести в `docs/en/`:
  1. отредактировать `specs/<feature>/<file>` или `docs/ru/<file>`;
  2. перевести в `docs/en/`;
  3. проверить, что в `docs/en/` нет кириллицы, а ID (FR-, SC-, T-, CHK-) не изменились.

Быстрая проверка:

```powershell
Select-String -Path docs\en\*.md,docs\en\contracts\*.md,docs\en\checklists\*.md -Pattern '[\u0400-\u04FF]'
```

## 7. Разработка

1. Выберите следующую незавершённую задачу в `tasks.md` с учётом зависимостей.
2. Для критических модулей сначала напишите падающий тест (принцип V конституции).
3. Реализуйте задачу, держа файлы в границах, указанных в описании.
4. Отметьте задачу `[x]` только после прохождения тестов и проверок.
5. Обновите документацию, если поведение изменилось.

Обязательные проверки перед коммитом:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Коммиты — по [Conventional Commits](https://www.conventionalcommits.org/):
`feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`.

Чек-лист `checklists/requirements.md` поддерживается командами specify/clarify.

## 8. Полезные команды

```powershell
specify --version                 # версия Spec Kit CLI
specify check                     # проверка окружения и интеграций
git status                        # состояние репозитория
git log --oneline -5              # последние коммиты
cargo build --workspace           # сборка
cargo test --workspace            # тесты
```

Интерфейс:

```powershell
cd apps/freespace-desktop/ui
npm ci
npm run tauri dev
npm run typecheck
npm run lint
npm run test:unit
```

## 9. Частые проблемы

- **`uv` или `specify` не найдены.** Перезапустите терминал или добавьте
  `%USERPROFILE%\.local\bin` в `PATH`.
- **Spec Kit требует Python 3.11+.** Используйте `uv python install 3.12` и
  `uv tool install specify-cli --python 3.12`.
- **Команда не находит фичу.** Проверьте `.specify/feature.json`; он не коммитится и создаётся
  при вызове `/speckit.specify`. Альтернатива — переменная `SPECIFY_FEATURE_DIRECTORY`.
- **Не редактируйте `.specify/templates/` и `.opencode/commands/` вручную.** Для изменений
  процесса используйте пресеты и проектные overrides, как описано в документации Spec Kit.

## 10. Ссылки

- [README](../../README.md)
- [Конституция](../../.specify/memory/constitution.md)
- [Спецификация](../../specs/001-freespace-disk-analyzer/spec.md)
- [План](../../specs/001-freespace-disk-analyzer/plan.md)
- [Задачи](../../specs/001-freespace-disk-analyzer/tasks.md)
- [Документация Spec Kit](https://github.github.io/spec-kit/)
