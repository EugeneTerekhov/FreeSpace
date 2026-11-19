import type {
  AboutInfo,
  AppBootstrap,
  AppCapabilities,
  CleanupCandidateView,
  DuplicateGroupPage,
  DuplicateGroupView,
  DuplicateStartRequest,
  DuplicateTask,
  ElevationStatus,
  FilterSet,
  JournalEntryView,
  JournalFilter,
  JournalPage,
  Metric,
  NavigationRequest,
  NodeRow,
  OperationAccepted,
  OperationPreview,
  OperationPreviewRequest,
  OperationProgress,
  ScanRequest,
  ScanSessionSummary,
  ScanStarted,
  SnapshotCreateRequest,
  SnapshotCreated,
  SnapshotDiffPage,
  SnapshotDiffSort,
  SnapshotSummary,
  SortSpec,
  Task,
  TaskStarted,
  TreePage,
  TypeSummary,
  UiSettings,
  UiSettingsPatch,
  VolumeDetail,
  VolumeSummary,
  WindowState,
} from './dto';
import type { Transport } from './transport';

/** Typed wrapper around every command of the IPC contract. */
export class ApiClient {
  constructor(readonly transport: Transport) {}

  appBootstrap(): Promise<AppBootstrap> {
    return this.transport.invoke<AppBootstrap>('app_bootstrap');
  }

  appCapabilities(): Promise<AppCapabilities> {
    return this.transport.invoke<AppCapabilities>('app_capabilities');
  }

  appAboutInfo(): Promise<AboutInfo> {
    return this.transport.invoke<AboutInfo>('app_about_info');
  }

  appSettingsGet(): Promise<UiSettings> {
    return this.transport.invoke<UiSettings>('app_settings_get');
  }

  appSettingsSet(patch: UiSettingsPatch): Promise<UiSettings> {
    return this.transport.invoke<UiSettings>('app_settings_set', { patch });
  }

  appWindowStateGet(): Promise<WindowState> {
    return this.transport.invoke<WindowState>('app_window_state_get');
  }

  appWindowStateSet(windowState: WindowState): Promise<void> {
    return this.transport.invoke<void>('app_window_state_set', {
      window_state: windowState,
    });
  }

  appNavigationRequest(request: NavigationRequest): Promise<void> {
    return this.transport.invoke<void>('app_navigation_request', { request });
  }

  appElevationStatus(): Promise<ElevationStatus> {
    return this.transport.invoke<ElevationStatus>('app_elevation_status');
  }

  volumesList(): Promise<VolumeSummary[]> {
    return this.transport.invoke<VolumeSummary[]>('volumes_list');
  }

  volumesRefresh(): Promise<VolumeSummary[]> {
    return this.transport.invoke<VolumeSummary[]>('volumes_refresh');
  }

  volumesSummary(volumeId: string): Promise<VolumeDetail> {
    return this.transport.invoke<VolumeDetail>('volumes_summary', {
      volume_id: volumeId,
    });
  }

  volumesCurrent(): Promise<VolumeSummary> {
    return this.transport.invoke<VolumeSummary>('volumes_current');
  }

  scanStart(request: ScanRequest): Promise<ScanStarted> {
    return this.transport.invoke<ScanStarted>('scan_start', {
      request,
    });
  }

  scanStatus(taskId: string): Promise<Task> {
    return this.transport.invoke<Task>('scan_status', { task_id: taskId });
  }

  scanCancel(taskId: string): Promise<void> {
    return this.transport.invoke<void>('scan_cancel', { task_id: taskId });
  }

  scanSessions(): Promise<ScanSessionSummary[]> {
    return this.transport.invoke<ScanSessionSummary[]>('scan_sessions');
  }

  treeRoot(
    generation: number,
    options: { metric?: Metric; sort?: SortSpec; pageSize?: number; cursor?: string | null | undefined } = {},
  ): Promise<TreePage> {
    return this.transport.invoke<TreePage>('tree_root', {
      generation,
      metric: options.metric,
      sort: options.sort,
      page_size: options.pageSize,
      cursor: options.cursor,
    });
  }

  treeChildren(
    generation: number,
    parentId: number,
    options: {
      metric?: Metric;
      sort?: SortSpec;
      filter?: FilterSet;
      pageSize?: number;
      cursor?: string | null | undefined;
    } = {},
  ): Promise<TreePage> {
    return this.transport.invoke<TreePage>('tree_children', {
      generation,
      parent_id: parentId,
      metric: options.metric,
      sort: options.sort,
      filter: options.filter,
      page_size: options.pageSize,
      cursor: options.cursor,
    });
  }

  treeSearch(
    generation: number,
    query: string,
    options: {
      metric?: Metric;
      sort?: SortSpec;
      filter?: FilterSet;
      pageSize?: number;
      cursor?: string | null | undefined;
    } = {},
  ): Promise<TreePage> {
    return this.transport.invoke<TreePage>('tree_search', {
      generation,
      query,
      metric: options.metric,
      sort: options.sort,
      filter: options.filter,
      page_size: options.pageSize,
      cursor: options.cursor,
    });
  }

  treeNode(generation: number, nodeId: number): Promise<NodeRow> {
    return this.transport.invoke<NodeRow>('tree_node', {
      generation,
      node_id: nodeId,
    });
  }

  treePath(generation: number, nodeId: number): Promise<NodeRow[]> {
    return this.transport.invoke<NodeRow[]>('tree_path', {
      generation,
      node_id: nodeId,
    });
  }

  topFilesPage(
    generation: number,
    options: {
      limit?: number;
      metric?: Metric;
      sort?: SortSpec;
      filter?: FilterSet;
      cursor?: string | null | undefined;
      pageSize?: number;
    } = {},
  ): Promise<TreePage> {
    return this.transport.invoke<TreePage>('top_files_page', {
      generation,
      limit: options.limit,
      metric: options.metric,
      sort: options.sort,
      filter: options.filter,
      cursor: options.cursor,
      page_size: options.pageSize,
    });
  }

  typesSummary(
    generation: number,
    filter?: FilterSet,
    groupBy: 'extension' | 'category' = 'extension',
  ): Promise<TypeSummary[]> {
    return this.transport.invoke<TypeSummary[]>('types_summary', {
      generation,
      filter,
      group_by: groupBy,
    });
  }

  typesFilesPage(
    generation: number,
    fileType: string,
    options: {
      metric?: Metric;
      sort?: SortSpec;
      filter?: FilterSet;
      cursor?: string | null | undefined;
      pageSize?: number;
    } = {},
  ): Promise<TreePage> {
    return this.transport.invoke<TreePage>('types_files_page', {
      generation,
      file_type: fileType,
      metric: options.metric,
      sort: options.sort,
      filter: options.filter,
      cursor: options.cursor,
      page_size: options.pageSize,
    });
  }

  duplicatesStart(request: DuplicateStartRequest): Promise<TaskStarted> {
    return this.transport.invoke<TaskStarted>('duplicates_start', { request });
  }

  duplicatesStatus(taskId: string): Promise<DuplicateTask> {
    return this.transport.invoke<DuplicateTask>('duplicates_status', {
      task_id: taskId,
    });
  }

  duplicatesGroupsPage(
    taskId: string,
    options: { cursor?: string | null; pageSize?: number } = {},
  ): Promise<DuplicateGroupPage> {
    return this.transport.invoke<DuplicateGroupPage>('duplicates_groups_page', {
      task_id: taskId,
      cursor: options.cursor,
      page_size: options.pageSize,
    });
  }

  duplicatesGroup(taskId: string, groupId: string): Promise<DuplicateGroupView> {
    return this.transport.invoke<DuplicateGroupView>('duplicates_group', {
      task_id: taskId,
      group_id: groupId,
    });
  }

  duplicatesCancel(taskId: string): Promise<void> {
    return this.transport.invoke<void>('duplicates_cancel', { task_id: taskId });
  }

  cleanupCandidates(
    generation: number,
    olderThanDays: number,
    filter?: FilterSet,
  ): Promise<CleanupCandidateView[]> {
    return this.transport.invoke<CleanupCandidateView[]>('cleanup_candidates', {
      generation,
      older_than_days: olderThanDays,
      filter,
    });
  }

  cleanupEmptyDirsPage(
    generation: number,
    options: { cursor?: string | null; pageSize?: number } = {},
  ): Promise<TreePage> {
    return this.transport.invoke<TreePage>('cleanup_empty_dirs_page', {      generation,
      cursor: options.cursor,
      page_size: options.pageSize,
    });
  }

  snapshotsList(volumeId?: string | null): Promise<SnapshotSummary[]> {
    return this.transport.invoke<SnapshotSummary[]>('snapshots_list', {
      volume_id: volumeId,
    });
  }

  snapshotsCreate(request: SnapshotCreateRequest): Promise<SnapshotCreated> {
    return this.transport.invoke<SnapshotCreated>('snapshots_create', { request });
  }

  snapshotsCompare(baseId: string, targetId: string): Promise<TaskStarted> {
    return this.transport.invoke<TaskStarted>('snapshots_compare', {
      base_id: baseId,
      target_id: targetId,
    });
  }

  snapshotsDiffPage(
    taskId: string,
    options: {
      cursor?: string | null;
      pageSize?: number;
      sort?: SnapshotDiffSort;
    } = {},
  ): Promise<SnapshotDiffPage> {
    return this.transport.invoke<SnapshotDiffPage>('snapshots_diff_page', {
      task_id: taskId,
      cursor: options.cursor,
      page_size: options.pageSize,
      sort: options.sort,
    });
  }

  snapshotsDelete(snapshotId: string): Promise<void> {
    return this.transport.invoke<void>('snapshots_delete', {
      snapshot_id: snapshotId,
    });
  }

  snapshotsStatus(taskId: string): Promise<Task> {
    return this.transport.invoke<Task>('snapshots_status', { task_id: taskId });
  }

  operationPreview(request: OperationPreviewRequest): Promise<OperationPreview> {
    return this.transport.invoke<OperationPreview>('operation_preview', { request });
  }

  operationConfirm(
    previewId: string,
    confirmationToken: string,
    deleteMode?: string | null,
    extraConfirmation = false,
  ): Promise<OperationAccepted> {
    return this.transport.invoke<OperationAccepted>('operation_confirm', {
      preview_id: previewId,
      confirmation_token: confirmationToken,
      delete_mode: deleteMode,
      extra_confirmation: extraConfirmation,
    });
  }

  operationStatus(operationId: string): Promise<OperationProgress> {
    return this.transport.invoke<OperationProgress>('operation_status', {
      operation_id: operationId,
    });
  }

  operationCancel(operationId: string): Promise<void> {
    return this.transport.invoke<void>('operation_cancel', {
      operation_id: operationId,
    });
  }

  revealInExplorer(path: string): Promise<void> {
    return this.transport.invoke<void>('reveal_in_explorer', { path });
  }

  operationUndo(operationId: string): Promise<OperationAccepted> {
    return this.transport.invoke<OperationAccepted>('operation_undo', {
      operation_id: operationId,
    });
  }

  operationRestore(quarantineIds: string[]): Promise<OperationAccepted> {
    return this.transport.invoke<OperationAccepted>('operation_restore', {
      quarantine_ids: quarantineIds,
    });
  }

  journalPage(
    options: { filter?: JournalFilter; cursor?: string | null; pageSize?: number } = {},
  ): Promise<JournalPage> {
    return this.transport.invoke<JournalPage>('journal_page', {
      filter: options.filter,
      cursor: options.cursor,
      page_size: options.pageSize,
    });
  }

  journalEntry(entryId: string): Promise<JournalEntryView> {
    return this.transport.invoke<JournalEntryView>('journal_entry', {
      entry_id: entryId,
    });
  }

  journalUndo(entryId: string): Promise<OperationAccepted> {
    return this.transport.invoke<OperationAccepted>('journal_undo', {
      entry_id: entryId,
    });
  }

  journalRestore(entryId: string): Promise<OperationAccepted> {
    return this.transport.invoke<OperationAccepted>('journal_restore', {
      entry_id: entryId,
    });
  }
}
