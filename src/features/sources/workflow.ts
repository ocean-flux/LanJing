import {
  CATALOG_INSTALL_CAP,
  parseBookSourceCatalog,
  type CatalogItem,
} from '@/shared/tauri/catalog';
import {
  installPreparedSource,
  listInstalledSources,
  prepareSourceInstall,
  type InstallCandidate,
  type InstalledSource,
  type SourcePrepareRequest,
} from '@/shared/tauri/sources';

export interface SourceWorkflowAdapter {
  listSources: () => Promise<InstalledSource[]>;
  prepare: (request: SourcePrepareRequest) => Promise<InstallCandidate>;
  install: (candidateId: string) => Promise<InstalledSource>;
}

export interface PreparedSource {
  item: CatalogItem;
  candidate: InstallCandidate;
  isUpdate: boolean;
  previous: InstalledSource | null;
}

export type SourceWorkflowPhase =
  | 'idle'
  | 'pick'
  | 'preparing'
  | 'confirm'
  | 'installing'
  | 'done'
  | 'error';

export interface SourceWorkflowState {
  phase: SourceWorkflowPhase;
  rawInput: string;
  fileName: string | null;
  catalog: CatalogItem[];
  selectedIds: string[];
  prepared: PreparedSource[];
  sources: InstalledSource[];
  errorCode: string | null;
  errorDetail: string | null;
  failedItems: string[];
  sourcesLoading: boolean;
  sourcesError: string | null;
}

export const tauriSourceWorkflowAdapter: SourceWorkflowAdapter = {
  listSources: listInstalledSources,
  prepare: prepareSourceInstall,
  install: installPreparedSource,
};

function errorCode(error: unknown): string {
  if (typeof error === 'object' && error !== null && 'code' in error) {
    const { code } = error as { code?: unknown };
    if (typeof code === 'string' && code.length > 0) return code;
  }
  return 'source_operation_failed';
}

function errorDetail(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const { message } = error as { message?: unknown };
    if (typeof message === 'string') return message;
  }
  return String(error);
}

function catalogErrorCode(reason: 'invalid-json' | 'not-book-source' | 'empty'): string {
  switch (reason) {
    case 'invalid-json': {
      return 'input_invalid_json';
    }
    case 'not-book-source': {
      return 'input_not_book_source';
    }
    case 'empty': {
      return 'input_empty';
    }
  }
}

function isHttpUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return url.protocol === 'http:' || url.protocol === 'https:';
  } catch {
    return false;
  }
}

function isRetryableCandidateError(code: string): boolean {
  return RETRYABLE_CANDIDATE_ERRORS.has(code);
}

const RETRYABLE_CANDIDATE_ERRORS = new Set([
  'candidate_stale',
  'candidate_expired',
  'candidate_consumed',
  'candidate_not_found',
  'source_revision_conflict',
  'grant_insufficient',
]);

function candidateItem(candidate: InstallCandidate): CatalogItem {
  return {
    id: `candidate:${candidate.id}`,
    name: candidate.profile.title,
    group: candidate.profile.group,
    rawJson: '',
  };
}

/// 更新与否以 candidate 声明的操作为准, 不再由前端比对已安装来源推导。
function isUpdate(candidate: InstallCandidate): boolean {
  return candidate.operation === 'update';
}

function previousSource(
  candidate: InstallCandidate,
  sources: InstalledSource[],
): InstalledSource | null {
  return sources.find((source) => source.source_id === candidate.profile.id) ?? null;
}

function initialState(): SourceWorkflowState {
  return {
    phase: 'idle',
    rawInput: '',
    fileName: null,
    catalog: [],
    selectedIds: [],
    prepared: [],
    sources: [],
    errorCode: null,
    errorDetail: null,
    failedItems: [],
    sourcesLoading: false,
    sourcesError: null,
  };
}

export function candidateRequestsSystem(candidate: InstallCandidate): boolean {
  const { env, fs, process } = candidate.required_grant;
  return env || fs || process;
}

export function availableSourceGroups(items: CatalogItem[]): string[] {
  return [
    ...new Set(items.map((item) => item.group).filter((group): group is string => Boolean(group))),
  ].sort((left, right) => left.localeCompare(right));
}

export function createSourceWorkflow(adapter: SourceWorkflowAdapter = tauriSourceWorkflowAdapter) {
  let state = initialState();
  const listeners = new Set<() => void>();

  const publish = (next: SourceWorkflowState) => {
    state = next;
    listeners.forEach((listener) => listener());
  };

  const setInput = (rawInput: string, fileName: string | null = null) => {
    publish({
      ...state,
      phase: 'idle',
      rawInput,
      fileName,
      catalog: [],
      selectedIds: [],
      prepared: [],
      errorCode: null,
      errorDetail: null,
      failedItems: [],
    });
  };

  const setError = (code: string, detail: string | null = null) => {
    publish({ ...state, phase: 'error', errorCode: code, errorDetail: detail });
  };

  const refreshSources = async () => {
    publish({ ...state, sourcesLoading: true, sourcesError: null });
    try {
      publish({ ...state, sources: await adapter.listSources(), sourcesLoading: false });
    } catch (error) {
      publish({
        ...state,
        sourcesLoading: false,
        sourcesError: errorDetail(error),
      });
    }
  };

  const prepareInput = async () => {
    const rawInput = state.rawInput.trim();
    if (!rawInput) {
      publish({ ...state, phase: 'error', errorCode: 'input_empty', errorDetail: null });
      return;
    }

    if (isHttpUrl(rawInput)) {
      publish({
        ...state,
        phase: 'preparing',
        prepared: [],
        errorCode: null,
        errorDetail: null,
      });
      try {
        const candidate = await adapter.prepare({ kind: 'maccms_json', url: rawInput });
        publish({
          ...state,
          phase: 'confirm',
          catalog: [candidateItem(candidate)],
          selectedIds: [candidateItem(candidate).id],
          prepared: [
            {
              item: candidateItem(candidate),
              candidate,
              isUpdate: isUpdate(candidate),
              previous: previousSource(candidate, state.sources),
            },
          ],
          errorCode: null,
          errorDetail: null,
        });
      } catch (error) {
        publish({
          ...state,
          phase: 'error',
          errorCode: errorCode(error),
          errorDetail: errorDetail(error),
        });
      }
      return;
    }

    const looksLikeJson = rawInput.startsWith('{') || rawInput.startsWith('[');
    if (!looksLikeJson) {
      publish({ ...state, phase: 'error', errorCode: 'input_unrecognized', errorDetail: null });
      return;
    }

    const parsed = parseBookSourceCatalog(rawInput);
    if (!parsed.ok) {
      publish({
        ...state,
        phase: 'error',
        errorCode: catalogErrorCode(parsed.reason),
        errorDetail: null,
      });
      return;
    }
    publish({
      ...state,
      phase: 'pick',
      catalog: parsed.items,
      selectedIds: parsed.items.map((item) => item.id),
      prepared: [],
      errorCode: null,
      errorDetail: null,
      failedItems: [],
    });
  };

  const setSelectedIds = (selectedIds: string[]) => {
    const knownIds = new Set(state.catalog.map((item) => item.id));
    publish({
      ...state,
      selectedIds: selectedIds.filter(
        (id, index, values) => knownIds.has(id) && values.indexOf(id) === index,
      ),
    });
  };

  const prepareSelected = async () => {
    const selectedItems = state.catalog.filter((item) => state.selectedIds.includes(item.id));
    if (selectedItems.length === 0 || selectedItems.length > CATALOG_INSTALL_CAP) {
      publish({ ...state, phase: 'pick', errorCode: 'selection_invalid', errorDetail: null });
      return;
    }
    publish({ ...state, phase: 'preparing', errorCode: null, errorDetail: null, failedItems: [] });
    const results = await Promise.all(
      selectedItems.map(async (item) => {
        try {
          const candidate = await adapter.prepare({ kind: 'legado', source_json: item.rawJson });
          return {
            item,
            candidate,
            isUpdate: isUpdate(candidate),
            previous: previousSource(candidate, state.sources),
            failure: null,
          };
        } catch (error) {
          return { item, candidate: null, isUpdate: false, previous: null, failure: error };
        }
      }),
    );
    const prepared = results.filter(
      (
        result,
      ): result is {
        item: CatalogItem;
        candidate: InstallCandidate;
        isUpdate: boolean;
        previous: InstalledSource | null;
        failure: null;
      } => result.candidate !== null,
    );
    const failures = results.filter((result) => result.failure !== null);
    if (prepared.length === 0) {
      const firstFailure = failures[0]?.failure;
      publish({
        ...state,
        phase: 'error',
        prepared: [],
        errorCode: firstFailure ? errorCode(firstFailure) : 'source_prepare_failed',
        errorDetail: firstFailure ? errorDetail(firstFailure) : null,
        failedItems: failures.map((result) => result.item.name),
      });
      return;
    }
    publish({
      ...state,
      phase: 'confirm',
      prepared,
      errorCode: failures.length > 0 ? 'source_prepare_partial' : null,
      errorDetail: failures[0]?.failure ? errorDetail(failures[0].failure) : null,
      failedItems: failures.map((result) => result.item.name),
    });
  };

  const retryPreparation = async () => {
    if (isHttpUrl(state.rawInput.trim())) return prepareInput();
    return prepareSelected();
  };

  const install = async () => {
    if (state.phase !== 'confirm' || state.prepared.length === 0) return;
    if (state.prepared.some((entry) => candidateRequestsSystem(entry.candidate))) {
      publish({ ...state, errorCode: 'system_grant_unsupported', errorDetail: null });
      return;
    }
    const { prepared } = state;
    publish({ ...state, phase: 'installing', errorCode: null, errorDetail: null });
    const results = await Promise.all(
      prepared.map(async (entry) => {
        try {
          return {
            entry,
            installed: await adapter.install(entry.candidate.id),
            failure: null,
          };
        } catch (error) {
          return { entry, installed: null, failure: error };
        }
      }),
    );
    const failures = results.filter((result) => result.failure !== null);
    if (failures.length > 0) {
      const firstFailure = failures[0].failure;
      const code = errorCode(firstFailure);
      const retryable = isRetryableCandidateError(code);
      publish({
        ...state,
        phase: retryable ? 'pick' : 'confirm',
        prepared: retryable
          ? []
          : results.filter((result) => result.failure !== null).map((result) => result.entry),
        errorCode: code,
        errorDetail: errorDetail(firstFailure),
        failedItems: failures.map((result) => result.entry.item.name),
      });
      return;
    }
    try {
      const sources = await adapter.listSources();
      publish({
        ...state,
        phase: 'done',
        sources,
        prepared: [],
        errorCode: null,
        errorDetail: null,
        failedItems: [],
      });
    } catch {
      publish({
        ...state,
        phase: 'done',
        prepared: [],
        errorCode: null,
        errorDetail: null,
        failedItems: [],
      });
    }
  };

  return {
    getState: () => state,
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    setInput,
    setError,
    refreshSources,
    prepareInput,
    setSelectedIds,
    prepareSelected,
    retryPreparation,
    install,
  };
}
