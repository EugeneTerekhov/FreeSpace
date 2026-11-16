# How to Use the FreeSpace Repository

## 1. Purpose of the repository

This repository hosts the development of **FreeSpace**, a native Windows utility that analyzes and
reclaims disk space. It currently contains only the SDD documentation (specification, plan, tasks);
the source code has not been created yet.

The source of truth is the canonical artifacts in
[`specs/001-freespace-disk-analyzer/`](../specs/001-freespace-disk-analyzer/). The directories
[`ru/`](../ru/) and [`en/`](.) contain Russian and English documentation mirrors.

## 2. Requirements

| Tool | Version | Purpose |
|------|---------|---------|
| Windows | 10 21H2+ or 11, x64 | Target platform |
| Git | 2.40+ | Version control |
| Rust | stable 1.85+, edition 2024 | Implementation (once development starts) |
| uv | 0.5+ | Spec Kit CLI installer (optional) |
| Python | 3.11+ | Spec Kit CLI requirement (installed via uv) |

Installing the Spec Kit CLI (needed only for SDD commands):

```powershell
winget install astral-sh.uv
uv python install 3.12
uv tool install specify-cli --python 3.12
```

Make sure `%USERPROFILE%\.local\bin` is on `PATH`, then verify:

```powershell
specify --version
```

## 3. Repository structure

```text
FreeSpace/
├── .specify/                          # Spec Kit scaffolding: templates, scripts, constitution
│   └── memory/constitution.md         # project constitution (mandatory)
├── .opencode/commands/                # /speckit.* commands for OpenCode
├── specs/001-freespace-disk-analyzer/ # canonical artifacts (Russian)
│   ├── spec.md                        # specification: 23 stories, 96 FRs, 18 SCs
│   ├── plan.md                        # plan: stack, architecture, Constitution Check
│   ├── research.md                    # 22 technical decisions and alternatives
│   ├── data-model.md                  # 20 entities and integrity rules
│   ├── quickstart.md                  # end-to-end validation scenarios
│   ├── contracts/                     # CLI, report formats, snapshot format
│   ├── checklists/                    # requirements quality checklists
│   └── tasks.md                       # 169 tasks across 27 phases
├── ru/                                # Russian documentation mirrors
├── en/                                # English documentation translations
├── AGENTS.md                          # rules for AI agents
└── README.md                          # short overview and links
```

## 4. Where to start

Recommended reading order:

1. [`en/constitution.md`](constitution.md) — principles that must not be violated.
2. [`en/spec.md`](spec.md) — what we build and why.
3. [`en/plan.md`](plan.md) — how we build it: stack and architecture.
4. [`en/research.md`](research.md) — why specific technical decisions were made.
5. [`en/data-model.md`](data-model.md) — entities and relationships.
6. [`en/contracts/`](contracts/) — external interfaces (CLI, reports, snapshots).
7. [`en/tasks.md`](tasks.md) — step-by-step work items.
8. [`en/quickstart.md`](quickstart.md) — how to validate the result.

## 5. How the SDD process works

The project uses [GitHub Spec Kit](https://github.com/github/spec-kit). Commands are invoked in the
OpenCode chat (not the terminal) and read the active feature from `.specify/feature.json`.

| Stage | Command | Output |
|-------|---------|--------|
| Constitution | `/speckit.constitution` | `.specify/memory/constitution.md` |
| Specification | `/speckit.specify` | `spec.md` and `checklists/requirements.md` |
| Clarification | `/speckit.clarify` | `Clarifications` section in `spec.md` |
| Plan | `/speckit.plan` | `plan.md`, `research.md`, `data-model.md`, `contracts/`, `quickstart.md` |
| Checklist | `/speckit.checklist` | `checklists/<domain>.md` |
| Tasks | `/speckit.tasks` | `tasks.md` |
| Analysis | `/speckit.analyze` | consistency report (no file changes) |
| Implementation | `/speckit.implement` | code per tasks |
| Convergence | `/speckit.converge` | appends missing tasks to `tasks.md` |

The current feature already exists (`001-freespace-disk-analyzer`). A new feature is created with
`/speckit.specify`; it receives the next number (`002-…`), and `.specify/feature.json` switches to it.

## 6. Bilingual policy

- Canonical Spec Kit artifacts are written **in Russian** in `specs/<feature>/`.
- `ru/` holds Russian mirrors; `en/` holds English translations.
- Every change to a canonical file must be mirrored to both directories:
  1. edit `specs/<feature>/<file>`;
  2. copy it to `ru/`;
  3. translate it to `en/`;
  4. verify that `en/` contains no Cyrillic and that IDs (FR-, SC-, T-, CHK-) are unchanged.

Quick check:

```powershell
Select-String -Path en\*.md,en\contracts\*.md,en\checklists\*.md -Pattern '[\u0400-\u04FF]'
```

## 7. Development (once code exists)

1. Pick the next unfinished task in `tasks.md`, respecting dependencies.
2. For critical modules, write a failing test first (Constitution Principle V).
3. Implement the task while staying within the files listed in its description.
4. Mark the task `[x]` only after tests and checks pass.
5. Update documentation if behavior changes.

Mandatory checks before committing:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Commits follow [Conventional Commits](https://www.conventionalcommits.org/):
`feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`.

`checklists/requirements.md` is maintained by the specify/clarify commands. `checklists/quality.md`
belongs to the reviewer: agents must not mark its items.

## 8. Useful commands

```powershell
specify --version                 # Spec Kit CLI version
specify check                     # environment and integration checks
git status                        # repository state
git log --oneline -5              # latest commits
cargo build --workspace           # build (after implementation starts)
cargo test --workspace            # tests
```

## 9. Troubleshooting

- **`uv` or `specify` not found.** Restart the terminal or add `%USERPROFILE%\.local\bin` to `PATH`.
- **Spec Kit requires Python 3.11+.** Use `uv python install 3.12` and
  `uv tool install specify-cli --python 3.12`.
- **A command cannot find the feature.** Check `.specify/feature.json`; it is not committed and is
  created by `/speckit.specify`. Alternatively, set `SPECIFY_FEATURE_DIRECTORY`.
- **Do not edit `.specify/templates/` or `.opencode/commands/` manually.** Use presets and project
  overrides as described in the Spec Kit documentation.
- **`cargo` commands fail.** This is expected: implementation has not started; the workspace will
  appear in Phase 1 of the tasks.

## 10. Links

- [README](../README.md)
- [Constitution](constitution.md)
- [Specification](spec.md)
- [Plan](plan.md)
- [Tasks](tasks.md)
- [Spec Kit documentation](https://github.github.io/spec-kit/)
