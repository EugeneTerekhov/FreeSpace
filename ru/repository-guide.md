# Как пользоваться репозиторием FreeSpace

## 1. Назначение репозитория

Здесь ведётся разработка **FreeSpace** — нативной утилиты для Windows, которая анализирует и
освобождает дисковое пространство. Сейчас репозиторий содержит только SDD-документацию
(спецификация, план, задачи); исходный код ещё не создан.

Источник истины — канонические артефакты в
[`specs/001-freespace-disk-analyzer/`](../specs/001-freespace-disk-analyzer/). Каталоги
[`ru/`](.) и [`en/`](../en/) содержат зеркала документации на русском и английском языках.

## 2. Требования

| Инструмент | Версия | Назначение |
|------------|--------|------------|
| Windows | 10 21H2+ или 11, x64 | Целевая платформа |
| Git | 2.40+ | Версионный контроль |
| Rust | stable 1.85+, edition 2024 | Реализация (после старта разработки) |
| uv | 0.5+ | Установка Spec Kit CLI (опционально) |
| Python | 3.11+ | Требование Spec Kit CLI (ставится через uv) |

Установка Spec Kit CLI (нужна только для работы с SDD-командами):

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
│   ├── spec.md                        # спецификация: 23 истории, 95 FR, 16 SC
│   ├── plan.md                        # план: стек, архитектура, Constitution Check
│   ├── research.md                    # 22 технических решения и альтернативы
│   ├── data-model.md                  # 20 сущностей и правила целостности
│   ├── quickstart.md                  # сценарии сквозной проверки
│   ├── contracts/                     # CLI, форматы отчётов, формат снимков
│   ├── checklists/                    # чек-листы качества требований
│   └── tasks.md                       # 161 задача по 26 фазам
├── ru/                                # русские зеркала документации
├── en/                                # английские переводы документации
├── AGENTS.md                          # правила для AI-агентов
└── README.md                          # краткое описание и ссылки
```

## 4. С чего начать

Рекомендуемый порядок чтения:

1. [`ru/constitution.md`](constitution.md) — принципы, которые нельзя нарушать.
2. [`ru/spec.md`](spec.md) — что и зачем строим.
3. [`ru/plan.md`](plan.md) — как строим: стек и архитектура.
4. [`ru/research.md`](research.md) — почему выбраны конкретные технические решения.
5. [`ru/data-model.md`](data-model.md) — сущности и связи.
6. [`ru/contracts/`](contracts/) — внешние интерфейсы (CLI, отчёты, снимки).
7. [`ru/tasks.md`](tasks.md) — что делать по шагам.
8. [`ru/quickstart.md`](quickstart.md) — как проверять результат.

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

Текущая фича уже создана (`001-freespace-disk-analyzer`). Новая фича создаётся командой
`/speckit.specify` — она получит следующий номер (`002-…`), а `.specify/feature.json`
переключится на неё.

## 6. Правило двуязычности

- Канонические артефакты Spec Kit пишутся **на русском** в `specs/<feature>/`.
- В `ru/` лежат русские зеркала, в `en/` — английские переводы.
- Любое изменение канонического файла нужно перенести в оба каталога:
  1. отредактировать `specs/<feature>/<file>`;
  2. скопировать в `ru/`;
  3. перевести в `en/`;
  4. проверить, что в `en/` нет кириллицы, а ID (FR-, SC-, T-, CHK-) не изменились.

Быстрая проверка:

```powershell
Select-String -Path en\*.md,en\contracts\*.md,en\checklists\*.md -Pattern '[\u0400-\u04FF]'
```

## 7. Разработка (когда появится код)

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
Чек-лист `checklists/quality.md` принадлежит ревьюеру: агенты не должны отмечать его пункты.

## 8. Полезные команды

```powershell
specify --version                 # версия Spec Kit CLI
specify check                     # проверка окружения и интеграций
git status                        # состояние репозитория
git log --oneline -5              # последние коммиты
cargo build --workspace           # сборка (после старта реализации)
cargo test --workspace            # тесты
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
- **`cargo`-команды падают.** Это ожидаемо: реализация ещё не начата, workspace появится на
  Фазе 1 задач.

## 10. Ссылки

- [README](../README.md)
- [Конституция](constitution.md)
- [Спецификация](spec.md)
- [План](plan.md)
- [Задачи](tasks.md)
- [Документация Spec Kit](https://github.github.io/spec-kit/)
