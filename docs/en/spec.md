# Feature Specification: FreeSpace — Windows Disk Space Analyzer and Manager

**Feature Branch**: `001-freespace-disk-analyzer`

**Created**: 2026-11-15

**Status**: Implemented

> **2.0 note (2026-11-20)**: this document records the historical 1.x decisions; requirement IDs do
> not change. FreeSpace 2.0 removes the quarantine, supports only system-trash and permanent
> deletion, ships no installers, reduces the GUI to 9 screens, and keeps the shredder in the Windows
> CLI only. Stories 5, 6 and 9, FR-043, FR-044, FR-093 and FR-096 must be read together with the
> [003 specification](../../specs/003-cross-platform-portable/spec.md); the current platforms and
> deletion modes are described there.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Fast volume scan and size tree (Priority: P1)

The user selects a disk or directory and starts the analysis. Within a few seconds they see a hierarchical tree where each node shows size, file count, and folder count; the tree can be expanded, sorted, filtered, and searched. For NTFS volumes, direct reading of the master file table (MFT) is used; for other file systems, directory tree traversal is used, with the applied method explicitly indicated. Re-analysis of a volume supports incremental updating via the USN change journal. Objects that cannot be accessed are marked and do not distort the totals.

**Why this priority**: this is the core of the product — all other features operate on the data collected by the scanner. Without it, the application has no value.

**Independent Test**: scan a test volume with a known structure and size checksums, verify that the tree is built, the totals match the reference, inaccessible items are marked, and search and sorting work.

**Acceptance Scenarios**:

1. **Given** the user has selected an NTFS volume, **When** they start the analysis, **Then** results appear progressively, the tree is available for navigation, and a full analysis of a volume up to 8 TB completes in no more than 60 seconds on a typical consumer SSD.
2. **Given** the analysis is complete, **When** the user sorts child nodes by size, **Then** the largest objects are at the top and the totals for nodes are consistent with the sizes of their descendants.
3. **Given** the volume contains directories inaccessible to the current user, **When** the analysis is complete, **Then** these directories are displayed marked "no access" and do not distort the totals.
4. **Given** a volume with a file system other than NTFS is selected, **When** the analysis starts, **Then** tree traversal is used, and the scanning method is explicitly shown in the interface and the report.
5. **Given** the analysis is complete, **When** the user enters a name, mask, or extension in the search, **Then** the tree is filtered, showing matches together with their parent nodes.
6. **Given** the volume has already been scanned and the change journal is available, **When** the user starts a re-analysis, **Then** only changed objects are updated, and the result is marked as incremental.
7. **Given** the analysis is running, **When** the user scrolls the tree or changes the sorting, **Then** the interface remains responsive and shows progress.

---

### User Story 2 — Interactive Treemap (Priority: P1)

The user opens a visual map where each file or folder is represented by a rectangle whose area is proportional to the space it occupies. The map supports navigation with drill-down into directories, tooltips with full information about an object, and coloring by file type, which makes it possible to instantly find the heaviest objects.

**Why this priority**: the visual map is the key way to instantly detect "space hogs" and the main differentiator of the product.

**Independent Test**: open the map for a set with known sizes, verify the proportionality of areas, and that zoom, tooltips, and the color legend work.

**Acceptance Scenarios**:

1. **Given** the analysis is complete, **When** the user opens the map, **Then** the area of each rectangle is proportional to the size of the object with sufficient accuracy for visual comparison.
2. **Given** the map is displayed, **When** the user selects a rectangle and drills into it, **Then** the map is rebuilt for the selected directory, and backward navigation is available via "breadcrumbs".
3. **Given** the map is displayed, **When** the user hovers the cursor over a rectangle, **Then** the name, full path, size, and share of the parent node are shown.
4. **Given** coloring by file type is enabled, **When** the map is rendered, **Then** files of the same type have the same color, and the legend explains the color mapping.
5. **Given** the volume contains millions of files, **When** the user interacts with the map, **Then** rendering remains responsive thanks to levels of detail.

---

### User Story 3 — Unlocking busy files (Priority: P1)

The user finds out which process is holding a file and can release the lock by closing the handle or terminating the process after confirmation. If the lock is at the kernel or driver level, the user can place the file in the system delete-on-reboot queue.

**Why this priority**: releasing locks is the second basic feature of the product, without which deletion and moving of busy files is impossible.

**Independent Test**: create a file held by a test process, find the blocking process, release the lock, and verify that the file has become accessible; check placement in the delete-on-reboot queue.

**Acceptance Scenarios**:

1. **Given** a file is held by a process, **When** the user selects "Who is locking it?", **Then** a list of processes and handles with the access type is displayed.
2. **Given** the list of locks is obtained, **When** the user confirms releasing the lock, **Then** the handle is closed, and file accessibility is verified by a repeated operation.
3. **Given** the file is locked at the kernel or driver level, **When** the user selects delete on reboot, **Then** the operation is registered and performed before the Windows graphical shell loads.
4. **Given** the unlock operation has been performed, **When** the user opens the journal, **Then** they see the process, time, operation type, and result.

---

### User Story 4 — Top largest files and grouping by type (Priority: P1)

The user opens a flat list of the top 100 or top 1000 heaviest files without folder hierarchy, as well as a separate report by extensions and categories that shows how much space is occupied by videos, archives, disk images, logs, and other data types.

**Why this priority**: these are the fastest answers to the question "what is taking up space", which users look for first.

**Independent Test**: on a set with known files, verify the correctness of the top list for different N, the sums and counts by extension, and navigation to an object in the tree.

**Acceptance Scenarios**:

1. **Given** the analysis is complete, **When** the user opens "Top files", **Then** the N largest files with paths and sizes are displayed, and N is configurable (at least 100 and 1000).
2. **Given** the "Types" tab is open, **When** the user selects an extension or category, **Then** the total size, file count, and share of occupied space are visible.
3. **Given** the user double-clicks a list entry, **When** the tree opens, **Then** the corresponding node is selected and highlighted.
4. **Given** the top list contains a file that cannot be accessed, **When** the list is generated, **Then** the file is present with a mark and does not distort the sums.

---

### User Story 5 — Duplicate search and safe deletion of copies (Priority: P2)

The user searches for duplicates by name, size, or checksum (MD5/SHA-256), gets groups of exact matches with an indication of the potentially reclaimable space, and applies rules for automatically selecting the copy to keep. Deletion of copies is performed only after preview and confirmation, by default to the Recycle Bin or quarantine.

**Why this priority**: duplicates are one of the most common causes of lost free space; the feature provides immediate practical benefit.

**Independent Test**: prepare a set with known duplicates (including files of the same size with different contents), verify the absence of false matches, the operation of selection rules, and the preservation of one copy.

**Acceptance Scenarios**:

1. **Given** the user has selected search areas, **When** a hash-based search starts, **Then** groups of exact byte-for-byte matches are formed without false positives.
2. **Given** a duplicate group is found, **When** the user applies the rule "keep newest/oldest/in the original directory", **Then** the remaining copies are marked for deletion, but deletion does not occur until confirmation.
3. **Given** the user confirms deletion, **When** the operation completes, **Then** the copies are moved to the selected safe location (Recycle Bin or quarantine) or permanently deleted if this method is explicitly selected, and at least one copy is guaranteed to be preserved.
4. **Given** the files changed during the search, **When** a discrepancy is detected, **Then** the group is marked as outdated and is not offered for deletion.
5. **Given** the user cancelled the operation, **When** they open the journal, **Then** they can restore objects from quarantine.

---

### User Story 6 — Shredder and delete on reboot (Priority: P2)

The user permanently destroys selected data by overwriting it with zeros or random data with a configurable number of passes. The application warns about irreversibility and about warranty limitations on SSDs. For files locked at the kernel level, placement in the system delete-on-reboot queue is available.

**Why this priority**: the feature is in demand when transferring a computer, disposing of disks, and deleting confidential data.

**Independent Test**: destroy a test set of files and try to recover them with an independent tool; check the journal, warnings, and the operation of delete on reboot.

**Acceptance Scenarios**:

1. **Given** the user has selected files for destruction, **When** they confirm the operation, **Then** overwriting with the selected method and deletion are performed, and recovery of the files with an independent tool is impossible.
2. **Given** an SSD volume is selected, **When** the shredder is opened, **Then** a warning about warranty limitations on SSDs is shown, explicit confirmation is required, and TRIM is initiated after overwriting.
3. **Given** the operation is complete, **When** the journal is open, **Then** the objects, method, number of passes, and result are recorded.
4. **Given** the file is locked at the kernel level, **When** delete on reboot is selected, **Then** the file is deleted before the graphical shell loads, and the result is visible after reboot.

---

### User Story 7 — Stale files, temporary data, empty folders, and recommendations (Priority: P2)

The user filters data by the date of last opening or modification, finds forgotten backups, application caches, logs, and temporary files, as well as empty directories left after uninstalling programs. A separate recommendations screen offers safe cleanup categories with an estimate of the space to be freed and the risk level.

**Why this priority**: these are ready-to-use cleanup scenarios that the user can apply immediately without understanding the disk structure.

**Independent Test**: on a prepared set with known dates and empty directories, verify the accuracy of the filters, the completeness of the empty folder list, and the correctness of the recommendations.

**Acceptance Scenarios**:

1. **Given** the threshold "not opened for more than 180 days" is set, **When** the filter is applied, **Then** the list contains only matching objects, calculated from the maximum of the access and modification dates, with the dates shown and the unreliability of the access date marked.
2. **Given** the analysis is complete, **When** the "Empty folders" report is open, **Then** all empty directories are listed, including nested ones, and their batch deletion is available after confirmation.
3. **Given** cleanup candidates are found, **When** the recommendations screen is open, **Then** categories with the potential size and an explanation of risks are shown.
4. **Given** the user selects a recommendation, **When** they confirm the action, **Then** the operation is performed according to the general safety rules via the Recycle Bin or quarantine.

---

### User Story 8 — Disk state snapshots and comparison (Priority: P2)

The user saves the disk state to a snapshot file, and after some time compares it with the current state and sees which folders have grown, what has appeared, what has been deleted, and what has changed.

**Why this priority**: comparison over time turns a one-off analysis into a tool for controlling data growth and finding space leaks.

**Independent Test**: create two snapshots with known differences, perform a comparison, and verify the completeness and sorting of the report.

**Acceptance Scenarios**:

1. **Given** the analysis is complete, **When** the user saves a snapshot, **Then** a versioned file is created with metadata of the volume, date, and scanning method.
2. **Given** there are two compatible snapshots, **When** a comparison is started, **Then** the report shows growth and reduction by directories and files.
3. **Given** the comparison is complete, **When** the user sorts by growth, **Then** the largest regressions are at the top.
4. **Given** snapshots of different volumes or incompatible format versions are compared, **When** the comparison starts, **Then** a clear error is displayed without data loss.

---

### User Story 9 — Safe operations: Recycle Bin, quarantine, journal, and undo (Priority: P2)

Any destructive operation goes through a unified preview and confirmation mechanism indicating the objects, the total size, and irreversibility. All operations are recorded in the journal; deleted objects go by default to the Recycle Bin or quarantine, from where they can be restored; where possible, undo is available.

**Why this priority**: this is the safety layer for all cleanup features; without it, the application is dangerous to use on real data.

**Independent Test**: perform a deletion, verify that preview and confirmation are present, restore an object from quarantine, and check the journal entry and the operation of undo.

**Acceptance Scenarios**:

1. **Given** the user initiates a destructive operation, **When** the confirmation opens, **Then** the list of objects, the total size, the irreversibility warning, and the choice of deletion method are shown: Windows Recycle Bin, application quarantine, or permanent deletion.
2. **Given** the operation is complete, **When** the user opens the journal, **Then** they see the parameters, the result, and the availability of undo, if applicable.
3. **Given** objects are in quarantine, **When** the user selects "Restore", **Then** the objects are returned to their original paths, and name conflicts are resolved predictably.
4. **Given** the operation affects a system or protected path, **When** the user requests it, **Then** privilege elevation and additional confirmation are required.

---

### User Story 10 — Accurate space accounting: links, compression, sparse files, and ADS (Priority: P3)

The user sees the real space usage: hard links are counted once per physical file, junction points and symbolic links are not traversed without explicit instruction, logical and physical sizes are shown for compressed and sparse files, and NTFS alternate data streams (ADS) are listed and accounted for.

**Why this priority**: without this, reports overstate occupied space and mislead when working with virtual machine images, compressed directories, and system files.

**Independent Test**: prepare a set with hard links, junctions, compressed and sparse files, and ADS, and verify that logical and physical sizes match the reference.

**Acceptance Scenarios**:

1. **Given** a directory contains several hard links to one file, **When** the analysis is complete, **Then** the physical size is counted once, the logical size is counted by the number of links, and both values are shown.
2. **Given** a directory contains a junction point, **When** the analysis is performed without instruction to follow links, **Then** the target's contents are not counted again, and the link is marked.
3. **Given** the volume contains compressed and sparse files, **When** the report is open, **Then** the logical size, size on disk, and amount of savings are shown.
4. **Given** a file has alternate data streams, **When** the properties are open, **Then** the streams are listed with their sizes and accounted for in the report.

---

### User Story 11 — File manager for long paths (Priority: P3)

The user renames, moves, copies, and deletes files whose path exceeds the standard Windows limit of 260 characters without enabling the system long paths policy.

**Why this priority**: long paths are a common reason why files cannot be deleted with ordinary tools; this is a unique practical capability.

**Independent Test**: create paths longer than 260 and longer than 1000 characters and perform the full set of operations, checking the journal and handling of limit values.

**Acceptance Scenarios**:

1. **Given** the path exceeds 260 characters, **When** the user renames, moves, or deletes an object, **Then** the operation completes successfully.
2. **Given** the path is approaching the system limit, **When** the operation would cause it to be exceeded, **Then** a clear error is displayed before any data is changed.
3. **Given** an operation with a long path is complete, **When** the journal is open, **Then** the entry contains the full path and the result.

---

### User Story 12 — Rule-based bulk rename (Priority: P3)

The user changes names, extensions, and case of thousands of files in a batch using regular expressions, patterns, or metadata, sees a preview of the result, and can undo the operation.

**Why this priority**: bulk file operations are a typical task when organizing archives and media libraries.

**Independent Test**: apply a rule to a test set of 1000 files, and check the preview, conflicts, result, and rollback.

**Acceptance Scenarios**:

1. **Given** multiple files are selected, **When** the user defines a rule with a regular expression, **Then** the preview shows old and new names, as well as conflicts.
2. **Given** there are no conflicts, **When** the user applies the rule, **Then** all names are changed, and the journal allows rollback.
3. **Given** the rule produces duplicate names, **When** the user attempts to apply it, **Then** the operation is blocked until conflicts are resolved.
4. **Given** the rename is complete, **When** the user clicks "Undo", **Then** the original names are restored.

---

### User Story 13 — Bulk change of NTFS attributes and permissions (Priority: P3)

The user batch-clears or sets file attributes, changes the owner, resets or copies NTFS permissions, sees a preview of the impact, and receives a report on successful and failed objects.

**Why this priority**: permission and attribute administration tasks arise en masse during data migration and access recovery.

**Independent Test**: on a test set, perform a change of attributes, owner, and permissions, and verify the result and the error report.

**Acceptance Scenarios**:

1. **Given** objects are selected, **When** the user batch-clears the "Read-only" attribute, **Then** the attribute is cleared on all accessible objects, and errors are listed separately.
2. **Given** an owner change is required, **When** the application lacks sufficient permissions, **Then** it requests privilege elevation and performs the operation after confirmation.
3. **Given** access permissions are being reset, **When** the user confirms, **Then** the selected or inherited permissions are assigned, and the journal records the changes.
4. **Given** some objects are inaccessible, **When** the operation is complete, **Then** the report shows successful and failed objects with reasons.

---

### User Story 14 — Metadata and advanced filtering (Priority: P3)

The user views and filters files by internal metadata: video resolution and duration, audio bitrate, camera model and shooting parameters from EXIF, executable file versions.

**Why this priority**: metadata makes it possible to find heavy media files and outdated program versions where size and name give no answer.

**Independent Test**: prepare media files and executable files with known metadata, and verify display, filtering, and resilience to corrupted files.

**Acceptance Scenarios**:

1. **Given** there are media files in the analysis area, **When** the user opens the metadata column, **Then** the supported tags are displayed.
2. **Given** the filter "resolution above 4K" is set, **When** it is applied, **Then** the list contains only matching files.
3. **Given** an executable file is selected, **When** the properties are open, **Then** the file and product versions are displayed.
4. **Given** the file is corrupted or contains no metadata, **When** reading is performed, **Then** "n/a" is shown without an application failure.

---

### User Story 15 — Analysis by file owners (Priority: P3)

The user sees how much space is occupied by files of each owner, can filter the tree by owner, and can use the report to find space abuse in a corporate environment or on a shared computer.

**Why this priority**: in organizations and on PCs with multiple accounts, it is important to understand who exactly is consuming space.

**Independent Test**: on a set with files of different owners, verify the totals by owner, filtering, and the "unknown" category.

**Acceptance Scenarios**:

1. **Given** the analysis is complete, **When** the owner report is open, **Then** the total size and file count are shown for each owner.
2. **Given** an owner is selected, **When** the filter is applied, **Then** the tree shows only that owner's objects.
3. **Given** the owner of an object cannot be determined, **When** the report is built, **Then** the object falls into the "unknown" category.

---

### User Story 16 — Viewing archive contents without extraction (Priority: P3)

The user opens ZIP, RAR, 7z, TAR/GZ, or ISO directly in the application, sees the list of files inside with sizes and shares, finds the heaviest items, and exports the list.

**Why this priority**: archives and images often hide a significant part of the occupied space, and extracting them for analysis is impractical.

**Independent Test**: open archives of known formats with known contents, and verify the list, sorting, handling of passwords, and corrupted files.

**Acceptance Scenarios**:

1. **Given** a supported archive is selected, **When** the user opens it, **Then** a tree or list of contents with sizes is displayed.
2. **Given** the archive contains a large file, **When** sorting by size is applied, **Then** this file is at the top.
3. **Given** the archive is corrupted or password-protected, **When** the user tries to open it, **Then** a clear message or password prompt is shown, and the application does not crash.
4. **Given** the contents list is open, **When** the user performs an export, **Then** they receive a report in the selected format.

---

### User Story 17 — Report export (Priority: P4)

The user exports analysis results to CSV, XML, HTML, or Excel, chooses the composition of the report, and passes it to a system administrator or saves it for statistics.

**Why this priority**: sharing results and documenting disk state are needed in teamwork but do not block basic cleanup.

**Independent Test**: export the same result to all formats and verify opening in the target tools, encodings, and matching sums.

**Acceptance Scenarios**:

1. **Given** the analysis result is ready, **When** the user exports it to CSV, **Then** the file opens in Excel with the correct encoding, and the sums match the application.
2. **Given** the HTML format is selected, **When** the export is complete, **Then** the report reads in a browser and contains a summary and tables.
3. **Given** the XML format is selected, **When** the export is complete, **Then** the structure conforms to the documented schema.
4. **Given** the Excel format is selected, **When** the export is complete, **Then** the data, headers, and total sums are correct.

---

### User Story 18 — Command line and scheduled runs (Priority: P4)

The user automates scanning, duplicate search, cleanup, export, and snapshots via the command line, receives machine-readable output, and configures runs via Windows Task Scheduler.

**Why this priority**: automation is critical for system administrators but secondary for the average user.

**Independent Test**: run all main commands in non-interactive mode, and verify JSON output, return codes, and scheduled execution.

**Acceptance Scenarios**:

1. **Given** a scan command with machine-readable output, **When** it is executed, **Then** standard output contains valid JSON, and errors are directed to the error stream.
2. **Given** non-interactive mode, **When** an operation requires confirmation, **Then** it is not performed without an explicit confirmation flag, and the return code reflects the reason.
3. **Given** a task is added to Windows Task Scheduler, **When** the start time arrives, **Then** the report is created, and the return code is zero on success.
4. **Given** some capability is available in the graphical interface, **When** the user looks for its counterpart in the command line, **Then** the counterpart exists and is documented.

---

### User Story 19 — Integration into the Windows context menu (Priority: P4)

The user starts size analysis or file unlocking directly from the Explorer context menu, and also enables, disables and removes this integration. By default and immediately after installation there are no entries; the portable build contains no integration at all.

**Why this priority**: speeds up access to the main scenarios but does not affect the analysis functionality itself.

**Independent Test**: verify that no entries exist after installation, enable the integration through the setting, verify the menu items for a file, folder, and disk, disable the setting and remove the entries with the CLI command.

**Acceptance Scenarios**:

1. **Given** the application has just been installed or the setting is disabled, **When** the user opens the context menu, **Then** there are no FreeSpace items.
2. **Given** `context_menu_enabled` is enabled, **When** the user opens the context menu of a folder, **Then** they see an analysis item that launches the application with the selected path.
3. **Given** a file is busy with a process, **When** the user selects the unlock item, **Then** the list of blocking processes opens.
4. **Given** the setting is disabled, **When** the user checks the context menu, **Then** the items disappear.
5. **Given** the portable build is used, **When** the user checks the menu or the `context-menu` command, **Then** the integration and the command are absent.
6. **Given** the application is uninstalled, **When** uninstall completes, **Then** the entries and the FreeSpace data directory are absent.

---

### User Story 20 — Real-time disk write monitoring (Priority: P4)

In the background, the user sees which programs are currently writing data to disk, at what speed, and to which directories, and receives a notification when a configured threshold is exceeded.

**Why this priority**: helps identify the causes of sudden disk filling but is an addition to the main analysis.

**Independent Test**: start a test write of a known size, and verify the display of the process, speed, and the notification at the threshold.

**Acceptance Scenarios**:

1. **Given** monitoring is enabled, **When** a process is actively writing data, **Then** the process, speed, and write path are displayed.
2. **Given** a threshold for the size exceeded per interval is set, **When** the threshold is exceeded, **Then** a notification appears.
3. **Given** the application window is minimized, **When** monitoring is enabled, **Then** it continues to work in the notification area.
4. **Given** monitoring is stopped, **When** the user opens the report, **Then** the history of recorded events is available.

---

### User Story 21 — System directories and privilege elevation (Priority: P4)

The user launches the application with administrator rights on demand and gains access to hidden system directories, including System Volume Information, where regular Explorer does not allow entry. Without elevated rights, the application works correctly in a limited mode.

**Why this priority**: access to system areas is important for a complete analysis but requires explicit privilege elevation.

**Independent Test**: verify operation without and with elevated rights, access to the protected directory, and correct degradation when elevation is declined.

**Acceptance Scenarios**:

1. **Given** the application is running without administrator rights, **When** the user opens a protected directory, **Then** privilege elevation is offered with an explanation of the reason.
2. **Given** rights are elevated, **When** the analysis is performed, **Then** protected system objects are accessible and accounted for.
3. **Given** the user declined privilege elevation, **When** the analysis completes, **Then** protected areas are marked as inaccessible, and the remaining data is processed correctly.
4. **Given** the operation affects a system object, **When** the user confirms it, **Then** an additional warning is displayed.

---

### User Story 22 — Dashboard of all disks (Priority: P4)

The application shows a summary of all volumes on the computer: total size, used and free space, file system type, largest consumers, and the dynamics of usage change based on snapshots.

**Why this priority**: a convenient entry point and overview of the machine's state; it complements but does not replace the analysis of a specific volume.

**Independent Test**: verify the display of all available volumes, the correctness of the figures, and quick navigation to the analysis of the selected volume.

**Acceptance Scenarios**:

1. **Given** the application is running, **When** the dashboard is open, **Then** for each available volume, used, free, and total space are visible, as well as the file system type.
2. **Given** a volume is selected on the dashboard, **When** the user starts the analysis, **Then** the tree of that volume opens.
3. **Given** there are saved snapshots for the volume, **When** the dashboard is open, **Then** the dynamics of usage change are shown.

---

### User Story 23 — Settings, localization, and the portable build (Priority: P4)

The user configures units of measurement, exclusions, thresholds, the visual theme, the interface language and the Explorer context-menu integration. All application data is stored only in the `data` directory next to the executable; the portable build contains no Explorer integration.

**Why this priority**: comfort and flexibility are important for regular use but do not affect the main analysis scenarios.

**Independent Test**: switch the language and theme, set exclusions, run the application from a read-only folder, and verify that no records are written to system directories.

**Acceptance Scenarios**:

1. **Given** the application is running, **When** the user switches the interface language, **Then** all interface strings are displayed in the selected language, and the current state is not lost.
2. **Given** the application runs from any location, **When** it saves settings or the journal, **Then** the data is in the `data` directory next to the executable, and `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` and temporary directories are not used.
3. **Given** the application folder is read-only, **When** the user saves settings, **Then** a clear error is shown, the settings remain in memory for the session, and no write to a system directory is attempted.
4. **Given** the user has set excluded paths, **When** the analysis is performed, **Then** excluded objects are not counted, and the fact of exclusion is reflected in the report.
5. **Given** light and dark themes are available, **When** the user switches the theme, **Then** the interface and the map are redrawn correctly.
6. **Given** the user enables or disables the context-menu setting, **When** settings are saved, **Then** the entries appear or disappear without restarting the application.

---

### Edge Cases

- What happens when a file or directory is changed or deleted during scanning? The scan MUST mark such objects as "changed during analysis" and not distort the totals.
- How does the system handle a volume that is disconnected or becomes unavailable during an operation? The operation MUST be aborted with a clear error, and already performed actions MUST remain in the journal.
- What happens with cyclic symbolic links? The scan MUST detect cycles and not loop.
- How are paths longer than the system limit handled? The operation MUST complete with a clear error before any data is changed.
- What happens if a destructive operation is interrupted due to a power failure? The journal MUST allow determining the state and restoring objects from quarantine.
- What happens if there is not enough free space for the Recycle Bin or quarantine? The system MUST show the required size and offer to free up space, choose permanent deletion, or cancel the operation; partial execution without explicit confirmation is unacceptable.
- How are password-protected archives, corrupted archives, and archives whose extraction requires disproportionate resources handled? The application MUST limit resources and report the problem without crashing.
- What happens when names collide during restoration from quarantine or bulk rename? Conflicts MUST be detected before applying, and the resolution strategy MUST be predictable.
- How are EFS-encrypted, compressed, and sparse files handled during shredder deletion? The application MUST warn about warranty limitations.
- What happens if the application is launched from a folder without write permissions? Settings MUST remain in memory for the session and the user MUST receive a clear notification; a fallback write to `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` or a temporary directory MUST NOT happen.
- How are simultaneous launches of multiple application instances or conflicting scheduled tasks handled? Conflicting operations MUST be blocked or queued.
- How are names with Unicode, emoji, and surrogate pairs handled? Display, search, and operations MUST work without data loss.
- What happens when exporting a report with millions of rows? The export MUST be performed in a streaming manner and not lead to memory exhaustion.
- How are files whose contents changed during hashing handled? The duplicate group MUST be marked as outdated.

## Requirements *(mandatory)*

### Functional Requirements

**Scanning, tree, and navigation**

- **FR-001**: The user MUST be able to select a volume, directory, or set of directories for analysis.
- **FR-002**: The system MUST build a hierarchical tree in which each node displays the size, the number of files, and the number of nested directories.
- **FR-003**: For NTFS volumes, the system MUST use direct MFT reading and MUST analyze a volume up to 8 TB in no more than 60 seconds on a typical consumer SSD.
- **FR-004**: For file systems other than NTFS, the system MUST use directory tree traversal and MUST explicitly display the applied scanning method.
- **FR-005**: The system MUST display the logical size, the size on disk, and, where applicable, the physical size of an object.
- **FR-006**: The system MUST mark objects inaccessible due to permissions without interrupting the analysis and without distorting the totals.
- **FR-007**: The system MUST support search and filtering by name, mask, extension, size, dates, attributes, owner, and metadata.
- **FR-008**: The system MUST support sorting by name, size, date, type, owner, and metadata.
- **FR-009**: The system MUST support incremental updating of a repeated NTFS volume analysis via the USN change journal.
- **FR-010**: Scanning MUST be performed in the background with progress display and the ability to cancel, without blocking the interface.
- **FR-011**: The user MUST be able to set excluded paths that are not counted during analysis.
- **FR-012**: With an unchanged file system, the results of a repeated scan MUST be deterministic.

**Treemap**

- **FR-013**: The system MUST display an interactive Treemap in which the area of a rectangle is proportional to the size of the object.
- **FR-014**: The map MUST support drill-down into directories, backward navigation, and "breadcrumbs".
- **FR-015**: The map MUST show a tooltip with the name, full path, size, and share of the object.
- **FR-016**: The map MUST support coloring by file type with a legend.
- **FR-017**: The map MUST remain responsive on volumes with millions of files thanks to levels of detail and rendering virtualization.

**Top files and types**

- **FR-018**: The system MUST produce a flat list of the top-N largest files with a configurable N (at least 100 and 1000).
- **FR-019**: The system MUST group occupied space by extensions and categories, indicating the sum, the number of files, and the share.
- **FR-020**: The system MUST provide navigation from the top-files list and type groups to the corresponding node of the tree or map.

**File unlocking**

- **FR-021**: The system MUST identify the processes and handles holding the selected file.
- **FR-022**: The system MUST allow releasing a lock by closing a handle or terminating a process only after explicit confirmation.
- **FR-023**: The system MUST allow placing a locked file in the system delete-on-reboot queue.
- **FR-024**: All unlock operations MUST be journaled, indicating the process, time, and result.

**Duplicates**

- **FR-025**: The system MUST search for duplicates by name, size, or checksum (MD5/SHA-256) in the selected area.
- **FR-026**: The system MUST confirm a match with a full byte-for-byte comparison after comparing hashes.
- **FR-027**: The system MUST group duplicates and show the potentially reclaimable space.
- **FR-028**: The system MUST support rules for automatically selecting the copy to keep: oldest, newest, shortest path, original directory.
- **FR-029**: Deletion of duplicates MUST be performed only after preview and confirmation and MUST guarantee that at least one copy is preserved.

**Shredder and delete on reboot**

- **FR-030**: The system MUST perform permanent deletion by overwriting with zeros or random data with a configurable number of passes (at least one).
- **FR-031**: For SSDs, the system MUST show a warning about warranty limitations, require explicit confirmation, and initiate TRIM after overwriting; for HDDs, the warning is limited to the irreversibility of the operation.
- **FR-032**: The system MUST support placing files in the system deletion queue before the Windows graphical shell loads.
- **FR-033**: Shredder operations MUST be journaled, indicating the method, the number of passes, and the result.

**Stale data, empty folders, recommendations**

- **FR-034**: The system MUST filter objects by last access, modification, and creation date with configurable thresholds; for the "stale" criterion, the maximum of the access date and the modification date MUST be used, and the unreliability of the access date MUST be explicitly marked.
- **FR-035**: The system MUST identify categories of temporary files, cache, logs, and backups according to defined rules.
- **FR-036**: The system MUST find empty directories, including nested ones, and support their batch deletion after confirmation.
- **FR-037**: The system MUST generate cleanup recommendations with an estimate of the space to be freed and the risk level.

**Snapshots**

- **FR-038**: The system MUST save a disk state snapshot to a file with metadata of the volume, time, and scanning method, and the format version.
- **FR-039**: The system MUST compare two snapshots and show added, deleted, and changed objects and growth by directory.
- **FR-040**: The system MUST support sorting comparison results by growth and exporting the comparison report.
- **FR-041**: The system MUST verify snapshot compatibility and produce a clear error when volumes or format versions do not match.

**Operation safety**

- **FR-042**: Any destructive operation MUST be performed only after previewing the affected objects, their total size, and explicit confirmation.
- **FR-043**: For each destructive operation, the system MUST offer the user a choice of deletion method: Windows Recycle Bin, application quarantine, or permanent deletion, with an explanation of the consequences of each option; the selected method MUST apply only to the current operation. If there is not enough free space for the selected method, the system MUST show the required size and offer to free up space, choose permanent deletion, or cancel the operation.
- **FR-044**: The system MUST keep a journal of all operations, support undo where possible, and support restoration of objects from quarantine.
- **FR-045**: Operations on system and protected paths MUST require elevated rights and additional confirmation.
- **FR-046**: The system MUST guarantee that operations do not go beyond the area explicitly selected by the user.

**Links, compression, sparse files, and ADS**

- **FR-047**: The system MUST distinguish hard links, junction points, and symbolic links, MUST mark them, and MUST NOT traverse links without explicit user instruction.
- **FR-048**: The system MUST count hard links once in the physical size and separately show their logical contribution.
- **FR-049**: The system MUST show the logical and physical size of compressed and sparse files, as well as the amount of savings.
- **FR-050**: The system MUST list NTFS alternate data streams and account for their size.

**Long paths**

- **FR-051**: The system MUST perform rename, move, copy, and delete for paths longer than 260 characters without enabling the system long paths policy.
- **FR-052**: When the system path length limit is exceeded, the system MUST produce a clear error before any data is changed.
- **FR-053**: Journals and reports MUST contain full paths without truncation.

**Bulk rename**

- **FR-054**: The system MUST support rename rules based on regular expressions, patterns, case changes, extensions, numbering, and metadata.
- **FR-055**: The system MUST show a preview, detect conflicts, and support rollback of a completed rename.
- **FR-056**: Bulk renaming of thousands of files MUST be performed without blocking the interface, with progress display.

**NTFS attributes and permissions**

- **FR-057**: The system MUST support batch setting and clearing of the "Read-only", "Hidden", "System", and "Archive" attributes.
- **FR-058**: The system MUST support batch owner change, reset and copy of NTFS permissions, as well as inheritance management.
- **FR-059**: Permission operations MUST be accompanied by a report on successful and failed objects, indicating the reasons.
- **FR-060**: When permissions are insufficient, the system MUST request privilege elevation and MUST clearly explain a refusal.

**Metadata**

- **FR-061**: The system MUST read and display metadata: EXIF (camera model, shooting date, resolution), audio (bitrate, duration), video (resolution, codec, duration), executable files (file and product version).
- **FR-062**: The system MUST support filtering and sorting by supported metadata.
- **FR-063**: Metadata read errors MUST not interrupt application operation and MUST be displayed as "n/a".

**Owners**

- **FR-064**: The system MUST generate a report on the total size and number of files by owner with an "unknown" category.
- **FR-065**: The system MUST support filtering of the tree and lists by owner.

**Archives**

- **FR-066**: The system MUST display the contents of ZIP, 7z, TAR/GZ, ISO, and RAR archives (if a compatible library is available) without full extraction.
- **FR-067**: The system MUST show the sizes of archive entries and their share of the contents, and support searching for the largest entries.
- **FR-068**: The system MUST correctly handle password-protected archives and corrupted archives, and limit resources when an archive bomb is suspected.
- **FR-069**: The system MUST support exporting the list of archive contents.

**Report export**

- **FR-070**: The system MUST export analysis results to CSV, XML, HTML, and Excel.
- **FR-071**: The user MUST be able to choose the composition and scope of the exported report.
- **FR-072**: Export MUST work correctly with encodings and large volumes of data and MUST preserve the consistency of sums.
- **FR-073**: Report formats and schemas MUST be documented.

**Command line and scheduling**

- **FR-074**: Every graphical interface feature MUST have a command-line counterpart with machine-readable output (JSON) and error messages on the error stream.
- **FR-075**: The command line MUST support a non-interactive mode and MUST require an explicit confirmation flag for destructive operations.
- **FR-076**: Command return codes MUST be documented and stable within a major version.
- **FR-077**: The system MUST provide examples and templates for running via Windows Task Scheduler.
- **FR-078**: The system MUST prevent conflicting parallel runs or queue them.

**Context menu**

- **FR-079**: The system MUST NOT add Explorer context-menu items by default or during installation; size analysis and unlock items for files, folders, and volumes MUST be enabled and disabled by the `context_menu_enabled` setting. The portable build MUST NOT contain the context-menu component or the `context-menu` command.
- **FR-080**: The system MUST support installing and removing the integration both for the current user and for all users when permissions are available. Uninstalling the application MUST remove its files, the data directory and the context-menu entries; the CLI MUST provide a command for removing the entries.
- **FR-081**: Passing paths from the context menu MUST work correctly with spaces, Unicode, and long paths.

**Real-time monitoring**

- **FR-082**: The system MUST display processes, paths, and disk write speed in real time.
- **FR-083**: The system MUST support notifications when configured thresholds are exceeded and keep an event history.
- **FR-084**: Monitoring MUST work in the background and in the notification area with minimal impact on the system.
- **FR-085**: The user MUST be able to exclude processes and paths from monitoring.

**Permissions and system directories**

- **FR-086**: The system MUST request privilege elevation on demand with an explanation of the reason.
- **FR-087**: When permissions are available, the system MUST provide access to System Volume Information and other protected directories.
- **FR-088**: Without elevated rights, the system MUST degrade correctly, marking inaccessible areas.

**Disk dashboard**

- **FR-089**: The system MUST show a summary of all available volumes: total size, used and free space, file system type.
- **FR-090**: The system MUST provide quick start of analysis for the selected volume and display of usage dynamics based on saved snapshots.

**Settings, localization, and portability**

- **FR-091**: The user MUST be able to configure units of measurement, exclusions, thresholds, the visual theme, and the interface language.
- **FR-092**: The interface MUST be fully localized into Russian and English; switching the language MUST NOT cause loss of the current state.
- **FR-093**: The system MUST store settings, the journal, quarantine, the hash cache and locks only in the `data` directory next to the executable. `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` and temporary directories MUST NOT be used. Without write access the system MUST keep settings in memory for the session, report the failure and MUST NOT choose a fallback system directory.
- **FR-094**: The application MUST NOT send telemetry and MUST NOT perform network requests by default.
- **FR-095**: All application features MUST be available without a network connection.
- **FR-096**: The portable build MUST be produced without the context-menu component; the installer MUST NOT register menu entries, and the uninstaller MUST remove the data directory and menu entries.

### Key Entities *(including data, if applicable)*

- **Volume**: a disk or partition; attributes — letter, label, file system, total/used/free size, scanning method, availability.
- **File system node (FsNode)**: a file, directory, or link; attributes — name, full path, type, sizes (logical, on disk, physical), dates, attributes, owner, accessibility flag.
- **Size metrics (SizeMetrics)**: logical size, size on disk, allocated size, hard link contribution, alternate stream size.
- **Link (FsLink)**: type (hard, junction, symbolic), target, flag indicating whether the target object is counted.
- **Scan session (ScanSession)**: volume, scope, method, start and end time, status, exclusions, access errors.
- **Duplicate group (DuplicateGroup)**: checksum, size, list of members, copy recommended to keep, relevance flag.
- **Cleanup candidate (CleanupCandidate)**: category, reason, size, risk level, recommended action.
- **Snapshot**: format version, volume, date, scanning method, aggregated tree data.
- **Snapshot delta (SnapshotDiff)**: added, deleted, and changed objects and growth by directory.
- **File lock (FileLock)**: process, process ID, handle, access type, ability to release.
- **Operation**: type, list of objects, parameters, status, reversibility, permission requirement.
- **Journal entry (JournalEntry)**: time, user, operation, result, rollback capability.
- **Quarantine item (QuarantineItem)**: original path, placement date, reason, retention period, restore status.
- **Report**: format, composition, scope, file path, creation date.
- **Automation profile (AutomationProfile)**: command, parameters, schedule, return code, journal path.
- **Monitoring event (DiskEvent)**: process, path, write speed, size, time.
- **Metadata (MetadataRecord)**: file type, tags and values (EXIF, audio, video, executable version).
- **Owner summary (OwnerSummary)**: owner, total size, number of files.
- **Archive entry (ArchiveEntry)**: path inside the archive, size, compressed size, share.
- **Settings**: units of measurement, exclusions, thresholds, theme, language, context-menu enablement (disabled by default). The only storage mode is the `data` directory next to the executable.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A full analysis of an 8 TB NTFS volume on a typical consumer SSD takes no more than 60 seconds.
- **SC-002**: The interface remains responsive during scanning and long-running operations: response to user actions does not exceed 200 ms, and progress is displayed continuously.
- **SC-003**: On the control data set, discrepancies between the sizes in the application and the reference values are 0 bytes, including hard links, compressed files, sparse files, and ADS.
- **SC-004**: Duplicate search on the control set finds 100% of true duplicates and produces no false matches.
- **SC-005**: No object is deleted without explicit confirmation; 100% of destructive operations are recorded in the journal.
- **SC-006**: After the shredder runs on the control set, an independent recovery tool recovers none of the files (for HDD).
- **SC-007**: 100% of operations with paths from 261 to 32,000 characters complete successfully or with a clear error before any data is changed.
- **SC-008**: Snapshot comparison in the control scenario identifies 100% of added, deleted, and changed objects.
- **SC-009**: All four export formats open in the target tools without errors, and the sums in the reports match the application.
- **SC-010**: 100% of graphical interface capabilities are available from the command line; machine-readable output passes validation against the documented schema.
- **SC-011**: On the prepared control set, a typical cleanup scenario allows freeing at least 10 GB in no more than 10 minutes, including confirmation of operations.
- **SC-012**: 100% of interface strings have translations into Russian and English; switching the language does not cause loss of state data.
- **SC-013**: 100% of settings, journals, quarantine, cache and locks are located in the `data` directory next to the executable; no records are created in `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` or a temporary directory.
- **SC-014**: A disk write event is displayed in monitoring no later than 2 seconds after the write starts, and idle overhead does not exceed 2% of CPU.
- **SC-015**: At least 80% of testing participants find the largest file on the volume without instructions in less than 1 minute after starting the analysis.
- **SC-016**: Starting the analysis of the selected object from the context menu opens the application with the required path in no more than 2 seconds.
- **SC-017**: After installation and with default settings there is not a single FreeSpace context-menu entry; after uninstalling the application neither entries nor the data directory remain.
- **SC-018**: The portable build contains no `context-menu` command and creates no Explorer registry entries.

## Assumptions

- The target operating systems are Windows 10 version 21H2 and later, as well as Windows 11, x64 architecture; the primary scenario is local disks with NTFS.
- Network locations (including UNC paths), cloud storage, and remote machines are out of scope for v1; only local and mounted volumes are analyzed, including removable media with supported file systems. For non-NTFS volumes, tree traversal is used in a limited-functionality mode.
- The user has the ability to elevate rights to administrator for access to system directories and for changing the owner and NTFS permissions; without elevation, the application works in a limited mode.
- The application is intended for local single-user use; server and multi-user modes are out of scope for v1.
- A third-party library may be used to view RAR; if it is absent, the archive is displayed as an ordinary file with an explanation.
- Guarantees of permanent deletion on SSD are limited by the internal wear-leveling logic; the application is obliged to warn about this.
- Snapshots are stored in local files and are compatible within a major version of the format; transferring snapshots between machines is allowed when the volume matches.
- Reports in Excel format are created in a format that opens in Microsoft Excel and LibreOffice Calc without additional settings.
- The v1 interface languages are Russian and English; if a translation is missing, the string is displayed in English.
- The application does not perform recovery of deleted files, defragmentation, backup, antivirus scanning, and does not manage other computers.
- Cross-platform support (Linux, macOS) is out of scope for v1.
- Only one destructive operation can be performed on a volume at a time; the others are queued or blocked with a clear message.
