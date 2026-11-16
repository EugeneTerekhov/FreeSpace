# GUI and CLI parity matrix (002)

**Date**: 2026-11-16 | **Feature**: `002-native-ui-redesign` | **Tasks**: T177, T181

Every screen of the new GUI has a CLI equivalent with JSON output. The screenshot column names one
cell of the full visual matrix in `apps/freespace-desktop/ui/tests/visual/baselines/`
(`<screen>-dark-100-ru.png`). The `verified` status means the screen is implemented, covered by a
Rust or Vitest test, an E2E scenario and a baseline, and its CLI behaviour was compared on the same
data set.

| Screen | Capability | CLI command | Tests | Screenshot | Status |
|--------|------------|-------------|-------|------------|--------|
| `dashboard` | Volume list, used and free space | `drives` | Rust + E2E + visual | `dashboard-dark-100-ru` | verified |
| `scan` | Start, progress and cancel a scan | `scan` | Rust + E2E + visual | `scan-dark-100-ru` | verified |
| `tree` | Hierarchy, search, filter and exact sizes | `tree`, `filter` | Rust + E2E + visual | `tree-dark-100-ru` | verified |
| `treemap` | Map, shares and level drill-down | `tree` | Rust + E2E + visual | `treemap-dark-100-ru` | verified |
| `top_files` | Largest files with filters | `top` | Rust + E2E + visual | `top-files-dark-100-ru` | verified |
| `types` | Distribution by extension and category | `types` | Rust + E2E + visual | `types-dark-100-ru` | verified |
| `duplicates` | Duplicate groups and copy selection | `duplicates` | Rust + E2E + visual | `duplicates-dark-100-ru` | verified |
| `cleanup` | Cleanup recommendations and empty directories | `cleanup` | Rust + E2E + visual | `cleanup-dark-100-ru` | verified |
| `snapshots` | Creating and comparing snapshots | `snapshot` | Rust + E2E + visual | `snapshots-dark-100-ru` | verified |
| `journal` | Operation journal, undo and restore | `journal` | Rust + E2E + visual | `journal-dark-100-ru` | verified |
| `size_details` | Logical, allocated and physical size | `tree --json` | Rust + E2E + visual | `size-details-dark-100-ru` | verified |
| `long_paths` | Operations on long paths | `tree`, `rename` | Rust + E2E + visual | `long-paths-dark-100-ru` | verified |
| `rename` | Rename preview and apply | `rename` | Rust + E2E + visual | `rename-dark-100-ru` | verified |
| `permissions` | Attributes, owner and ACL | `attrs`, `acl` | Rust + E2E + visual | `permissions-dark-100-ru` | verified |
| `metadata` | EXIF, media and PE metadata | `filter` | Rust + E2E + visual | `metadata-dark-100-ru` | verified |
| `owners` | Space by owner and owner files | `owners` | Rust + E2E + visual | `owners-dark-100-ru` | verified |
| `archives` | Archive inspection without unpacking | `archives` | Rust + E2E + visual | `archives-dark-100-ru` | verified |
| `export` | CSV, XML, HTML and XLSX export | `export` | Rust + E2E + visual | `export-dark-100-ru` | verified |
| `unlock` | Locks, soft and hard release | `unlock` | Rust + E2E + visual | `unlock-dark-100-ru` | verified |
| `shred` | Irreversible overwrite with a preview | `shred` | Rust + E2E + visual | `shred-dark-100-ru` | verified |
| `monitor` | Write monitoring, thresholds and exclusions | `monitor` | Rust + E2E + visual | `monitor-dark-100-ru` | verified |
| `elevation` | Elevation status and request | — (`app_elevation_status`) | Vitest + E2E + visual | `elevation-dark-100-ru` | verified |
| `settings` | Settings, theme, language and paths | `config` | Rust + E2E + visual | `settings-dark-100-ru` | verified |
| `about` | Version, WebView2, licences and `data` | — (`app_about_info`) | Rust + E2E + visual | `about-dark-100-ru` | verified |

## How it was checked

- Rust: `tests/contract/{core,analysis,operations,filesystem,inspection,settings}_commands.rs`.
- CLI: `tests/cli_contract.rs` and the golden fixtures in `tests/contract/fixtures/`; commands return
  the same JSON envelopes as the Tauri commands.
- Frontend: Vitest for the 24 screens, Playwright E2E on the mock transport, an axe audit, the full
  visual matrix `24 × 2 × 4 × 2` and the pull-request subset.
- Screenshots: `apps/freespace-desktop/ui/tests/visual/baselines/`, gallery in
  `apps/freespace-desktop/ui/tests/visual/gallery/index.html`.

`elevation` and `about` intentionally have no dedicated CLI command: elevation starts only from the
GUI and the CLI reports its version through `--version`. This is recorded in
`specs/002-native-ui-redesign/spec.md` (FR-104, FR-112) and is not a parity gap.
