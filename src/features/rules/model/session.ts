//! 原生规则编辑器 session：唯一的页面状态 owner。
//!
//! 持有 core 的不可变 state、文档 metadata 与全部 IPC 编排。组件只调 action、
//! 读投影，不直接接触 `reduce`。
//!
//! 用 zustand 的 vanilla store 而不是 `useReducer`：编辑器状态要被画布、
//! 检查器、诊断列表、工具栏多处订阅，xyflow 的拖拽/连线是高频回调，
//! 选择器订阅能让每个组件只在自己关心的切片变化时重渲染。
//! store 由工厂创建而非模块单例，切换文档就换一个实例，不会残留上一份历史。
//!
//! 不变量（继承自 core）：credential 明文只经由一次性请求，不进 history /
//! undo / snapshot，`saveRequest` 之后立即从 state 清空，永不写入 tauri-store。

import { createStore, type StoreApi } from 'zustand/vanilla';
import {
  cloneJson,
  createBlankDefinition,
  createInitialState,
  readLayout,
  reduce,
  type AuthoringAction,
  type NativeRuleAuthoringState,
  type Position,
  type ValidationState,
} from './core';
import {
  loopRegionSelectionToken,
  portSelectionToken,
  semanticToFlow,
  type FlowHandleDirection,
  type FlowProjection,
} from './flow-adapter';
import { parseLayoutJson } from './layout-json';
import { canonicalConfig } from './node-defaults';
import {
  createNativeRuleDocument,
  getNativeRuleDocument,
  prepareNativeRuleDocument,
  saveNativeRuleDocument,
  validateNativeRuleDocument,
  type CreateMode,
  type ExpectedDataType,
  type FlowEdge,
  type FlowNode,
  type FlowNodeKind,
  type InstallCandidate,
  type InstallDiagnostic,
  type NativeRuleDocumentDetail,
  type NativeRuleDocumentSummary,
  type RuleDefinition,
  type SaveNativeRuleDocumentOutcome,
  type StandardIntent,
  type ValidateNativeRuleDocumentPreview,
} from '@/shared/tauri/rules';

/** Session 抛出的稳定错误码；展示文案由视图按 locale 组装。 */
export type SessionErrorCode =
  | 'document_not_found'
  | 'document_semantic_missing'
  | 'document_semantic_unsaved'
  | 'document_validation_required'
  | 'document_not_open';

/** 携带稳定 code 的 session 错误。 */
export class SessionError extends Error {
  readonly code: SessionErrorCode;

  constructor(code: SessionErrorCode) {
    super(code);
    this.name = 'SessionError';
    this.code = code;
  }
}

/** 从详情快照重建本地 state；缺少语义快照时返回稳定错误。 */
function stateFromDetail(detail: NativeRuleDocumentDetail): NativeRuleAuthoringState {
  if (!detail.definition) throw new SessionError('document_semantic_missing');
  const semanticRevision = detail.semantic_revision ?? detail.summary.semantic_revision;
  const layoutRevision = detail.layout_revision ?? detail.summary.layout_revision;
  return createInitialState({
    definition: detail.definition,
    layout: parseLayoutJson(detail.layout_json),
    savedSemanticRevision: semanticRevision === 0 ? null : semanticRevision,
    savedLayoutRevision: layoutRevision === 0 ? null : layoutRevision,
  });
}

export type SessionState = {
  /** 当前文档摘要（只读 metadata；null = 未打开文档）。 */
  summary: NativeRuleDocumentSummary | null;
  /** Core 的不可变快照；只整体替换，不就地修改。 */
  core: NativeRuleAuthoringState;
};

export type SessionActions = {
  // 文档生命周期
  loadDocument: (documentId: string) => Promise<void>;
  createBlank: () => Promise<NativeRuleDocumentSummary>;
  createTemplate: (options: {
    title: string;
    intent: StandardIntent;
    dataType: ExpectedDataType;
    baseUrl: string;
  }) => Promise<NativeRuleDocumentSummary>;
  createImported: (options: {
    title: string;
    definition: RuleDefinition;
  }) => Promise<NativeRuleDocumentSummary>;
  save: () => Promise<SaveNativeRuleDocumentOutcome>;
  validate: () => Promise<ValidateNativeRuleDocumentPreview>;
  prepare: () => Promise<InstallCandidate>;

  // 状态变更
  dispatch: (action: AuthoringAction) => void;
  dispatchAll: (actions: AuthoringAction[]) => void;
  undo: () => void;
  redo: () => void;

  // 画布便捷操作
  connect: (edge: FlowEdge) => void;
  disconnect: (edge: FlowEdge) => void;
  reconnect: (from: FlowEdge, to: FlowEdge) => void;
  moveNode: (nodeId: string, position: Position) => void;
  layoutNodes: (positions: Record<string, Position>) => void;
  collapseNode: (nodeId: string, collapsed: boolean) => void;
  collapseLoopRegion: (loopNodeId: string, collapsed: boolean) => void;
  selectNode: (nodeId: string | null) => void;
  selectEdge: (edgeId: string) => void;
  selectPort: (nodeId: string, direction: FlowHandleDirection, handle: string) => void;
  selectLoopRegion: (loopNodeId: string) => void;
  focusIntent: (intent: StandardIntent | null) => void;
  setNodeConfig: (nodeId: string, patch: Record<string, unknown>) => void;
  credentialReplace: (
    nodeId: string,
    jsonPointer: string,
    logicalName: string,
    value: string,
  ) => void;
  credentialClear: (nodeId: string, jsonPointer: string, logicalName: string) => void;
  setValidation: (validation: ValidationState) => void;
  setDiagnostics: (diagnostics: InstallDiagnostic[]) => void;
  addNode: (kind: FlowNodeKind, configValue?: Record<string, unknown>) => string;
  deleteNode: (nodeId: string) => void;
  deleteSelection: (nodeIds: string[], edgeIds: string[]) => void;
};

export type SessionStore = SessionState & SessionActions;
export type RuleEditorSession = StoreApi<SessionStore>;

/**
 * 创建一个编辑器 session store。
 *
 * `newNodeId` 可注入，测试里给确定性 id；默认用 `crypto.randomUUID()`。
 */
export function createRuleEditorSession(options?: { newNodeId?: () => string }): RuleEditorSession {
  const newNodeId = options?.newNodeId ?? (() => crypto.randomUUID());

  return createStore<SessionStore>((set, get) => {
    /** 把 core 推进一步；core 是不可变的，整体替换即可。 */
    const step = (action: AuthoringAction) => {
      set({ core: reduce(get().core, action) });
    };

    /** 创建文档后回读详情并接管 state；三种创建模式共用。 */
    const openCreated = async (mode: CreateMode): Promise<NativeRuleDocumentSummary> => {
      const summary = await createNativeRuleDocument({ mode });
      const detail = await getNativeRuleDocument({ document_id: summary.document_id });
      if (!detail) throw new SessionError('document_not_found');
      set({ summary: detail.summary, core: stateFromDetail(detail) });
      return summary;
    };

    /** 取当前文档 id；未打开文档时是调用方的用法错误。 */
    const requireDocumentId = (): string => {
      const documentId = get().summary?.document_id;
      if (!documentId) throw new SessionError('document_not_open');
      return documentId;
    };

    return {
      summary: null,
      core: createInitialState({ definition: createBlankDefinition('') }),

      async loadDocument(documentId) {
        const detail = await getNativeRuleDocument({ document_id: documentId });
        if (!detail) throw new SessionError('document_not_found');
        set({ summary: detail.summary, core: stateFromDetail(detail) });
      },

      createBlank: () => openCreated({ kind: 'blank' }),

      createTemplate: ({ title, intent, dataType, baseUrl }) =>
        openCreated({
          kind: 'template',
          title,
          intent,
          data_type: dataType,
          base_url: baseUrl,
        }),

      createImported: ({ title, definition }) =>
        openCreated({ kind: 'import', title, definition: cloneJson(definition) }),

      async save() {
        const documentId = requireDocumentId();

        // SaveRequest 拍快照、递增 epoch，并把一次性 credential 明文移出 state。
        step({ kind: 'saveRequest' });
        const inFlight = get().core.inFlightSave;
        if (!inFlight) {
          // 没有 dirty 域：幂等返回当前 revision，不发请求。
          const { core } = get();
          return {
            document_id: documentId,
            semantic: { revision: core.savedSemanticRevision ?? 0, conflict: null },
            layout: { revision: core.savedLayoutRevision ?? 0, conflict: null },
          };
        }

        try {
          const outcome = await saveNativeRuleDocument({
            document_id: documentId,
            semantic: inFlight.semantic
              ? {
                  expected_revision: inFlight.semantic.revision,
                  definition: inFlight.semantic.content as RuleDefinition,
                  credential_mutations: [...inFlight.semantic.credentialMutations],
                }
              : null,
            layout: inFlight.layout
              ? {
                  expected_revision: inFlight.layout.revision,
                  layout_json: JSON.stringify(inFlight.layout.content),
                }
              : null,
          });

          step({ kind: 'saveResponse', epoch: inFlight.epoch, outcome });

          const { summary } = get();
          if (summary) {
            set({
              summary: {
                ...summary,
                semantic_revision: outcome.semantic?.revision ?? summary.semantic_revision,
                layout_revision: outcome.layout?.revision ?? summary.layout_revision,
              },
            });
          }
          return outcome;
        } catch (error) {
          step({ kind: 'saveFailure', epoch: inFlight.epoch });
          throw error;
        }
      },

      async validate() {
        const documentId = requireDocumentId();
        const { core } = get();
        if (core.dirty.semantic || core.conflict.semantic) {
          throw new SessionError('document_semantic_unsaved');
        }
        const revision = core.savedSemanticRevision ?? 0;
        step({
          kind: 'setValidation',
          validation: { ...core.validation, status: 'pending' },
        });
        try {
          const preview = await validateNativeRuleDocument({
            document_id: documentId,
            revision,
          });
          // 请求期间语义又被改动过时，结果只能标 stale，不能冒充当前版本的结论。
          const after = get().core;
          const stillCurrent =
            after.savedSemanticRevision === revision &&
            !after.dirty.semantic &&
            !after.conflict.semantic;
          step({
            kind: 'setValidation',
            validation: {
              status: stillCurrent ? (preview.valid ? 'valid' : 'invalid') : 'stale',
              revision: preview.revision,
              definitionHash: preview.definition_hash,
              planHash: preview.plan_hash,
              diagnostics: cloneJson(preview.diagnostics),
            },
          });
          return preview;
        } catch (error) {
          step({
            kind: 'setValidation',
            validation: { ...get().core.validation, status: 'error' },
          });
          throw error;
        }
      },

      async prepare() {
        const documentId = requireDocumentId();
        // 先落盘所有 dirty 域；冲突响应不会被误当作保存成功。
        if (get().core.dirty.semantic || get().core.dirty.layout) {
          await get().save();
        }
        const { core } = get();
        const revision = core.savedSemanticRevision;
        if (
          revision === null ||
          core.dirty.semantic ||
          core.conflict.semantic ||
          core.validation.status !== 'valid' ||
          core.validation.revision !== revision ||
          !core.validation.definitionHash
        ) {
          throw new SessionError('document_validation_required');
        }
        const candidate = await prepareNativeRuleDocument({
          document_id: documentId,
          revision,
        });
        step({
          kind: 'setCandidate',
          candidate: { id: candidate.id, expires_at_ms: candidate.expires_at_ms },
        });
        return candidate;
      },

      dispatch: step,
      dispatchAll(actions) {
        set({ core: actions.reduce(reduce, get().core) });
      },
      undo: () => step({ kind: 'undo' }),
      redo: () => step({ kind: 'redo' }),

      connect: (edge) => step({ kind: 'edgeConnect', edge, connected: true }),
      disconnect: (edge) => step({ kind: 'edgeConnect', edge, connected: false }),
      reconnect: (from, to) => step({ kind: 'edgeReconnect', from, to }),
      moveNode: (nodeId, position) => step({ kind: 'moveNode', nodeId, position }),
      layoutNodes: (positions) => step({ kind: 'layoutNodes', positions }),
      collapseNode: (nodeId, collapsed) => step({ kind: 'collapseNode', nodeId, collapsed }),
      collapseLoopRegion: (loopNodeId, collapsed) =>
        step({ kind: 'collapseLoopRegion', loopNodeId, collapsed }),

      selectNode: (nodeId) => step({ kind: 'selection', nodeId }),
      selectEdge: (edgeId) => step({ kind: 'selection', nodeId: `edge:${edgeId}` }),
      selectPort: (nodeId, direction, handle) =>
        step({ kind: 'selection', nodeId: portSelectionToken(nodeId, direction, handle) }),
      selectLoopRegion: (loopNodeId) =>
        step({ kind: 'selection', nodeId: loopRegionSelectionToken(loopNodeId) }),
      focusIntent: (intent) => step({ kind: 'intentFocus', intent }),

      setNodeConfig: (nodeId, patch) => step({ kind: 'setNodeConfig', nodeId, patch }),
      credentialReplace: (nodeId, jsonPointer, logicalName, value) =>
        step({ kind: 'credentialReplace', nodeId, jsonPointer, logicalName, value }),
      credentialClear: (nodeId, jsonPointer, logicalName) =>
        step({ kind: 'credentialClear', nodeId, jsonPointer, logicalName }),
      setValidation: (validation) => step({ kind: 'setValidation', validation }),
      setDiagnostics(diagnostics) {
        // 只给裸诊断时标 error，避免伪装成已通过校验。
        step({
          kind: 'setValidation',
          validation: {
            ...get().core.validation,
            status: 'error',
            diagnostics: cloneJson(diagnostics),
          },
        });
      },

      addNode(kind, configValue) {
        const id = newNodeId();
        const node: FlowNode = {
          id,
          config: { kind, value: canonicalConfig(kind, configValue ?? null) },
        };
        step({ kind: 'nodeAdd', node });
        step({ kind: 'selection', nodeId: id });
        return id;
      },

      deleteNode(nodeId) {
        step({ kind: 'nodeDelete', nodeId });
        if (get().core.selection === nodeId) step({ kind: 'selection', nodeId: null });
      },

      deleteSelection(nodeIds, edgeIds) {
        step({ kind: 'deleteSelection', nodeIds, edgeIds });
        const { core } = get();
        if (core.selection && !core.definition.flow.nodes.some((n) => n.id === core.selection)) {
          step({ kind: 'selection', nodeId: null });
        }
      },
    };
  });
}

// ---------------------------------------------------------------------------
// 派生投影
// ---------------------------------------------------------------------------

/**
 * Flow 投影按 core 身份缓存。
 *
 * core 是不可变的，同一个引用必然对应同一份投影；WeakMap 让旧快照能被回收。
 * 缓存是必需的：selector 每次返回新对象会让 zustand 的 Object.is 判定为变化，
 * 陷入无限重渲染。
 */
const PROJECTION_CACHE = new WeakMap<NativeRuleAuthoringState, FlowProjection>();

/** 从 core 取 Flow 投影（同一 core 引用复用同一份结果）。 */
export function selectFlowProjection(state: SessionState): FlowProjection {
  const { core } = state;
  const cached = PROJECTION_CACHE.get(core);
  if (cached) return cached;
  const projection = semanticToFlow(
    core.definition,
    readLayout(core.layout),
    core.intentFocus,
    core.selection,
  );
  PROJECTION_CACHE.set(core, projection);
  return projection;
}

/** 任一域有未保存变更。 */
export function selectHasUnsavedChanges(state: SessionState): boolean {
  return state.core.dirty.semantic || state.core.dirty.layout;
}

/** 是否有保存在途。 */
export function selectIsSaving(state: SessionState): boolean {
  return state.core.inFlightSave !== null;
}

/** 能否撤销。 */
export function selectCanUndo(state: SessionState): boolean {
  return state.core.history.length > 0;
}

/** 能否重做。 */
export function selectCanRedo(state: SessionState): boolean {
  return state.core.redo.length > 0;
}
