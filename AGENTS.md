# AGENTS.md — FreeSpace

Правила для AI-агентов, работающих в этом репозитории. Файл обязателен к прочтению перед любыми
изменениями. При конфликте инструкций приоритет: конституция → спецификация → план → задачи →
этот файл.

Rules for AI agents working in this repository. Read this file before making any changes. When
instructions conflict, precedence is: constitution → specification → plan → tasks → this file.

---

## 🇷🇺 Русская версия

### Контекст

FreeSpace — нативная утилита для Windows (Rust + egui/eframe) для анализа и освобождения дискового
пространства. Репозиторий сейчас содержит только SDD-документацию; исходного кода ещё нет.

### Обязательный порядок чтения

1. `.specify/memory/constitution.md` — 7 принципов, нарушать нельзя.
2. `specs/001-freespace-disk-analyzer/spec.md` — 23 истории, 95 FR, 16 SC.
3. `specs/001-freespace-disk-analyzer/plan.md`, `research.md`, `data-model.md`, `contracts/`.
4. `specs/001-freespace-disk-analyzer/tasks.md` — 161 задача, порядок и зависимости.
5. `specs/001-freespace-disk-analyzer/quickstart.md` — сценарии проверки.
6. `checklists/requirements.md` — ворота качества; `checklists/quality.md` принадлежит ревьюеру.

### Иерархия истины

Конституция > спецификация > план > задачи > код. Если код расходится с документацией — правьте
код или сначала обновите документацию через соответствующую `/speckit.*`-команду. Не меняйте
конституцию без явного согласия пользователя и повышения версии.

### Правила SDD

- Используйте команды `/speckit.*` для изменения процесса и артефактов; не переписывайте
  сгенерированные шаблоны вручную.
- Не пропускайте clarify/plan при добавлении новой функциональности.
- Активная фича хранится в `.specify/feature.json` (локальный файл, не коммитится).
- При неоднозначности сначала задайте вопрос или зафиксируйте решение в `Clarifications`;
  не угадывайте молча.
- Отмечайте задачи `[x]` в `tasks.md` только после выполнения критериев готовности.
- Не отмечайте пункты `checklists/quality.md` — это делает только ревьюер.

### Двуязычность

- Канонические артефакты — на русском в `specs/<feature>/`.
- Любое изменение зеркалируется: `ru/` (копия) и `en/` (перевод).
- В `en/` не должно оставаться кириллицы; ID (FR-, SC-, T-, CHK-) менять нельзя.

### Технические правила

- Rust stable 1.85+, edition 2024; workspace из `crates/*` и `apps/freespace-gui`.
- Все вызовы WinAPI — только в `crates/freespace-platform`. Остальные крейты не используют
  `windows` напрямую.
- Разрушающие операции — только через `crates/freespace-ops`: предпросмотр, подтверждение,
  выбор способа удаления, журнал, карантин/Корзина, защита системных путей.
- Длинные пути — через verbatim-префиксы; нельзя полагаться на `LongPathsEnabled`.
- Никаких сетевых вызовов и телеметрии; приложение полностью офлайн.
- Паритет CLI и GUI: у каждой возможности GUI должен быть аналог в CLI с JSON-выводом.
- Бюджеты: анализ 8 ТБ ≤ 60 с на SSD, отклик UI ≤ 200 мс, память ≤ 4 ГБ на 10 млн узлов.
- Тесты критических модулей обязательны (тест → падение → реализация). Фикстуры — только
  временные каталоги и тестовые VHD; реальные пользовательские данные не трогать.
- Административные тесты запускаются только при `FREESPACE_ADMIN_TESTS=1`.
- Перед коммитом обязательны `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test --workspace`.

### Запрещено

- Удалять или изменять реальные файлы пользователя, включая тестовые прогоны на рабочих дисках.
- Запускать разрушающие команды на хост-машине без явного разрешения.
- Редактировать `.specify/templates/`, `.specify/scripts/`, `.opencode/commands/` вручную.
- Коммитить секреты, `.specify/feature.json` и временные файлы.
- Переписывать историю git, делать force push без явного указания.
- Добавлять зависимости без обоснования и фиксации в `plan.md`/`research.md`.
- Расширять область v1 (сеть, облака, кроссплатформенность) без обновления спецификации.

### Критерии готовности задачи

- [ ] Код реализован в указанных файлах.
- [ ] Тесты критических модулей написаны и проходят; для UI — ручная проверка по quickstart.
- [ ] `cargo fmt`, `cargo clippy -D warnings`, `cargo test` — зелёные.
- [ ] Документация обновлена на русском и английском (канон + `ru/` + `en/`).
- [ ] Задача отмечена `[x]` в `tasks.md`.
- [ ] Коммит по Conventional Commits.

### Команды

```powershell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

### Git

- Ветки: `<###>-<feature-name>` (например, `001-freespace-disk-analyzer`).
- Коммиты: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`.
- Один коммит — одна логическая задача или группа.

---

## 🇬🇧 English version

### Context

FreeSpace is a native Windows utility (Rust + egui/eframe) for analyzing and reclaiming disk space.
The repository currently contains only the SDD documentation; there is no source code yet.

### Mandatory reading order

1. `.specify/memory/constitution.md` — 7 principles that must not be violated.
2. `specs/001-freespace-disk-analyzer/spec.md` — 23 stories, 95 FRs, 16 SCs.
3. `specs/001-freespace-disk-analyzer/plan.md`, `research.md`, `data-model.md`, `contracts/`.
4. `specs/001-freespace-disk-analyzer/tasks.md` — 161 tasks, their order and dependencies.
5. `specs/001-freespace-disk-analyzer/quickstart.md` — validation scenarios.
6. `checklists/requirements.md` — quality gate; `checklists/quality.md` belongs to the reviewer.

### Source of truth

Constitution > specification > plan > tasks > code. If code disagrees with documentation, fix the
code or update the documentation first via the appropriate `/speckit.*` command. Do not change the
constitution without explicit user approval and a version bump.

### SDD rules

- Use `/speckit.*` commands to change process artifacts; do not rewrite generated templates by hand.
- Do not skip clarify/plan when adding new functionality.
- The active feature is stored in `.specify/feature.json` (local file, not committed).
- When something is ambiguous, ask first or record a decision under `Clarifications`; never guess
  silently.
- Mark tasks `[x]` in `tasks.md` only after the definition of done is met.
- Do not mark items in `checklists/quality.md`; only the reviewer does that.

### Bilingual policy

- Canonical artifacts are written in Russian in `specs/<feature>/`.
- Every change is mirrored to `ru/` (copy) and `en/` (translation).
- `en/` must contain no Cyrillic; IDs (FR-, SC-, T-, CHK-) must not change.

### Technical rules

- Rust stable 1.85+, edition 2024; workspace of `crates/*` plus `apps/freespace-gui`.
- All WinAPI calls live in `crates/freespace-platform` only. Other crates must not use `windows`
  directly.
- Destructive operations go through `crates/freespace-ops` only: preview, confirmation, deletion
  mode selection, journal, quarantine/Recycle Bin, system-path protection.
- Long paths use verbatim prefixes; never rely on `LongPathsEnabled`.
- No network calls or telemetry; the application is fully offline.
- CLI/GUI parity: every GUI capability needs a CLI equivalent with JSON output.
- Budgets: 8 TB scan ≤ 60 s on SSD, UI response ≤ 200 ms, memory ≤ 4 GB for 10 million nodes.
- Tests are mandatory for critical modules (test → fail → implement). Use only temporary
  directories and test VHDs; never touch real user data.
- Administrative tests run only when `FREESPACE_ADMIN_TESTS=1`.
- Before committing: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace`.

### Prohibited

- Deleting or modifying real user files, including test runs on working disks.
- Running destructive commands on the host machine without explicit permission.
- Editing `.specify/templates/`, `.specify/scripts/`, or `.opencode/commands/` manually.
- Committing secrets, `.specify/feature.json`, or temporary files.
- Rewriting git history or force-pushing without explicit instruction.
- Adding dependencies without justification recorded in `plan.md`/`research.md`.
- Expanding v1 scope (network, cloud, cross-platform) without updating the specification.

### Task definition of done

- [ ] Code implemented in the specified files.
- [ ] Tests for critical modules written and passing; UI verified manually per quickstart.
- [ ] `cargo fmt`, `cargo clippy -D warnings`, `cargo test` are green.
- [ ] Documentation updated in Russian and English (canonical + `ru/` + `en/`).
- [ ] Task marked `[x]` in `tasks.md`.
- [ ] Commit follows Conventional Commits.

### Commands

```powershell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

### Git

- Branches: `<###>-<feature-name>` (for example, `001-freespace-disk-analyzer`).
- Commits: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`.
- One commit per logical task or group.
