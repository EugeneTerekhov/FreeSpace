# Feature Specification: Multi-volume snapshot

**Feature Branch**: `004-multi-volume-snapshot`

**Created**: 2026-11-20

**Status**: Draft

**Input**: User description: "If I scanned two volumes, why can they not both be displayed in the
tree? Do it as feature 004: one scan and one snapshot with several roots, both volumes visible at
the root of the tree at once. Affects the scan request, IPC, CLI and the tree."

## Overview

Today one scan accepts exactly one volume or directory, and the application keeps exactly one
snapshot. A second scan replaces the first, so a user who scanned `C:\` and `D:\` sees only the last
volume in the tree and has to rescan to return to the previous one.

Feature 004 lets the user select several volumes and run **one scan** that creates **one snapshot
with several roots**. The tree shows every volume as a separate top-level row; the tree, search and
all analysis screens work on the combined snapshot. A new analysis adds the selected volumes to the
current snapshot and refreshes the volumes scanned again; no application restart is required.

## Clarifications

### Session 2026-11-20

- Q: How many snapshots does the application keep? → A: One current snapshot, as before; a new scan
  merges the selected volumes into the already scanned ones. Several volumes live inside one
  snapshot.
- Q: How are volumes selected? → A: By clicking volume cards on the overview screen; several
  selected cards are analysed in one run. The scan form has no separate path input.
- Q: Are volumes scanned in parallel or sequentially? → A: Sequentially, one root at a time. This
  keeps disk load predictable and root order deterministic.
- Q: What happens when one selected volume is unavailable? → A: The other volumes are scanned; the
  unavailable one becomes a warning and the task finishes as partially successful.
- Q: What happens when every volume is unavailable? → A: The scan fails and the previous snapshot is
  kept.
- Q: May nested paths be selected (for example `C:\` and `C:\Users`)? → A: No; overlapping roots are
  rejected so the same files are not counted twice.
- Q: Are several snapshots and switching between past scans required? → A: No, that is a separate
  future capability; this feature keeps one current snapshot.
- Q: Are report export and snapshot files in scope? → A: Export and snapshot files keep describing
  the primary volume (the first root) and list every root; extending the formats is a separate
  future feature.
- Q: How many roots may one scan contain? → A: At most 32.

### Session 2026-11-20 (largest files table)

- Q: How is the Object column sorted? → A: By the full path (`SortKey::Path`), case-insensitively;
  in a multi-volume snapshot this groups files by volume. Sorting happens on the server, and the
  `path` key is passed to `top_files_page`.
- Q: How does a user open an object in the tree? → A: With the `ListTree` icon in the last column;
  the accessible name is preserved (`treemap.open_tree`) and the column shrinks to 44 px.
- Q: Which date format does the table use? → A: `DD.MM.YYYY` with dots. Dates with time in the
  other tables use `DD.MM.YYYY HH:MM` — every formatter uses dots instead of slashes.
- Q: How are truncated values shown? → A: A `HoverCard` with the full name, path, sizes and date;
  the native `title` tooltips on cells are removed.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — One scan of several volumes (Priority: P1)

The user opens the volume overview, ticks two volumes and starts the analysis. The application shows
overall progress, the current volume and the current path, and creates one snapshot in which both
volumes are available in the tree. When one volume is disconnected, the user sees a warning but the
result for the other volumes is kept.

**Why this priority**: this is the user's original problem — the second scanned volume disappears.
Without a combined scan the remaining scenarios have no value.

**Independent Test**: select two available volumes, start the analysis and verify that both volumes
are present at the tree root after one scan.

**Acceptance Scenarios**:

1. **Given** two available volumes, **When** the user ticks both and starts the analysis, **Then**
   one snapshot is created and both volumes are visible at the tree root.
2. **Given** two selected volumes, **When** the analysis runs, **Then** progress shows the overall
   percentage and the current volume, and cancellation stops the remaining volumes.
3. **Given** two selected volumes, one of them unavailable, **When** the analysis finishes, **Then**
   the available volume stays in the snapshot, the unavailable one is named in a warning, and the
   task state is partially successful.
4. **Given** every selected volume is unavailable, **When** the analysis finishes, **Then** an error
   is shown and the previous snapshot is unchanged.
5. **Given** a two-volume scan is running, **When** the user tries to start a second scan, **Then**
   the application does not start it and shows the running progress.

---

### User Story 2 — Every volume in one tree and one analysis (Priority: P1)

The user opens the tree and sees every scanned volume as a separate top-level row. They expand any
volume, follow breadcrumbs, search for a file name and see each element's share relative to its
volume and to the whole snapshot. The other analysis screens — largest files, types, owners,
duplicates, cleanup, inspection — work across all volumes at once.

**Why this priority**: without this, the combined snapshot gives the user no benefit.

**Independent Test**: open the tree on a two-volume snapshot, expand both roots, search for a file
name on the second volume and verify that the result is found.

**Acceptance Scenarios**:

1. **Given** a two-volume snapshot, **When** the user opens the tree, **Then** both volume rows are
   visible at the root with their sizes and item counts.
2. **Given** a two-volume snapshot, **When** the user expands the second volume, **Then** its
   directories are visible and each element's share is computed from that volume's size.
3. **Given** a two-volume snapshot, **When** the user searches for a file name, **Then** the search
   covers both volumes and returns matches regardless of their volume.
4. **Given** a two-volume snapshot, **When** the user opens largest files, types, owners, duplicates
   or cleanup, **Then** the results include items from both volumes.
5. **Given** a two-volume snapshot, **When** the user sees the context line, **Then** it names the
   number of volumes in the current snapshot.

---

### User Story 3 — CLI parity and explicit snapshot composition (Priority: P2)

An administrator scans several volumes from the command line and receives machine-readable output
with the volume and root lists. In the graphical interface they always understand which volumes are
in the current snapshot and can extend it by selecting one or several more volumes.

**Why this priority**: CLI and GUI parity is required by the constitution; without it the
multi-volume scenario is unsuitable for automation.

**Independent Test**: run a two-path scan from the command line with JSON output and verify that the
response contains both volumes and both roots; then repeat the scenario in the graphical interface.

**Acceptance Scenarios**:

1. **Given** two paths, **When** the user runs a scan from the command line, **Then** the JSON output
   contains a volume array, a root array and the total size.
2. **Given** a two-volume snapshot, **When** the user prints the tree from the command line, **Then**
   the output contains both roots and their hierarchy.
3. **Given** one path on the command line, **When** the scan finishes, **Then** the behaviour and
   output shape remain compatible with a single volume.
4. **Given** a current two-volume snapshot, **When** the user re-analyses one volume, **Then** the
   snapshot contains both volumes with the selected volume refreshed, and the interface clearly
   shows the snapshot composition.
5. **Given** overlapping paths on the command line, **When** the user starts the scan, **Then** the
   command fails validation without starting the analysis.

---

### Edge Cases

- The same volume is listed twice or written in two forms (`D:\` and `d:/`) — the request is rejected
  as containing a duplicate.
- One root is an ancestor of another (`C:\` and `C:\Users`) — the request is rejected so files are
  not counted twice.
- A volume is disconnected between selection and scan start — it becomes a warning and the other
  volumes are scanned.
- A volume is disconnected during the scan — the task finishes partially and the data already
  collected is kept.
- The user cancels between volumes — the remaining volumes are not scanned and the task is marked
  cancelled.
- One volume is empty — it is still shown at the tree root with a zero size.
- Volumes have different file systems and methods — each root reports its own method and the summary
  marks the set as mixed.
- A single-volume snapshot — behaviour, output shape and screens are unchanged.
- A previous snapshot exists and the new scan fails — the previous snapshot stays available.
- More than 32 roots are selected — the request is rejected with a clear error.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The user MUST be able to select one to 32 available volumes by clicking their cards and
  start one scan.
- **FR-002**: A scan request MUST accept one or several roots; an empty list and relative paths are
  rejected.
- **FR-003**: A scan request MUST reject duplicate and overlapping roots.
- **FR-004**: One scan MUST create one snapshot of one generation containing the previously scanned
  volumes and every successfully scanned root of the new run.
- **FR-005**: Every successfully scanned root MUST be shown as a separate row at the tree root with
  its name, path, size and item count.
- **FR-006**: The total size and item count of the snapshot MUST include every root.
- **FR-007**: The tree, search and every analysis screen MUST work across every root of the current
  snapshot without another scan.
- **FR-008**: Scan progress MUST show the overall percentage, the current root and the current path;
  progress MUST be monotonic.
- **FR-009**: Cancellation MUST stop the current and every remaining root; the task is marked
  cancelled.
- **FR-010**: When a root is unavailable or cannot be scanned, the other roots MUST still be scanned
  and the task MUST finish as partially successful with a warning naming the problem root and cause.
- **FR-011**: When no root could be scanned, the scan MUST fail and the previous snapshot MUST remain
  unchanged.
- **FR-012**: At most one scan MUST run at a time; reserved volumes MUST be protected from
  concurrent scans.
- **FR-013**: The interface MUST clearly show the composition of the current snapshot (which volumes
  it contains).
- **FR-014**: The command line MUST accept several paths for scanning and tree output and MUST emit
  machine-readable output with the volume list, root list and total size.
- **FR-015**: Behaviour, output shape and screens for a single volume MUST remain compatible with
  the current ones.
- **FR-016**: Delete and rename operations MUST journal the volume that owns the operation target,
  not the first volume of the snapshot.
- **FR-017**: Privacy MUST be preserved: a multi-volume scan makes no network calls and sends no
  telemetry.
- **FR-018**: The existing performance and memory budgets MUST hold for the combined snapshot: up to
  10 million nodes within 4 GB and interface responses within 200 ms.
- **FR-019**: Re-analysing a volume that is already in the snapshot MUST refresh its data instead of
  duplicating the root.

### Key Entities

- **Scan root**: a volume or directory selected by the user; it has a path, a volume, a scan method
  and a result (success or error).
- **Multi-volume snapshot**: one immutable scan result; it contains one or several roots, the volume
  list, a session per root, aggregate counters and a generation.
- **Scan task**: background work with progress, cancellation and a terminal state; it knows every
  reserved volume and every root.
- **Root session**: metadata of one root inside a task: volume, path, method, time, node count,
  errors and warnings.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user who selects two available volumes sees both volumes at the tree root after one
  scan, without starting the analysis again.
- **SC-002**: A single-volume scan keeps the current budget: 8 TB within 60 seconds on a typical
  consumer SSD; two volumes totalling 8 TB finish within 120 seconds.
- **SC-003**: A 10-million-node snapshot across all roots fits within 4 GB of memory.
- **SC-004**: Opening the tree root page on a multi-volume snapshot takes no more than 200 ms.
- **SC-005**: 100% of analysis screens open on a multi-volume snapshot and return data from at least
  one non-empty volume.
- **SC-006**: A file-name search finds a file on any volume of the snapshot in 100% of tested
  scenarios.
- **SC-007**: When one volume is unavailable, the user receives results for the other volumes and a
  warning naming the unavailable volume; no results are lost.
- **SC-008**: CLI output for two paths contains both volumes and both roots and remains valid JSON.

## Assumptions

- The user runs the application on Windows or Linux within the supported 2.0 platforms.
- Volumes are available through ordinary mount points; network and cloud paths are out of scope.
- One current snapshot is enough for the scenario; comparing several past snapshots is out of
  scope.
- Report export and snapshot files remain compatible: they record the primary (first) volume and
  list every root; extending the formats is a separate future feature.
- The scan runs sequentially per root; parallel volume scanning is out of scope.
- The user expects a re-analysis to refresh the selected volumes while the other volumes remain in
  the current snapshot.
