# Feature Specification: FreeSpace — a unified next-generation native interface

**Feature Branch**: `002-native-ui-redesign`

**Created**: 2026-11-16

**Status**: Implemented

## Clarifications

### Session 2026-11-20

- Q: Which screen should open when the application starts? → A: Always the Overview, regardless of the stored `last_screen`.

## User Scenarios & Testing *(mandatory)*

<!--
  Stories are ordered by importance. Each story is independently verifiable:
  implementing only one story must produce a working increment.
-->

### User Story 1 — Core: volumes, scanning, tree (Priority: P1)

The user launches the new interface, sees a dashboard of available volumes with used and free space, starts an analysis of the selected volume and watches gradual progress. When the analysis completes, they open the virtualized size tree with inline progress bars, drill into large directories and see details of an object in a compact hover card. The interface remains responsive on a volume with millions of nodes, and a long scan can be cancelled.

**Why this priority**: this is the core of the product and the minimum viable product of the new client. Without volumes, scanning and the tree, the other 9 screens have no data and cannot be verified.

**Independent Test**: on a test VHD or a temporary directory with a known structure, start an analysis, verify the final totals and the number of objects, check that progress is gradual, that the tree loads page by page, that inline node progress bars work, that the hover details card works, and that the scan can be cancelled.

**Acceptance Scenarios**:

1. **Given** the application is running and local volumes are available, **When** the "Overview" screen is open, **Then** each volume shows its letter, label, file system, total, used and free space, as well as an "Analyze" action.
2. **Given** the user has selected a volume, **When** they start the analysis, **Then** progress, the current path, the number of processed objects and the elapsed time are updated at least once per second, and the interface remains responsive.
3. **Given** an analysis is running, **When** the user clicks "Cancel", **Then** scanning stops within 2 seconds, partial results are saved with an explicit "incomplete" mark, and the interface returns to a working state.
4. **Given** the analysis is complete, **When** the "Tree" screen is open, **Then** the first screen of rows is displayed in no more than 1 second, scrolling and expanding nodes do not block the interface, and sizes and file counts match the reference.
5. **Given** the tree is displayed, **When** the user expands a node, **Then** the row shows an inline progress bar proportional to the node's share of its parent, and hovering or focusing a row opens the details card.
6. **Given** the user drills into a directory in the tree, **When** they collapse a node or use "Back", **Then** the tree returns to the previous level without re-reading the entire volume.
7. **Given** the volume contains 10 million nodes, **When** the user scrolls the tree and expands nodes, **Then** the response to an action does not exceed 200 ms, and tree rendering stays at no less than 30 frames per second.

---

### User Story 2 — Analysis and cleanup: top, types, duplicates, cleanup and snapshots (Priority: P1)

The user finds the largest files, looks at the distribution by types, searches for duplicates, receives recommendations for safe cleanup and compares the state of the disk with a snapshot. The analysis and cleanup screens use the common interface language and the common table, filter and card components.

**Why this priority**: these are the most frequent tasks after building the tree. They deliver immediate practical value and prove that the new UI kit withstands dense data and bulk actions.

**Independent Test**: on a control set with known largest files, extensions, duplicates and two snapshots, verify the correctness of totals, filters, sorting, auto-selection rules and comparison, without performing destructive operations on real data.

**Acceptance Scenarios**:

1. **Given** the analysis is complete, **When** the "Analysis" screen is open on the "Largest files" tab, **Then** the user can choose N (100, 1000 or a custom value), filter by path and type, jump to the object in the tree and see the full path and size.
2. **Given** the "Analysis" screen is open on the "Types" tab, **When** the user selects an extension or a category, **Then** the total size, the number of files and the share of used space are shown, and the filter is applied without reloading the screen.
3. **Given** the "Duplicates" screen is open, **When** the search is complete, **Then** groups of exact matches are shown with the potentially reclaimable volume, and the "keep one copy" rule does not delete anything before confirmation.
4. **Given** the "Cleanup" screen is open, **When** the user selects a recommendation, **Then** the category, volume, risk level and explanation are visible, and the action leads to the common operation preview.
5. **Given** two compatible snapshots are saved, **When** the user opens the "Cleanup" screen on the "Snapshots" tab and starts a comparison, **Then** the report shows growth and reduction by directory, and incompatible snapshots produce a clear error without data loss.
6. **Given** the data changed during the duplicate search, **When** a discrepancy is detected, **Then** the group is marked as stale and is not offered for deletion.

---

### User Story 3 — Safe operations with preview, journal and undo (Priority: P1)

Any destructive operation in the new interface goes through a single preview dialog: the list of objects, the total volume, the deletion method, warnings and the permissions requirement. After execution the operation appears in the journal, and where it is technically possible, undo is available.

**Why this priority**: the new presentation layer must not weaken the main guarantee of the product. A mistake in the UI wiring of dangerous operations is irreversible, so safety matters more than the speed of building screens.

**Independent Test**: on a temporary directory, perform deletion to the Recycle Bin, to quarantine and permanent deletion; verify the preview, confirmation, journal record, restoration from quarantine and undo where it is supported.

**Acceptance Scenarios**:

1. **Given** the user initiates a deletion, **When** the preview opens, **Then** all affected objects, the total volume, the irreversibility and the choice of method are shown: Windows Recycle Bin, application quarantine or permanent deletion.
2. **Given** a deletion method is selected, **When** the user confirms, **Then** the operation is performed only through the native `freespace-ops` layer, and the interface shows progress and allows cancelling the operation while it is cancellable.
3. **Given** the operation affects a system or protected path, **When** the user requests it, **Then** the interface requires privilege elevation and additional confirmation, and declining the elevation leaves the data unchanged.
4. **Given** the operation is complete, **When** the "Journal" screen is open, **Then** the record contains the time, user, type, objects, result and the availability of undo or restoration.
5. **Given** objects are in quarantine, **When** the user chooses "Restore", **Then** the objects return to their original paths, and name conflicts are resolved predictably and shown before applying.
6. **Given** the user closes the confirmation dialog, **When** the operation has not started yet, **Then** no data is changed and no background tasks remain.

---

### User Story 4 — File system and operations: journal, details, long paths, rename and permissions (Priority: P2)

The user works with non-standard objects: examines size details in the tree hover card, operates on paths longer than 260 characters, performs bulk rename and changes NTFS permissions. All actions use the same dialogs, tables and states as the other screens.

**Why this priority**: these are unique capabilities of the product, but they rely on the already finished core and the common UI kit. They can be shipped after the core and cleanup.

**Independent Test**: create a temporary structure with long paths and test permissions, perform rename, move, delete and permission change, verify the preview, the journal and error handling.

**Acceptance Scenarios**:

1. **Given** the tree is open and the pointer or focus rests on an object row, **Then** the "Size details" hover card shows the logical size, the size on disk, the hard link contribution, ADS, compression and sparse savings, and the size type is explicitly labeled.
2. **Given** the path exceeds 260 characters, **When** the user renames, moves or deletes the object, **Then** the operation is performed without enabling the system long-path policy, and the full path is visible in the preview and the journal.
3. **Given** the "File operations" screen is open on the "Rename" tab, **When** the user sets a template and a scope, **Then** the preview shows old and new names, conflicts and the number of affected objects before applying.
4. **Given** the "File operations" screen is open on the "Permissions" tab, **When** the user changes an ACL, **Then** a preview of the changes is shown before applying, and without elevated privileges the interface explains the reason for the refusal.
5. **Given** an operation with a long path or with permissions failed, **When** the journal is open, **Then** the record contains the full path, the error code and a clear explanation, without losing previous records.

---

### User Story 5 — Inspection and reports: metadata, owners, archives, export and unlocking (Priority: P2)

The user examines content without changing data: reads metadata, looks at the distribution by owners, previews archives without extraction, exports reports and finds out which process is locking a file.

**Why this priority**: these are read and diagnostic scenarios. They extend the value of the product but do not block the main cleanup operations.

**Independent Test**: on a set with EXIF/audio/video files, an archive, known owners and a held file, verify the display of data, the export and the list of blocking processes without changing any files.

**Acceptance Scenarios**:

1. **Given** the "Inspect" screen is open on the "Metadata" tab, **When** a file is selected, **Then** the available metadata is shown with its source indicated, and the absence of data does not look like an error.
2. **Given** the "Analysis" screen is open on the "Owners" tab, **When** the data is collected, **Then** the owner, the total volume, the number of files and the navigation to objects are shown, and inaccessible objects are marked.
3. **Given** the "Inspect" screen is open on the "Archives" tab, **When** the user previews an archive's content, **Then** the paths inside the archive, sizes and the share of entries are visible, and password-protected, corrupted and suspicious archives produce a clear message without extraction to disk.
4. **Given** the composition of the report is chosen, **When** the user invokes the global "Export" action (CSV, XML, HTML or Excel), **Then** the file is created in the chosen location, and the totals match the interface.
5. **Given** a file is held by a process, **When** the user opens the "Inspect" screen on the "Unlock" tab, **Then** the processes, handles and access type are shown, and removing the lock requires confirmation.
6. **Given** the file is locked at the kernel level, **When** the user chooses deletion on reboot, **Then** the operation is registered and reflected in the journal.

---

### User Story 6 — Operations and settings: shredder, monitoring and settings (Priority: P2)

The user configures the application, works with permanent data destruction, observes disk writes in real time, elevates privileges on demand and sees information about the version and the license.

**Why this priority**: this is the final batch of screens. It is needed for full parity but does not prevent shipping the core, the analysis and the file operations.

**Independent Test**: on temporary files, verify the theme, language, density and unit settings; perform a test destruction; turn write monitoring on and off; verify the elevation request and the "About" screen.

**Acceptance Scenarios**:

1. **Given** the "Settings" screen is open, **When** the user changes the theme, language, density, units or reduced motion, **Then** the change is applied without a restart and without losing the current screen and selection.
2. **Given** files are selected for destruction, **When** the shredder is open, **Then** the method, the number of passes, the warning about irreversibility and SSD limitations are shown, and starting requires separate confirmation.
3. **Given** the "Write monitoring" screen is open, **When** a process writes to disk, **Then** the event appears no later than 2 seconds afterwards, and the list can be filtered by process and path.
4. **Given** an operation needs administrator privileges, **When** the user agrees through the "Elevation" section of the "Settings" screen, **Then** the reason for the elevation is shown, and declining returns to the original state without changing data.
5. **Given** the "About" section of the "Settings" screen is open, **When** it is displayed, **Then** the version, build, WebView2, licenses and links to local documentation are visible without network requests.

---

### User Story 7 — Both themes, DPI and functional motion (Priority: P2)

The user chooses the dark, light or system theme. The interface looks correct at 100, 125, 150 and 200% scaling, does not "jump" at startup and respects the system-wide disabling of animations.

**Why this priority**: a unified visual language is the main reason for the redesign. Without both themes and correct DPI, the product cannot be considered ready to replace the old UI.

**Independent Test**: switch the theme on every type of screen, restart the application and confirm there is no flash; check 100/125/150/200% DPI and reduced motion on the key screens.

**Acceptance Scenarios**:

1. **Given** the first launch, **When** the interface appears, **Then** the theme is applied before the first rendering of content and no flash of the light theme occurs when the dark setting is selected.
2. **Given** the system theme is selected, **When** the Windows theme changes, **Then** the interface updates without a restart and without losing state.
3. **Given** the light theme is selected, **When** any of the 11 screens is open, **Then** text and controls meet AA contrast, and the tree and charts use the light theme palette.
4. **Given** Windows scaling is 200%, **When** a dense screen is open, **Then** elements are not clipped or overlapped and remain accessible from the keyboard.
5. **Given** "Reduce motion" is enabled in the system, **When** the user opens panels and dialogs, **Then** optional transitions are disabled, while functional state changes remain clear.

---

### User Story 8 — Keyboard, focus and screen reader (Priority: P2)

The user performs the main tasks without a mouse: moves through the shell and tables, opens dialogs, confirms and cancels operations, and receives announcements about progress and errors through a screen reader.

**Why this priority**: accessibility is a mandatory quality of a native application and an acceptance condition. It cannot be added after 11 screens have been laid out without semantics.

**Independent Test**: walk through ten key scenarios using only the keyboard and with Narrator enabled, checking focus order, focus traps, focus return after dialogs and progress announcements.

**Acceptance Scenarios**:

1. **Given** the application is running, **When** the user presses Tab, **Then** focus moves sequentially through the shell and the current screen in a logical order, and the current element is visually highlighted.
2. **Given** a dialog is open, **When** the user presses Tab or Shift+Tab, **Then** focus does not leave the dialog, and Esc closes it without side effects.
3. **Given** the user has closed a dialog, **When** focus returns, **Then** it lands on the element that opened the dialog.
4. **Given** Narrator is enabled, **When** scan progress changes or an error appears, **Then** a brief meaningful message is announced without spamming on every frame.
5. **Given** a status is conveyed by color only, **When** the user cannot distinguish colors, **Then** the status is additionally conveyed by text, an icon or a shape.

---

### User Story 9 — Full RU/EN localization (Priority: P3)

The user switches the interface language between Russian and English. All 11 screens, dialogs, errors, tooltips and notifications are translated; switching does not reset state and does not require a restart.

**Why this priority**: localization already exists in the product and must be preserved. It is important for acceptance but does not block the main technical scenarios.

**Independent Test**: switch the language on every screen, verify there are no untranslated strings, that filters, selection and progress are preserved, and that there is no Cyrillic in the English catalog.

**Acceptance Scenarios**:

1. **Given** the interface is open in Russian, **When** the user switches the language to English, **Then** all visible strings change without a restart and without losing the state of the screen.
2. **Given** a scan or an operation is running, **When** the language changes, **Then** progress and already shown results are preserved, and new messages appear in the selected language.
3. **Given** a key is missing from the translation catalog, **When** the string is rendered, **Then** the English fallback is used and the missing key is recorded by a test, rather than an empty space being displayed.
4. **Given** the English interface, **When** the build is checked, **Then** there is no Cyrillic in the English resources, and message identifiers match the Russian ones.

---

### User Story 10 — Visual regression and automated checks (Priority: P3)

The team sees a gallery of screenshots of all 11 screens in both themes, at different DPI and in both languages. Any change that breaks layout, contrast or behavior is detected before release by automated tests.

**Why this priority**: 11 screens and two themes cannot be verified manually on every change. Without automated protection, the redesign degrades after the first iterations.

**Independent Test**: change a single token or spacing value and confirm that the visual test fails; restore the value and confirm that the test passes; open the gallery and verify its composition.

**Acceptance Scenarios**:

1. **Given** the frontend is built, **When** the visual tests run, **Then** there is a baseline for each screen, theme and DPI, and differences beyond the tolerance cause a failure.
2. **Given** a new screen is added, **When** gallery generation runs, **Then** it appears in the set, and without a baseline the test is not considered passed.
3. **Given** a hard-coded color appears in a component, **When** the token check runs, **Then** the build or the test fails, indicating the file.
4. **Given** a string without a localization key appears in the interface, **When** the check runs, **Then** the test fails, indicating the location.
5. **Given** the Playwright report is open, **When** a test fails, **Then** a screenshot, a diff and a trace are available for reproduction.

---

### User Story 11 — Single delivery and offline packaging (Priority: P3)

Once parity is reached, the new client remains the only FreeSpace graphical interface. The old egui interface is removed as part of this same feature, the installer and the portable build work without a network, the `data` directory is preserved, and the CLI and the core do not change their contract.

**Why this priority**: this is the final step, which must not be taken before parity. It completes the redesign and removes the old UI without creating a second one.

**Independent Test**: build the installer and the portable build on a clean Windows machine without a network, install, launch, run the smoke scenarios, uninstall the application and verify that no `%APPDATA%` entries or leftover files remain.

**Acceptance Scenarios**:

1. **Given** all 11 screens are accepted, **When** the interface replacement is completed, **Then** the old egui interface and its dependencies are removed from the workspace and the delivery, and the new client keeps the executable file name and shortcut compatibility.
2. **Given** a clean machine without a network and without WebView2, **When** the user runs the installer, **Then** installation completes successfully, the application starts and performs no network requests.
3. **Given** the installed application, **When** the user works with it, **Then** settings, the journal, quarantine, cache and WebView2 data reside only in the `data` directory next to the executable.
4. **Given** the application is being uninstalled, **When** the uninstaller finishes, **Then** the application files, the `data` directory and the context-menu entries are removed, and user files outside the application area are untouched.
5. **Given** a new build, **When** the parity checks run, **Then** every capability of the 11 screens has a CLI equivalent, and the existing JSON schemas and return codes are unchanged.

---

### Edge Cases

- **WebView2 is missing or outdated.** The installer must install a compatible version offline; if that is impossible, it must show a clear error and must not leave a partially installed application.
- **No write permission for the `data` directory.** The interface reports an error, works with settings in memory and does not fall back to `%APPDATA%`, `%LOCALAPPDATA%` or `%ProgramData%`.
- **WebView2 crashed or reloaded the page.** The application restores the shell, opens the Overview screen, does not lose unfinished operations and offers to repeat the safe action.
- **Scanning 10 million nodes.** The tree does not load the whole set into presentation memory; when the memory budget is exceeded, the interface reports the limitation and stays operational.
- **Empty volume, empty directory, zero size.** All screens show a meaningful empty state rather than an empty table or an endless spinner.
- **Access error for part of the tree.** Inaccessible objects are marked and do not distort the totals; the interface does not block the rest of the data.
- **Long path or Unicode name.** The path is displayed with wrapping and a tooltip, is copied in full and does not break the table.
- **DPI or monitor change during operation.** The window and virtualized lists are recalculated without losing position and selection.
- **Language or theme change during scanning.** Progress, filters and selection are preserved; only labels and the palette are updated.
- **Cancelling an operation midway.** Reversible steps already performed are rolled back or moved to quarantine; the journal records the actual result rather than the intention.
- **Two destructive operations attempted at the same time.** The second operation is blocked or queued with a clear message; data is not changed concurrently.
- **System or protected path.** Elevation and additional confirmation are required; a refusal leaves no partial changes.
- **Snapshots of incompatible versions or of different volumes.** The comparison ends with a clear error without changing files.
- **Corrupted or password-protected archive.** Previewing does not extract the file to disk and does not cause a hang.
- **Screen reader under high load.** Progress announcements are throttled so that they do not interrupt the user's speech.
- **The system shuts down during an operation.** On the next launch the journal shows the unfinished operation, and quarantine and data are not considered corrupted.
- **Portable build next to an installed one.** The two instances do not share a single `data` directory and do not conflict over the journal and quarantine.
- **Attempting to open an external link.** The application performs no network requests and shows local help or a clear limitation.

## Requirements *(mandatory)*

### Functional Requirements

#### Architecture and data ownership

- **FR-001**: The interface MUST be built on the agreed stack of Tauri 2, React 18, TypeScript and Vite; replacing the stack MUST require a new ADR and a repeated Constitution Check.
- **FR-002**: The presentation layer MUST NOT read, modify or delete files directly and MUST NOT call WinAPI; the sole source of data and operations MUST be the native application layer.
- **FR-003**: All logic for scanning, size accounting, analysis, operations, permissions, long paths, the journal and quarantine MUST remain in the existing Rust crates; duplicating this logic in TypeScript MUST be prohibited.
- **FR-004**: Application services shared by the old and the new interface MUST be extracted into a separate layer, so that the 11 screens do not copy the logic of the egui views.
- **FR-005**: The interface MUST obtain data only through typed commands and events with explicit input and output schemas.
- **FR-006**: Every command MUST validate its input, including paths, identifiers, page sizes and enumerations, before accessing the file system.
- **FR-007**: Native layer errors MUST be returned to the interface with a stable code, a category, a clear message and a flag indicating whether the action can be retried.
- **FR-008**: Long-running tasks MUST have an identifier, a state, progress and cancellation support; the interface MUST distinguish active, completed, cancelled and failed tasks.
- **FR-009**: The application MUST remain operational when an individual service or screen is unavailable; a crash of one screen MUST NOT destroy the shell and navigation.
- **FR-010**: Presentation state MUST be stored separately from domain data and MUST NOT substitute for the source of truth for sizes, operations and the journal.
- **FR-011**: All user preferences of the new interface MUST be persisted through the native layer in the `data` directory next to the executable; `localStorage`, `IndexedDB` and the WebView2 profile outside `data` MUST NOT be used as settings storage.
- **FR-012**: The application MUST ship as a single graphical client; an "old/new interface" switcher, parallel support of two UIs and a beta flag MUST NOT be implemented.

#### Shell and navigation

- **FR-013**: The interface MUST have a single shell: side navigation across the 11 screens in 6 groups, a top bar of contextual actions, a status bar, a notification area and a dialog layer.
- **FR-014**: The shell MUST show the current volume, the scanning state, active background tasks and the availability of cancellation at any moment.
- **FR-015**: Navigation MUST preserve the state of every screen when switching: filters, sorting, selection, scroll position and open panels.
- **FR-016**: The application MUST open the Overview screen on every launch, regardless of the stored `last_screen` value.
- **FR-017**: The shell MUST use the native Windows window title bar with correct drag, minimize, maximize and close behavior; a custom content header MAY exist inside the application, but the system window buttons MUST remain native.
- **FR-018**: Closing the window during a scan or an operation MUST require an explicit decision: minimize to background, cancel the task, or exit the application with a warning.
- **FR-019**: The application MUST support a single instance per `data` directory; a repeated launch MUST activate the existing window and pass it the requested screen or path.
- **FR-020**: The shell MUST provide a global search over the available commands and screens, invoked from the keyboard.
- **FR-021**: Notifications MUST distinguish success, information, warning and error, MUST NOT overlap a critical dialog, and MUST auto-dismiss only for non-destructive messages.
- **FR-022**: All 11 screens MUST open from the side navigation, from the global search and through a direct launch command, using a clear empty state if the screen requires context.

#### Screens and parity

- **FR-023**: The new interface MUST contain exactly 11 screens in 6 sidebar groups: "Overview" (Overview = Disks + inline scanning), "Analysis" (Tree with the hover "Size details" card, Analysis = Largest files + Types + Owners, Duplicates), "Cleanup" (Cleanup + Snapshots), "Operations" (Journal, Shredder, Write monitoring), "Tools" (Inspect = Metadata + Archives + Unlock + Long paths, File operations = Rename + Permissions), "System" (Settings + Elevation + About). Absorbed screens MUST remain tabs or sections of their screens, exporting a report MUST be a global action rather than a screen, and a separate heatmap (Treemap) screen MUST NOT exist.
- **FR-024**: Every screen MUST implement the full set of capabilities of the corresponding screen of the old interface and of the corresponding CLI contract; losing a function during the migration MUST block acceptance.
- **FR-025**: Every screen MUST have loading, empty result, error, partial result, long-running operation and cancellation states; an empty screen MUST explain which action needs to be performed.
- **FR-026**: Every screen MUST support data refresh without a full application reload and without losing the current context.
- **FR-027**: The "Overview" screen MUST show a summary of all available volumes, usage dynamics by snapshots and a quick start of the analysis.
- **FR-028**: The "Overview" screen MUST show the inline scan progress: the scanning method, the current path, speed, the number of objects, access errors and the ability to cancel.
- **FR-029**: The "Tree" screen MUST support hierarchy, sorting, search, filters, node expansion, selection and a row-details card shown on hover or keyboard focus.
- **FR-030**: The "Size details" card MUST open when the pointer hovers a tree row or keyboard focus lands on it, after a short delay, show the full details of that node and disappear when the pointer or focus leaves the row.
- **FR-031**: Analysis and cleanup screens MUST use the shared table, filter, card, chart and bulk action components, rather than separate implementations on each screen.
- **FR-032**: File operation screens MUST show the full path, the operation type, permission requirements, the preview and the result; long paths MUST be displayed and copied without truncation.
- **FR-033**: Inspection and report screens MUST be read-only, except for explicitly marked actions, and MUST NOT change data on simple viewing.
- **FR-034**: Operation and settings screens MUST save changes immediately or show an explicit save button; unsaved changes MUST NOT be lost when switching screens without a warning.

#### Tree: virtualized list, inline shares and the details card

- **FR-035**: The "Tree" screen MUST display the hierarchy of volumes, directories and files as a virtualized list of rows, where each node has an inline progress bar — a transparent fill whose share is proportional to the node's size relative to its parent.
- **FR-036**: The progress bar MUST span the full row height and start immediately after the expand chevron, MUST NOT overlap the name, size and other row values, and the fill MUST be semi-transparent; in both the dark and the light theme the fill MUST remain distinguishable without losing label contrast.
- **FR-037**: The "Size details" card MUST show, for the node under the pointer or focus, the logical, allocated and physical size, the hard link contribution, ADS, compression and sparse savings with an explicitly labeled size type, as well as the name, full path, share of the parent, number of objects and files and object type; the card MUST be dense (10–12 px type, two columns) and MUST NOT capture the pointer from the row.
- **FR-038**: The tree MUST use distinct icons for different object types (directory, open directory, text file, image, video, archive, volume, file), and the open-directory icon MUST reflect the expansion state.
- **FR-039**: Expanding and collapsing a node MUST work without re-reading the entire volume; the current path MUST offer breadcrumbs and a return to the previous level that preserve scroll position and selection.
- **FR-040**: The tree MUST support search, filtering and sorting performed by the native layer, keeping the totals consistent with the displayed rows.
- **FR-041**: The tree MUST be fully operable from the keyboard: moving across rows and levels, expanding and collapsing a node, Home/End, navigation by the first letter of a name, reaching the details card by focusing a row, and context actions with a visible focus indicator (open in Explorer, copy path, rename, delete to the Recycle Bin with confirmation).
- **FR-042**: The tree MUST load nodes page by page and aggregate small objects, without creating unreadable rows below the display threshold and without passing the whole node set to the view in a single response.
- **FR-043**: The tree MUST preserve readability and responsiveness at 10 million nodes and MUST work correctly with empty directories, zero-size objects, inaccessible nodes and very long names and paths.

#### Tables, trees, filters and bulk actions

- **FR-044**: Tables MUST be virtualized and request data in pages; the full set of rows MUST NOT be passed to the view in a single response.
- **FR-045**: Sorting, filtering and search MUST be performed by the native layer and MUST keep the final totals consistent with the displayed rows.
- **FR-046**: The user MUST be able to configure column visibility, order and width; the settings MUST persist between launches.
- **FR-047**: Selection MUST support single, range and multiple selection, "select all in the current filter" and clearing the selection.
- **FR-048**: Bulk actions MUST show the count and the total volume of the selected objects before execution and MUST use the common operation preview.
- **FR-049**: Tables MUST correctly display long paths, Unicode, lack of access, zero sizes and very large numbers with localized formatting.
- **FR-050**: Copying MUST support copying a cell value, a row and the full path without losing Unicode.
- **FR-051**: Filters MUST support saving sets, resetting, combining conditions and explicitly displaying active conditions.
- **FR-052**: Tables and trees MUST preserve scroll position and selection when data is refreshed, if the object still exists.
- **FR-053**: Empty, error and partial results MUST be displayed inside the table without replacing the entire shell.

#### Safe operations

- **FR-054**: Any operation that modifies or deletes data MUST begin with a preview of the list of objects, the total volume, the deletion method and a warning about irreversibility.
- **FR-055**: Confirmation MUST be explicit; the Enter key by default MUST NOT trigger an irreversible action without separate focus on the confirmation button.
- **FR-056**: The choice of deletion method MUST offer the Windows Recycle Bin, application quarantine and permanent deletion; permanent deletion MUST require additional confirmation.
- **FR-057**: The preview MUST show permission requirements, system and protected paths, as well as warnings for SSDs and large volumes.
- **FR-058**: All operations MUST be journaled with the time, user, type, objects, parameters and actual result.
- **FR-059**: Undo MUST be available where the operation is reversible and MUST explicitly explain why it is unavailable in other cases.
- **FR-060**: Restoration from quarantine and the Recycle Bin MUST be available from the journal and MUST resolve name conflicts before applying.
- **FR-061**: At most one destructive operation MUST run on a single volume at a time; the others MUST be blocked or queued with a clear message.
- **FR-062**: The interface MUST NOT consider an operation successful until the result is confirmed by the native layer; partial success MUST be displayed as partial, not as complete.

#### Themes and design system

- **FR-063**: The interface MUST use a single set of design tokens for color, typography, spacing, radii, shadows, borders and motion; hard-coded colors in product components MUST be prohibited.
- **FR-064**: The application MUST support the dark, light and system theme; on the first launch the system theme MUST be used by default.
- **FR-065**: Both themes MUST use the same components and tokens, rather than separate design variants.
- **FR-066**: The theme MUST be applied before the first rendering of content, so that no flash of the wrong theme occurs.
- **FR-067**: A change of the system theme MUST be applied without restarting the application.
- **FR-068**: The contrast of text and interactive elements MUST comply with WCAG 2.2 AA in both themes.
- **FR-069**: The product UI kit MUST include the shell, navigation, headings, buttons, fields, selects, segments, tabs, switches, checkboxes, radio buttons, chips, badges, tooltips, progress, tables, the tree with inline progress bars, lists, dialogs, notifications, charts and empty/loading/error states.
- **FR-070**: Components MUST have documented states: normal, hover, focus, pressed, selected, disabled, loading and error.
- **FR-071**: The illustrative Stageshow kit from `web/` MUST NOT enter the product build; `web/` is used only as a visual reference for tokens and language.

#### Accessibility

- **FR-072**: All interactive elements MUST be accessible from the keyboard and MUST have a visible focus indicator.
- **FR-073**: Focus order MUST match the visual order; dialogs MUST trap focus and return it to the initiator.
- **FR-074**: All elements MUST have an accessible name, role and state; decorative elements MUST be hidden from the screen reader.
- **FR-075**: Statuses MUST be conveyed not only by color, but also by text, an icon or a shape.
- **FR-076**: Progress, completion, errors and warnings MUST be announced to the screen reader without excessive repetition.
- **FR-077**: The user MUST be able to perform the main scenarios without a mouse: navigation, search, selection, opening a dialog, confirmation, cancellation and switching the theme or language.
- **FR-078**: The application MUST support the system contrast and text size settings without losing functionality.
- **FR-079**: Automated accessibility checks MUST run in CI and MUST block the release on critical violations.
- **FR-080**: Manual verification with Narrator MUST be part of the acceptance of the key screens and scenarios.

#### DPI and motion

- **FR-081**: The interface MUST work correctly at Windows scaling of 100, 125, 150 and 200%.
- **FR-082**: When the DPI or the monitor changes, the window, tables, tree and dialogs MUST be recalculated without losing position and selection.
- **FR-083**: Animations MUST be functional and short: hover about 120 ms, panels about 180 ms, dialogs about 240 ms.
- **FR-084**: When the system reduce-motion setting is enabled, optional transitions MUST be disabled, but a clear state change MUST be preserved.
- **FR-085**: The interface MUST NOT block input during an animation and MUST NOT play animations that hide a critical warning.

#### Localization

- **FR-086**: All visible strings MUST take their text from the localization catalog; hard-coded user-facing strings in components MUST be prohibited.
- **FR-087**: The interface MUST support Russian and English with an identical set of keys.
- **FR-088**: Switching the language MUST apply changes without a restart and without losing the state of the screen, filters, selection and progress.
- **FR-089**: Errors, codes, operation names, units of measurement and number and date formats MUST be localized consistently.
- **FR-090**: When a key is missing, the English fallback MUST be used, and a test MUST record the missing key; empty strings MUST NOT reach the interface.

#### Performance and data transfer

- **FR-091**: The interface response to user actions MUST be no more than 200 ms in typical scenarios, including scrolling, selection and screen switching.
- **FR-092**: Tree rendering MUST be no lower than 30 frames per second on the 10-million-node control set.
- **FR-093**: The total memory consumption of the application, including the native layer and the view, MUST be no more than 4 GB for 10 million nodes.
- **FR-094**: The full set of nodes MUST NOT be serialized and passed to the view in its entirety; the tree and tables MUST use batched or aggregated transfer.
- **FR-095**: Progress events MUST be throttled so as not to overload the channel or cause unnecessary repaints.
- **FR-096**: Long-running tasks MUST perform their work off the interface thread and MUST NOT freeze the shell.
- **FR-097**: The first interactive screen MUST appear in no more than 3 seconds on the reference machine, and the first screen with data in no more than 5 seconds after the data is ready.
- **FR-098**: When the memory or time budget is exceeded, the interface MUST report the limitation and suggest narrowing the scope, rather than crashing.
- **FR-099**: Every screen MUST release unused data when leaving it, keeping only the necessary presentation state.

#### Security and offline operation

- **FR-100**: The application MUST work fully without a network; network requests, telemetry and loading of external resources MUST be absent.
- **FR-101**: The content security policy MUST allow only local application resources and MUST NOT allow remote scripts, styles, fonts or images.
- **FR-102**: The capabilities of the new client MUST be minimal: only the commands and events required by the 11 screens; broad file system, shell and HTTP permissions MUST be prohibited.
- **FR-103**: All paths MUST be canonicalized and checked for admissibility before being passed to operations; escaping the area explicitly selected by the user MUST be prohibited.
- **FR-104**: Developer tools MUST be disabled in the release build.
- **FR-105**: Logs MUST be stored locally in the `data` directory and MUST NOT contain the content of user files without necessity.
- **FR-106**: WebView2 data MUST be placed in the `data` directory next to the executable, so that no profile is created in `%LOCALAPPDATA%`.
- **FR-107**: The new interface MUST preserve the system path protection and the elevation requirement from the existing operations layer.

#### Packaging and installation

- **FR-108**: The delivery MUST include an installer and a portable build; both MUST work without a network.
- **FR-109**: The installer MUST provide a compatible WebView2 version offline and MUST verify it before the first launch.
- **FR-110**: The portable build MUST contain all required components and MUST NOT require installation into the system.
- **FR-111**: The executable file name `freespace-gui.exe` MUST be preserved after the interface replacement, for compatibility with shortcuts, scripts and the context menu.
- **FR-112**: The installer MUST preserve the existing scenarios for update, uninstall, removal of `data` and cleanup of context-menu entries.
- **FR-113**: The installer size and installation time MUST be measured and recorded; the decision on how to deliver WebView2 MUST be justified in research and an ADR.
- **FR-114**: Code signing MUST be verified in the release scenario; if a certificate is unavailable, this MUST be explicitly recorded as a release limitation.

#### Testing and quality

- **FR-115**: Rust tests of the core, the CLI and the operations MUST remain green; the CLI contract MUST be protected by reference JSON checks.
- **FR-116**: New commands, validation, serialization and settings migration MUST be covered by Rust tests.
- **FR-117**: Components and screens MUST be covered by Vitest and React Testing Library tests for loading, empty/error states, keyboard and localization.
- **FR-118**: Key user scenarios MUST be covered by Playwright tests on the Vite build and by Tauri E2E tests on Windows.
- **FR-119**: Each of the 11 screens MUST have reference screenshots in both themes, at 100/125/150/200% DPI and in both languages.
- **FR-120**: CI MUST run `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `npm ci`, lint, types, unit tests, visual tests and a check for the absence of network calls.
- **FR-121**: Test fixtures MUST use only temporary directories and test VHDs; real user data MUST NOT be touched.
- **FR-122**: Administrative and destructive tests MUST run only with `FREESPACE_ADMIN_TESTS=1` on an isolated test volume.

#### Cutover and backward compatibility

- **FR-123**: The release MUST happen only after acceptance of 11 out of 11 screens and parity of capabilities with the CLI.
- **FR-124**: The old egui interface and its dependencies MUST be removed from the workspace and the delivery as part of this feature; a second UI MUST NOT be built, shipped or maintained.
- **FR-125**: The core, the CLI, report formats, the snapshot format and return codes MUST NOT change as part of the redesign.
- **FR-126**: Existing settings, the journal, quarantine and snapshots MUST be readable by the new interface; the migration MUST be one-time, reversible at the backup level and MUST NOT lose data.
- **FR-127**: After the interface replacement, it MUST be possible to roll back to the previous version of the application without losing user data.
- **FR-128**: The documentation MUST describe the new interface, hotkeys, themes, DPI, localization, installation and the differences from the old UI.
- **FR-129**: All 11 screens MUST have a parity matrix "screen — capability — CLI command — test — screenshot", filled in before acceptance; the absorbed screens remain rows of the matrix.
- **FR-130**: The release MUST include a report on performance, accessibility, the offline check and the results of visual regression.

### Key Entities *(including data, if applicable)*

- **Screen**: identifier, name, sidebar group, data dependencies, available actions, states and permissions.
- **ScreenState**: loading, empty, error, ready, partial result, selection, filters, sorting, page and scroll position.
- **DesignToken**: name, category, value for the dark and light theme, purpose and application constraints.
- **Theme**: dark, light or system; a set of tokens, the chart palette, contrast.
- **Message**: key, Russian text, English text, context, pluralization and translation status.
- **IpcCommand**: name, input schema, output schema, errors, permission requirements and a long-running flag.
- **IpcEvent**: name, payload, frequency, subscriber and cancellation rule.
- **TreePage**: request, cursor, rows, total count, continuation flag and size metric.
- **NodeRow**: identifier, name, full path, type, sizes, dates, attributes, accessibility and the number of descendants.
- **NodeProgress**: node identifier, share of the parent, share of the volume, visibility and theme state.
- **OperationPreview**: objects, total volume, deletion method, risks, permission requirements, reversibility and retention period.
- **OperationProgress**: phase, processed, total, current object, speed, errors and cancellability.
- **UiSettings**: theme, language, density, units, reduced motion, window size and position, column visibility; `last_screen` is kept only for compatibility and does not affect startup.
- **Toast**: importance, message key, action, lifetime and a mandatory flag.
- **UiError**: code, category, message key, details, retryability and the link to the journal.
- **ScreenshotBaseline**: screen, theme, DPI, language, image, tolerance and update date.
- **ParityMatrix**: screen, capability, CLI command, test, screenshot and acceptance status.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 11 out of 11 screens are implemented on the product UI kit, open from the shell and pass the acceptance scenarios.
- **SC-002**: No screen or component performs network requests; the network guard check records 0 calls during the entire E2E run.
- **SC-003**: 95% of user actions (scrolling, selection, filtering, screen switching) receive a response in no more than 200 ms on the control set.
- **SC-004**: Scrolling the virtualized tree at 10 million nodes renders at no less than 30 frames per second, and node expansion and level switching take no more than 300 ms.
- **SC-005**: The total application memory for 10 million nodes does not exceed 4 GB, including the native layer and WebView2.
- **SC-006**: 100% of text and interactive elements pass AA contrast in both themes; the automated check finds no critical violations.
- **SC-007**: Both themes and the system mode are available on all 11 screens; a flash of the wrong theme at startup occurs in 0% of launches.
- **SC-008**: All 11 screens (11 × 4 DPI = 44 checks) display correctly at 100, 125, 150 and 200% DPI without clipping or overlapping of elements.
- **SC-009**: Ten key scenarios are fully performed with the keyboard alone, including confirmation and cancellation of destructive operations.
- **SC-010**: Narrator correctly announces headings, states, progress and errors in the key scenarios; manual verification finds no blocking defects.
- **SC-011**: 100% of visible strings are localized; switching RU/EN loses no state and leaves no untranslated strings.
- **SC-012**: 0 hard-coded colors in product components and 0 user-facing strings outside the localization catalog.
- **SC-013**: 100% of destructive operations go through a preview, explicit confirmation and a journal record; undo is available everywhere the operation is reversible.
- **SC-014**: The existing CLI contract is unchanged: reference JSON responses and return codes match before and after the redesign.
- **SC-015**: Installation, the first launch, operation and uninstallation pass on a clean Windows machine without a network; 0 entries are created in `%APPDATA%`, `%LOCALAPPDATA%` and `%ProgramData%`.
- **SC-016**: A reference screenshot exists for each of the 11 screens, theme, DPI and language — a full matrix of 11 × 2 × 4 × 2 = 176 baselines; the visual tests are green, and a deliberate token change causes a failure.
- **SC-017**: Rust, unit, component, E2E and visual tests are green in CI on Windows; critical accessibility and security defects block the build.
- **SC-018**: 100% of existing settings, the journal, quarantine and snapshots are readable by the new interface; the migration loses no data.
- **SC-019**: The first interactive screen appears in no more than 3 seconds, and the first screen with data in no more than 5 seconds after the data is ready on the reference machine.
- **SC-020**: The installer and the portable build work offline; size and installation time are measured, recorded and within the agreed budget.
- **SC-021**: After the replacement, the old egui code is absent from the workspace and the delivery, exactly one graphical client is built and shipped, and all 11 screens have a filled-in parity matrix.
- **SC-022**: 0 regressions in the tests of the core, the CLI and the operations; all existing user capabilities are preserved.

## Assumptions

- The technology stack is fixed by `specs/002-native-ui-redesign/plan.md` and ADR `docs/adr/0001-tauri-react-gui.md`: Tauri 2, React 19.3, TypeScript, Vite.
- `web/` is a visual reference for the new language, not the product frontend. The GitHub Pages project and the mockup itself are not part of the scope of 002.
- The old egui interface was never released and is fully replaced by the new client. The `apps/freespace-gui` directory and the egui dependencies are removed; a second UI is not built, shipped or maintained.
- The 11 screens include "About" and "Elevation" as sections of the "Settings" screen, and "Snapshots" as a tab of the "Cleanup" screen; the operation confirmation dialog is a shared component, not a separate screen.
- Data exploration happens only through the "Tree" screen with inline progress bars; the heatmap (Treemap), its Canvas rendering, hit-testing, legend and zoom are excluded from the product, while the `tree` CLI contract stays unchanged.
- The existing core, CLI, operations, reports and snapshot format do not change in meaning; the redesign adds no new analytical capabilities.
- All data and settings are stored locally in the `data` directory next to the executable; `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` and temporary directories are not used.
- The target platforms are Windows 10 21H2+ and Windows 11 x64; cross-platform support is not part of v1.
- WebView2 is available in the system or is delivered offline; the delivery method is recorded in research and an ADR.
- The interface languages are Russian and English; other languages are not part of the scope of 002.
- The product UI kit is created from scratch on the tokens from `web/`; the illustrative Stageshow kit is not carried over into the product.
- Visual and E2E tests run on Windows CI with fixed fonts and tolerances.
- Administrative and destructive tests run only with `FREESPACE_ADMIN_TESTS=1` on isolated test volumes.
- Exactly one graphical interface remains in the workspace and in the release; parallel support of egui and Tauri is not created even temporarily.

## Dependencies

- `specs/001-freespace-disk-analyzer/` — the core, CLI, operations, reports, snapshots and data model must remain stable.
- `specs/002-native-ui-redesign/plan.md` — the adopted decisions on the stack, themes, screen groups and budgets.
- `web/` — the visual reference for tokens, typography, density and chart palettes.
- `apps/freespace-gui/` — the source code of the old interface is available in git history as a source of capability parity and user scenarios; it is removed after the migration.
- `locales/ru.toml`, `locales/en.toml` — the existing strings for migration and reconciliation.
- `installer/` — the existing installation, uninstallation and context-menu scenarios.
- Windows CI with Node.js, Rust and Playwright; a machine or agent with WebView2 support.
- The ADR `docs/adr/0001-tauri-react-gui.md` — a mandatory condition for starting the implementation.

## Out of Scope

- The GitHub Pages project and the marketing site.
- New analytical capabilities, new report formats and new file operations.
- Changes to the CLI contract, return codes and JSON schemas.
- Cross-platform support, mobile clients, a web version of the product.
- Network, cloud and multi-user scenarios.
- Localization into languages other than Russian and English.
- The heatmap (Treemap) and any other cartographic representation of data: hit-testing, legend, zoom and tile fills; data exploration happens only through the tree.
- Replacing the scanning core, the operations layer or the snapshot format.
