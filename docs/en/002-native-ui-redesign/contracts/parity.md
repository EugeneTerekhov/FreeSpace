# GUI and CLI parity matrix (002)

**Date**: 2026-11-18 | **Feature**: `002-native-ui-redesign` | **Tasks**: T100, T101

Every screen of the new GUI has a CLI equivalent with JSON output. There are **11 screens in 6
groups**; the absorbed screens (Scan, Treemap, Size details, Owners, Snapshots, Export, Elevation,
About) remain separate capability rows. The Treemap is removed from the product and has no parity
row.

Statuses:

- **implemented** — the screen/tab is implemented, covered by Rust contracts and Vitest tests, and
  the E2E/axe suites are updated for the new composition.
- **visual pending** — the visual-regression reference is rebuilt in a separate pass (T100, full
  matrix 11 × 2 themes × 4 DPI × 2 languages) and is not pinned yet.

| Screen | Section/tab | Capability | CLI command | Rust/unit | E2E/axe | Screenshot | Status |
|--------|-------------|------------|-------------|-----------|---------|------------|--------|
| `overview` | — | Volume list, used and free space, metric and snapshots | `drives` | contract + Vitest | updated | T100 | implemented |
| `overview` | Scan | Start, embedded progress, cancellation, access errors | `scan` | contract + Vitest | updated | T100 | implemented |
| `tree` | — | Hierarchy, node shares, search, filter, paging | `tree`, `filter` | contract + Vitest | updated | T100 | implemented |
| `tree` | Size details (hover card) | Logical, allocated and physical size, parent share, links, ADS, compression | `tree --json` | contract + Vitest | updated | T100 | implemented |
| `tree` | Row context menu | Open in Explorer and copy path (UI only), rename with a preview, delete to the Recycle Bin only with confirmation | `tree`, `rename`, `delete --mode recycle` | contract + Vitest | updated | T035 | implemented |
| `analysis` | Largest files | Largest files with filters and N | `top` | contract + Vitest | updated | T100 | implemented |
| `analysis` | Types | Distribution by extension and category | `types` | contract + Vitest | updated | T100 | implemented |
| `analysis` | Owners | Space by owner and their files, export | `owners` | contract + Vitest | updated | T100 | implemented |
| `duplicates` | — | Duplicate groups, keep rule, deletion preview | `duplicates` | contract + Vitest | updated | T100 | implemented |
| `cleanup` | Cleanup | Recommendations, risk level, empty directories | `cleanup` | contract + Vitest | updated | T100 | implemented |
| `cleanup` | Snapshots | Create snapshots and compare | `snapshot` | contract + Vitest | updated | T100 | implemented |
| `journal` | — | Operation journal, undo and restore through confirmation | `journal` | contract + Vitest | updated | T100 | implemented |
| `shred` | — | Irreversible overwrite with preview and confirmation | `shred` | contract + Vitest | updated | T100 | implemented |
| `monitor` | — | Write monitoring, thresholds and filters | `monitor` | contract + Vitest | updated | T100 | implemented |
| `inspect` | Metadata | EXIF, media and PE metadata (read-only) | `filter --metadata` | contract + Vitest | updated | T100 | implemented |
| `inspect` | Archives | Inspect archives without unpacking | `archives` | contract + Vitest | updated | T100 | implemented |
| `inspect` | Unlock | Locks, soft and hard release | `unlock` | contract + Vitest | updated | T100 | implemented |
| `inspect` | Long paths | Operations on verbatim paths | `tree`, `rename` | contract + Vitest | updated | T100 | implemented |
| `fileops` | Rename | Rename preview and apply | `rename` | contract + Vitest | updated | T100 | implemented |
| `fileops` | Permissions | Attributes, owner and ACL | `attrs`, `acl` | contract + Vitest | updated | T100 | implemented |
| `settings` | Settings | Settings, theme, language, paths, density | `config` | contract + Vitest | updated | T100 | implemented |
| `settings` | Elevation | Elevation status and request (through confirmation) | — (`app_elevation_status`) | Vitest | updated | T100 | implemented |
| `settings` | About | Version, WebView2, licences, `data` | — (`app_about_info`) | contract + Vitest | updated | T100 | implemented |
| Export (global) | — | CSV, XML, HTML and XLSX; action on data screens | `export` | contract + Vitest | updated | T100 | implemented |

## How it was verified

- Rust: `tests/contract/{core,analysis,operations,filesystem,inspection,settings}_commands.rs`;
  the `treemap_*` commands and their DTOs/fixtures are removed (T091), and the `tree` CLI contract is
  unchanged.
- CLI: `tests/cli_contract.rs` and the golden fixtures in `tests/contract/fixtures/`; commands return
  the same JSON envelopes as the Tauri commands.
- Frontend: `npm run test:unit` — Vitest/RTL for the 11 screens and their tabs; `npx tsc --noEmit`,
  `npm run lint`, `check:tokens`, `check:i18n`, `check:network` are green.
- E2E/axe: `tests/e2e/*.spec.ts` and `tests/a11y/*.spec.ts` were rewritten for 11 screens, 6 groups,
  tabs and the global export. The 2026-11-18 run is green: 55 E2E scenarios and 24 axe tests (five
  dark-theme contrast defects were found and fixed).
- Visual references: the set is rebuilt in the separate T100 pass; screenshots are not pinned in the
  matrix, so the status is `visual pending`, not `verified`.

## Remaining

1. Rebuild the visual references for 11 × 2 × 4 × 2 plus the PR set.
2. Manual Narrator pass over the ten scenarios (`quickstart.md`).

The Settings tabs "Elevation" and "About" have no separate CLI command by specification: elevation is
requested only from the GUI, and the CLI prints its version via `--version`. This is recorded in
`specs/002-native-ui-redesign/spec.md` (FR-104, FR-107) and is not a parity gap.
