# Research: Multi-volume snapshot

**Feature**: `004-multi-volume-snapshot` | **Date**: 2026-11-20

## 1. Combining several volumes into one snapshot

- **Decision**: scan roots sequentially into one `FsTree` through the new
  `freespace_scan::scan_many` and internal `scan_into`.
- **Rationale**: `walk::scan` and `mft_scan::scan` already accept `&mut FsTree` and append a root
  through `add_node(None, …)`. Calling them again on the same arena creates a second root without a
  merge, copy or doubled memory. `FsTree::roots()` and `TreeSnapshot::root_page` already support
  several roots.
- **Alternatives considered**:
  - *Scan each volume separately and merge `FsTree`*. Rejected: requires a merge with `NodeId`
    remapping and peak memory of "result + last volume".
  - *Keep several snapshots and switch between them*. Rejected by the user: a different feature that
    increases memory and changes the generation model.
  - *Parallel volume scans*. Rejected: disk contention, non-monotonic progress, complex cancellation
    and non-determinism.

## 2. Snapshot model

- **Decision**: `TreeSnapshot` stores `volumes: Vec<Volume>` and `sessions: Vec<ScanSession>`;
  `volume()`/`session()` return the first elements.
- **Rationale**: dozens of existing callers (`export`, `snapshots`, `owners`, `operations`) expect one
  volume. Keeping the accessors preserves compatibility with a minimal diff; new callers use
  `volumes()`/`sessions()`.
- **Alternatives considered**:
  - *Replace `volume` with `volumes` and update every caller*. Rejected: a large diff with no
    benefit; export and snapshot files still need a primary volume.
  - *Virtual "This PC" root*. Rejected: breaks node paths and `tree_path` semantics.

## 3. Request format

- **Decision**: `ScanRequest.paths: Vec<String>`; the IPC version is bumped.
- **Rationale**: one source of truth instead of `path` plus `paths`. The protocol is internal; the
  version bump makes the incompatibility explicit.
- **Alternatives considered**:
  - *Keep `path` and add `paths`*. Rejected: two fields with unclear precedence and duplicated
    validation.
  - *A delimited string*. Rejected: breaks on paths containing separators and is worse for JSON.

## 4. Root validation

- **Decision**: 1 to 32 paths; reject duplicates by the platform key; reject overlap after
  canonicalisation.
- **Rationale**: a duplicate or nested root causes double counting and tree duplicates.
  Canonicalisation makes `D:\` and `d:/` the same root and prevents junction paths from bypassing the
  check.
- **Alternatives considered**:
  - *Allow overlaps and deduplicate nodes*. Rejected: complex, expensive and changes size semantics.
  - *Compare strings only*. Rejected: misses `D:\` vs `d:/` and symbolic links.

## 5. Progress

- **Decision**: overall percentage `(index + local_fraction) / roots_count`; the current root and path
  are passed through; a final `Finished` event reports 1.0.
- **Rationale**: does not require knowing volume sizes in advance, is monotonic and cheap.
- **Alternatives considered**:
  - *Weight by `total_bytes`*. Rejected: requires a preliminary `describe_volume` for every root and
    is still inaccurate during MFT phases.
  - *Show only the current volume's progress*. Rejected: the overall progress is unclear.

## 6. Cancellation and errors

- **Decision**: a root error does not stop the others; cancellation stops the current and remaining
  roots; when no root succeeds the first error is returned.
- **Rationale**: matches user story 1 and the current single-scan semantics (a snapshot is installed
  on success).
- **Alternatives considered**:
  - *Stop at the first error*. Rejected: one disconnected volume would deny results from the others.
  - *Ignore root errors*. Rejected: hides data loss.

## 7. Reservation and concurrency

- **Decision**: reserve every task volume atomically; if any volume is busy return `E_BUSY`.
- **Rationale**: avoids partial reservations and races; the GUI runs one scan anyway.
- **Alternatives considered**:
  - *Reserve volumes one by one*. Rejected: needs rollback and leaves a race window.

## 8. CLI

- **Decision**: `scan` and `tree` accept several positional paths; JSON gets `volumes`, `roots` and
  `methods`.
- **Rationale**: GUI parity and the natural `freespace scan C:\ D:\` syntax.
- **Alternatives considered**:
  - *Repeatable `--path`*. Rejected: breaks the existing positional call and reads worse.
  - *GUI only*. Rejected: violates constitution principle VI.

## 9. Reports and snapshot files

- **Decision**: do not change the formats in 004; record the primary (first) volume and every root.
- **Rationale**: bounds the feature and keeps existing files working; the user gets the main value, a
  multi-volume tree. Extending the format requires a separate specification and migration.
- **Alternatives considered**:
  - *Add `volumes` to `.fssnap` and reports now*. Rejected: changes the format, snapshot comparison
    and every report writer; bloats the feature.
