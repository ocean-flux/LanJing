import {
  loadLibraryProjection,
  loadMediaItems,
  projectLibrary,
  updateLibraryEntry,
  type LibraryEntry,
  type LibraryEntryUpdateRequest,
  type LibraryProjectionResponse,
  type LibraryUpdateReceipt,
} from '@/shared/tauri/library';
import type { MediaItem } from '@/shared/types/media';

export interface LibraryWorkflowRow {
  entry: LibraryEntry;
  media: MediaItem | null;
}

export interface LibraryWorkflowAdapter {
  loadProjection: () => Promise<LibraryProjectionResponse>;
  loadMediaItems: (resourceIds: string[]) => Promise<MediaItem[]>;
  updateLibraryEntry: (request: LibraryEntryUpdateRequest) => Promise<LibraryUpdateReceipt>;
}

export type LibraryWorkflowState =
  | {
      kind: 'loading';
      rows: LibraryWorkflowRow[];
      pendingResourceIds: string[];
      receipts: Record<string, LibraryUpdateReceipt>;
      failures: Record<string, string>;
    }
  | {
      kind: 'ready';
      globalSeq: number;
      rows: LibraryWorkflowRow[];
      pendingResourceIds: string[];
      receipts: Record<string, LibraryUpdateReceipt>;
      failures: Record<string, string>;
    }
  | {
      kind: 'error';
      detail: string;
      rows: LibraryWorkflowRow[];
      pendingResourceIds: string[];
      receipts: Record<string, LibraryUpdateReceipt>;
      failures: Record<string, string>;
    };

export const tauriLibraryWorkflowAdapter: LibraryWorkflowAdapter = {
  loadProjection: loadLibraryProjection,
  loadMediaItems,
  updateLibraryEntry,
};

function detailOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function rowsFromProjection(
  response: LibraryProjectionResponse,
  mediaItems: MediaItem[],
): LibraryWorkflowRow[] {
  const mediaById = new Map(mediaItems.map((item) => [item.id, item]));
  return projectLibrary(response).map((entry) => ({
    entry,
    media: mediaById.get(entry.resource_id) ?? null,
  }));
}

function withoutResource(values: string[], resourceId: string): string[] {
  return values.filter((value) => value !== resourceId);
}

function isReady(
  state: LibraryWorkflowState,
): state is Extract<LibraryWorkflowState, { kind: 'ready' }> {
  return state.kind === 'ready';
}

export function createLibraryWorkflow(
  adapter: LibraryWorkflowAdapter = tauriLibraryWorkflowAdapter,
) {
  let state: LibraryWorkflowState = {
    kind: 'loading',
    rows: [],
    pendingResourceIds: [],
    receipts: {},
    failures: {},
  };
  const listeners = new Set<() => void>();

  const publish = (next: LibraryWorkflowState) => {
    state = next;
    listeners.forEach((listener) => listener());
  };

  const refresh = async () => {
    const previous = state;
    publish({
      kind: 'loading',
      rows: previous.rows,
      pendingResourceIds: previous.pendingResourceIds,
      receipts: previous.receipts,
      failures: previous.failures,
    });
    try {
      const response = await adapter.loadProjection();
      const mediaItems = await adapter.loadMediaItems(
        projectLibrary(response).map((entry) => entry.resource_id),
      );
      publish({
        kind: 'ready',
        globalSeq: response.global_seq,
        rows: rowsFromProjection(response, mediaItems),
        pendingResourceIds: state.pendingResourceIds,
        receipts: state.receipts,
        failures: state.failures,
      });
    } catch (error) {
      publish({
        kind: 'error',
        detail: detailOf(error),
        rows: previous.rows,
        pendingResourceIds: previous.pendingResourceIds,
        receipts: previous.receipts,
        failures: previous.failures,
      });
    }
  };

  const updateOwnership = async (resourceId: string, favorite: boolean, pinned: boolean) => {
    if (!isReady(state)) return;
    const row = state.rows.find((candidate) => candidate.entry.resource_id === resourceId);
    if (!row || state.pendingResourceIds.includes(resourceId)) return;

    const nextPending = [...state.pendingResourceIds, resourceId];
    const nextFailures = { ...state.failures };
    delete nextFailures[resourceId];
    publish({ ...state, pendingResourceIds: nextPending, failures: nextFailures });

    const request: LibraryEntryUpdateRequest = {
      resource_id: resourceId,
      favorite: favorite || pinned,
      pinned,
      last_opened_at: row.entry.last_opened_at,
      progress: row.entry.progress,
      expected_version: row.entry.revision,
    };

    try {
      const receipt = await adapter.updateLibraryEntry(request);
      const current = state;
      if (!isReady(current)) return;
      const updatedRows = current.rows.map((candidate) => {
        if (candidate.entry.resource_id !== resourceId) return candidate;
        return {
          ...candidate,
          entry: {
            ...candidate.entry,
            favorite: request.favorite,
            pinned: request.pinned,
            revision: receipt.revision,
            updated_global_seq: receipt.global_seq,
          },
        };
      });
      const updatedProjection: LibraryProjectionResponse = {
        global_seq: Math.max(current.globalSeq, receipt.global_seq),
        entries: updatedRows.map(({ entry }) => entry),
      };
      const mediaById = new Map(
        current.rows.map((candidate) => [candidate.entry.resource_id, candidate.media]),
      );
      publish({
        kind: 'ready',
        globalSeq: updatedProjection.global_seq,
        rows: projectLibrary(updatedProjection).map((entry) => ({
          entry,
          media: mediaById.get(entry.resource_id) ?? null,
        })),
        pendingResourceIds: withoutResource(current.pendingResourceIds, resourceId),
        receipts: { ...current.receipts, [resourceId]: receipt },
        failures: current.failures,
      });
    } catch (error) {
      const current = state;
      publish({
        kind: 'ready',
        globalSeq: isReady(current) ? current.globalSeq : 0,
        rows: current.rows,
        pendingResourceIds: withoutResource(current.pendingResourceIds, resourceId),
        receipts: current.receipts,
        failures: { ...current.failures, [resourceId]: detailOf(error) },
      });
    }
  };

  return {
    getState: () => state,
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    refresh,
    updateOwnership,
  };
}
