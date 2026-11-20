# How the FreeSpace repository works

## 1. Purpose

This repository develops **FreeSpace**, a native Windows utility that analyzes and reclaims disk
space. It contains the SDD documentation, Rust crates and the Tauri 2 + React client with its
tests.

The source of truth is the canonical material in [`../../specs/`](../../specs/). General Russian
documents live in [`../ru/`](../ru/); English translations live in [`docs/en/`](.).

## 2. Requirements

| Tool | Version | Purpose |
|------|---------|---------|
| Windows | 10 21H2+ or 11, x64 | Target platform |
| Git | 2.40+ | Version control |
| Rust | stable 1.85+, edition 2024 | Core code |
| Node.js | 20.19+ | Tauri 2 + React interface |
| uv | 0.5+ | Spec Kit CLI installation (optional) |
| Python | 3.11+ | Spec Kit CLI requirement (installed through uv) |

The Spec Kit CLI is needed only for the SDD commands:

```powershell
winget install astral-sh.uv
uv python install 3.12
uv tool install specify-cli --python 3.12
```

Make sure `%USERPROFILE%\.local\bin` is on `PATH`, then check:

```powershell
specify --version
```

## 3. Repository structure

```text
FreeSpace/
├── .specify/                          # Spec Kit scaffolding: templates, scripts, constitution
│   └── memory/constitution.md         # project constitution (binding)
├── .opencode/commands/                # /speckit.* commands for OpenCode
├── specs/001-freespace-disk-analyzer/ # canonical artifacts (Russian)
│   ├── spec.md                        # specification
│   ├── plan.md                        # plan: stack, architecture, Constitution Check
│   ├── research.md                    # technical decisions and alternatives
│   ├── data-model.md                  # entities and integrity rules
│   ├── quickstart.md                  # end-to-end validation scenarios
│   ├── contracts/                     # CLI, report formats, snapshot format
│   ├── checklists/                    # requirements quality checklists
│   └── tasks.md                       # phased task list
├── specs/002-native-ui-redesign/      # native interface specification
├── specs/003-cross-platform-portable/ # cross-platform portable release
├── specs/004-multi-volume-snapshot/   # multi-volume snapshot
├── docs/ru/                           # general Russian documents
├── docs/en/                           # English translations
├── apps/freespace-desktop/            # Tauri 2 + React client
├── crates/                            # Rust crates
├── AGENTS.md                          # AI agent rules
└── README.md                          # short overview and links
```

## 4. Where to start

1. [`constitution.md`](../../.specify/memory/constitution.md) — principles that must not be violated.
2. [`spec.md`](../../specs/001-freespace-disk-analyzer/spec.md) — what we build and why.
3. [`plan.md`](../../specs/001-freespace-disk-analyzer/plan.md) — how we build it: stack and architecture.
4. [`research.md`](../../specs/001-freespace-disk-analyzer/research.md) — why specific decisions were chosen.
5. [`data-model.md`](../../specs/001-freespace-disk-analyzer/data-model.md) — entities and relations.
6. [`contracts/`](../../specs/001-freespace-disk-analyzer/contracts/) — external interfaces.
7. [`tasks.md`](../../specs/001-freespace-disk-analyzer/tasks.md) — task history.
8. [`quickstart.md`](../../specs/001-freespace-disk-analyzer/quickstart.md) — how to validate results.

## 5. How the SDD process works

The project uses [GitHub Spec Kit](https://github.com/github/spec-kit). Commands run in the
OpenCode chat (not in the terminal) and read the active feature from `.specify/feature.json`.

| Stage | Command | Result |
|-------|---------|--------|
| Constitution | `/speckit.constitution` | `.specify/memory/constitution.md` |
| Specification | `/speckit.specify` | `spec.md` and `checklists/requirements.md` |
| Clarifications | `/speckit.clarify` | `Clarifications` section in `spec.md` |
| Plan | `/speckit.plan` | `plan.md`, `research.md`, `data-model.md`, `contracts/`, `quickstart.md` |
| Checklist | `/speckit.checklist` | `checklists/<domain>.md` |
| Tasks | `/speckit.tasks` | `tasks.md` |
| Analysis | `/speckit.analyze` | consistency report (no file changes) |
| Implementation | `/speckit.implement` | code for the tasks |
| Convergence | `/speckit.converge` | adds missing tasks to `tasks.md` |

## 6. Bilingual policy

- Canonical Spec Kit artifacts are written **in Russian** in `specs/<feature>/`.
- General Russian documents live in `docs/ru/`.
- English translations live in `docs/en/`; no separate `ru/` mirror is created.
- Every canonical change must be translated into `docs/en/`:
  1. edit `specs/<feature>/<file>` or `docs/ru/<file>`;
  2. translate it into `docs/en/`;
  3. make sure `docs/en/` contains no Cyrillic and that IDs (FR-, SC-, T-, CHK-) are unchanged.

Quick check:

```powershell
Select-String -Path docs\en\*.md,docs\en\contracts\*.md,docs\en\checklists\*.md -Pattern '[\u0400-\u04FF]'
```

## 7. Development

1. Pick the next unfinished task in `tasks.md`, respecting dependencies.
2. For critical modules write a failing test first (constitution principle V).
3. Implement the task while keeping files inside the stated boundaries.
4. Mark a task `[x]` only after the tests and checks pass.
5. Update the documentation when behavior changes.

Required checks before a commit:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Commits follow [Conventional Commits](https://www.conventionalcommits.org/):
`feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`.

`checklists/requirements.md` is maintained by the specify/clarify commands.

## 8. Useful commands

```powershell
specify --version                 # Spec Kit CLI version
specify check                     # environment and integration check
git status                        # repository state
git log --oneline -5              # recent commits
cargo build --workspace           # build
cargo test --workspace            # tests
```

Interface:

```powershell
cd apps/freespace-desktop/ui
npm ci
npm run tauri dev
npm run typecheck
npm run lint
npm run test:unit
```

## 9. Troubleshooting

- **`uv` or `specify` not found.** Restart the terminal or add
  `%USERPROFILE%\.local\bin` to `PATH`.
- **Spec Kit requires Python 3.11+.** Use `uv python install 3.12` and
  `uv tool install specify-cli --python 3.12`.
- **A command cannot find the feature.** Check `.specify/feature.json`; it is not committed and is
  created by `/speckit.specify`. Alternatively use the `SPECIFY_FEATURE_DIRECTORY` variable.
- **Do not edit `.specify/templates/` or `.opencode/commands/` by hand.** Use presets and project
  overrides as described in the Spec Kit documentation.

## 10. Links

- [README](../../README.md)
- [Constitution](../../.specify/memory/constitution.md)
- [Specification](../../specs/001-freespace-disk-analyzer/spec.md)
- [Plan](../../specs/001-freespace-disk-analyzer/plan.md)
- [Tasks](../../specs/001-freespace-disk-analyzer/tasks.md)
- [Russian documentation](../ru/)
- [Spec Kit documentation](https://github.github.io/spec-kit/)
