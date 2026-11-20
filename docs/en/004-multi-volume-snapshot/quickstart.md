# Quickstart: Multi-volume snapshot

**Feature**: `004-multi-volume-snapshot` | **Date**: 2026-11-20

## Prerequisites

- Windows 10 21H2+ or Linux x86_64; the application is built from the `004-multi-volume-snapshot`
  branch.
- Windows scenarios prefer two test volumes (VHD/VHDX) so working disks are not touched.
- Linux scenarios use two directories or two mount points on a temporary file system.
- Real user files are never modified; deletion is tested only on temporary data.

## 1. GUI: two volumes in one tree

1. Launch the application and open **Overview**.
2. Click the cards of two available volumes and press **Analyze**.
3. Wait for completion.

**Expected**:

- progress shows the overall percentage, the current volume and the path;
- after completion the tree has two top-level rows — both volumes;
- the tree context names two volumes;
- the tree total equals the sum of the volume sizes.

## 2. GUI: expansion, search and analysis

1. Expand the second volume in the tree.
2. Open a directory and check the breadcrumbs.
3. Search by name for a file that exists only on the second volume.
4. Open **Largest files**, **Types**, **Owners**, **Duplicates**, **Cleanup** and **Inspection**.

**Expected**:

- element shares are computed from the parent and from the whole snapshot;
- the search finds the file on the second volume;
- every screen opens without errors and contains items from both volumes where applicable.

## 3. GUI: partial success and cancellation

1. Select two volumes, one of them disconnected or unavailable.
2. Start the analysis.

**Expected**:

- the available volume appears in the tree;
- the warning names the unavailable volume and the cause;
- the task state is partially successful.

3. Repeat with two available volumes and press **Cancel** during the second volume.

**Expected**:

- the second volume is not scanned to the end;
- the task state is cancelled;
- the interface stays responsive.

## 4. GUI: adding volumes one by one

1. On a single-volume snapshot select the second volume and start the analysis.
2. Wait for completion.
3. Re-analyse the first volume.

**Expected**:

- after the second run the snapshot contains both volumes;
- the re-analysis refreshes the selected volume without duplicating its root;
- the interface clearly shows the snapshot composition; no application restart is required.

## 5. CLI: two paths

```powershell
freespace scan C:\ D:\ --json
freespace tree C:\ D:\ --depth 2 --json
```

**Expected**:

- `scan` returns `volumes` with two elements, `roots` with two paths and the total `total`;
- `tree` returns two elements in `roots`;
- the JSON is valid; the exit code is 0.

## 6. CLI: errors and limits

```powershell
freespace scan C:\ C:\ --json
freespace scan C:\ C:\Users --json
freespace scan C:\ D:\ E:\ F:\ ... (33 paths) --json
```

**Expected**:

- a duplicate, an overlap and the limit are rejected before scanning;
- the exit code is 2, the message is clear and no scan starts.

```powershell
freespace scan C:\ --json
```

**Expected**:

- behaviour and fields remain compatible with a single volume; `volumes` and `methods` are added.

## 7. Performance and memory

- One 8 TB volume: 60 s or less on a consumer SSD (Windows, MFT).
- Two volumes totalling 8 TB: 120 s or less.
- 10 million nodes across all roots: 4 GB or less.
- Tree root page: 200 ms or less.

Verification: `cargo test --workspace`, then a manual run on test VHDs; for memory use
`cargo run --release -p freespace-cli -- scan <path> --json` under a profiler.

## 8. Regressions

- Single volume: tree, search, analysis, operations, export and snapshot files work as before.
- `volumes_current` returns the first volume; `snapshot_volumes` returns all.
- The journal entry for an operation on the second volume contains the second volume id.
- Reports and `.fssnap` record the primary volume and list every root (feature limitation).
