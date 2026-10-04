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
  cancelExecution,
  executeRule,
  listenRuleExecutionEvents,
  type ExecutionEvent,
  type ExecutionId,
  type IntentInput,
} from '@/shared/tauri/execution';
import {
  attachExecutionId,
  beginExecutionRun,
  failExecutionRun,
  IDLE_EXECUTION_RUN,
  isExecutionRunFinished,
  recordExecutionRequestFailure,
  reduceExecutionRun,
  type ExecutionRunState,
} from './execution';
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
  isRuleErrorWire,
  saveNativeRuleDocument,
  validateNativeRuleDocument,
  type CreateMode,
  type ExpectedDataType,
  type FlowEdge,
  type FlowNode,
  type FlowNodeKind,
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
  | 'document_not_open'
  | 'save_failed';

/**
 * 携带稳定 code 的 session 错误。
 *
 * `detail` 只用作 `Error.message`（日志与断言）；面向用户的文案一律由 code 决定，
 * 因此不会因后端 message 变动而破坏稳定性。
 */
export class SessionError extends Error {
  readonly code: SessionErrorCode;

  constructor(code: SessionErrorCode, detail?: string) {
    super(detail ?? code);
    this.name = 'SessionError';
    this.code = code;
  }
}

/**
 * 保存失败 → 既有诊断表面的稳定诊断。
 *
 * 后端 `RuleError` 自带稳定 code 与已本地化 message，有 diagnostics 时原样透传；
 * 没有时用 code 合成一条。非 IPC 错误（网络/JS 异常）退化为合成为 `SAVE_FAILED`，
 * 仍然是稳定 code，而不是把任意对象当作后端错误解释。
 */
function saveFailureDiagnostics(error: unknown): InstallDiagnostic[] {
  if (isRuleErrorWire(error)) {
    return error.diagnostics.length > 0
      ? error.diagnostics
      : [{ code: error.code, severity: 'error', message: error.message }];
  }
  return [
    {
      code: 'SAVE_FAILED',
      severity: 'error',
      message: error instanceof Error ? error.message : String(error),
    },
  ];
}

/** 保存失败的用户可见摘要；展示文案由 SessionError code 决定。 */
function saveFailureDetail(error: unknown): string {
  if (isRuleErrorWire(error)) return error.message;
  return error instanceof Error ? error.message : String(error);
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

/** 图内复制的载荷：语义子图 + 各节点当前落点。 */
export type SubgraphClipboard = {
  nodes: FlowNode[];
  edges: FlowEdge[];
  positions: Record<string, Position>;
};

/** 粘贴落点相对来源的默认偏移；完全重叠会让人以为什么都没发生。 */
const PASTE_OFFSET: Position = { x: 32, y: 32 };

/**
 * 从当前投影里切出一段可粘贴的子图。
 *
 * 只带两端都在选区内的边：半截边粘过来会指回原节点，语义上不是复制。
 */
export function subgraphFromProjection(
  projection: FlowProjection,
  nodeIds: readonly string[],
): SubgraphClipboard | null {
  const picked = new Set(nodeIds);
  const source = projection.nodes.filter((node) => picked.has(node.id));
  if (source.length === 0) return null;
  const positions: Record<string, Position> = {};
  for (const node of source) positions[node.id] = { x: node.position.x, y: node.position.y };
  return {
    nodes: source.map((node) => ({
      id: node.id,
      config: { kind: node.data.kind, value: node.data.config },
    })),
    edges: projection.edges
      .map((edge) => edge.data.edge)
      .filter((edge) => picked.has(edge.from.node_id) && picked.has(edge.to.node_id)),
    positions,
  };
}

export type SessionState = {
  /** 当前文档摘要（只读 metadata；null = 未打开文档）。 */
  summary: NativeRuleDocumentSummary | null;
  /** 当前执行使用的 Effective Rule Revision；无有效版本时为 null。 */
  effectiveSemanticRevision: number | null;
  /** Core 的不可变快照；只整体替换，不就地修改。 */
  core: NativeRuleAuthoringState;
  /** 仅供画布消费的一次定位请求；不属于文档语义或布局，也不进入撤销历史。 */
  revealRequest: { nodeId: string; sequence: number } | null;
  /** 预览运行态：不属于文档语义，也不进保存 / 撤销历史。 */
  execution: ExecutionRunState;
};

/**
 * 预览运行请求。
 *
 * `sourceId` 是已安装来源：`execute` 只跑已安装来源的已编译 Plan，本地草稿不参与，
 * 因此预览运行不能当作「未保存改动生效」的验证。
 */
export type PreviewRunRequest = {
  sourceId: string;
  intent: StandardIntent;
  input: IntentInput;
};

/**
 * 重放请求：在预览请求上指一段历史 execution。
 *
 * 重放的目标来源、意图与输入必须与那次历史一致，否则后端会用稳定的 replay 专属 code
 * 如实拒绝；拒绝不会被前端改写成一次实时请求。
 */
export type ReplayRunRequest = PreviewRunRequest & { archivedExecutionId: ExecutionId };

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

  // 预览执行（不改文档语义，也不进撤销历史）
  /**
   * 启动一次 live execution 预览。
   *
   * 先订阅事件再发启动请求：启动返回前到达的事件按 execution id 补叠，
   * 不会丢掉 `started` 与启动阶段的诊断。
   */
  startPreviewRun: (request: PreviewRunRequest) => Promise<void>;
  /**
   * 重放一段历史 execution。
   *
   * 重放只读固定历史 archive：失败就如实失败，任何路径都不回退成 live 请求。
   */
  startReplayRun: (request: ReplayRunRequest) => Promise<void>;
  /** 请求取消进行中的预览；终态由 runtime 的 `cancelled` 事件落定。 */
  cancelPreviewRun: () => Promise<void>;

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
  /** 选中并要求画布带该节点入视野；重复请求也应生效。 */
  revealNode: (nodeId: string) => void;
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
  /**
   * 把一段子图复制进当前文档（粘贴 / 直接复制）。
   *
   * id 在这里重新分配并重写边的端点，调用方只给来源子图与落点偏移。
   * 与 `addNode` + `moveNode` 同构：语义、布局和历史在一次粘贴命令中同时落地，避免
   * 两次撤销才能恢复粘贴前的图。
   */
  pasteSubgraph: (source: SubgraphClipboard, offset?: Position) => void;
  /** 就地复制若干节点（含其内部边）；落点从当前投影读，相对原位偏移。 */
  duplicateNodes: (nodeIds: string[], offset?: Position) => void;
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
      set({
        summary: detail.summary,
        effectiveSemanticRevision: detail.effective_semantic_revision,
        core: stateFromDetail(detail),
      });
      return summary;
    };

    /** 取当前文档 id；未打开文档时是调用方的用法错误。 */
    const requireDocumentId = (): string => {
      const documentId = get().summary?.document_id;
      if (!documentId) throw new SessionError('document_not_open');
      return documentId;
    };

    // 预览运行的事件订阅：同一 session 同时只留一条，终态或下一次运行前收掉。
    let previewUnlisten: (() => void) | null = null;
    const stopPreviewListener = () => {
      previewUnlisten?.();
      previewUnlisten = null;
    };

    /**
     * 跑一次预览：live 与 replay 共用同一条折叠路径，只有请求里的 mode 不同。
     *
     * 请求本身失败（订阅不上、启动被拒）只落到运行诊断，不向调用方抛错 —— 诊断栏
     * 就是失败的去处，抛出去只会让调用方再报一次同样的事。
     */
    const runPreview = async (
      request: PreviewRunRequest,
      replayOf: ExecutionId | null,
    ): Promise<void> => {
      // 同一时刻只跑一次预览：上一次订阅先收掉，避免两条流折进同一个状态。
      stopPreviewListener();
      set({ execution: beginExecutionRun(replayOf) });

      // 订阅先于启动：execute 返回前到达的事件先缓冲，拿到 id 后再补叠。
      const buffered: ExecutionEvent[] = [];
      let activeExecutionId: ExecutionId | null = null;
      const fold = (event: ExecutionEvent) => {
        const current = get().execution;
        const next = reduceExecutionRun(current, event);
        if (next !== current) set({ execution: next });
        if (isExecutionRunFinished(next)) stopPreviewListener();
      };
      try {
        previewUnlisten = await listenRuleExecutionEvents((event) => {
          if (activeExecutionId === null) {
            buffered.push(event);
          } else if (event.execution_id === activeExecutionId) {
            fold(event);
          }
        });
      } catch (error) {
        // 订阅不上就不能老老实实报运行诊断；如实失败，不瓣一个看不到结果的运行。
        set({ execution: failExecutionRun(get().execution, error) });
        return;
      }

      let executionId: ExecutionId;
      try {
        const response = await executeRule({
          source_id: request.sourceId,
          intent: request.intent,
          input: request.input,
          mode: replayOf === null ? { mode: 'live' } : { mode: 'replay', execution_id: replayOf },
        });
        executionId = response.execution_id;
      } catch (error) {
        // 启动就失败（来源未安装、重放基线不可用、IPC 异常）：走同一诊断表面。
        stopPreviewListener();
        set({ execution: failExecutionRun(get().execution, error) });
        return;
      }

      activeExecutionId = executionId;
      set({ execution: attachExecutionId(get().execution, executionId) });
      for (const event of buffered) {
        if (event.execution_id === executionId) fold(event);
      }
    };

    return {
      summary: null,
      effectiveSemanticRevision: null,
      core: createInitialState({ definition: createBlankDefinition('') }),
      revealRequest: null,
      execution: IDLE_EXECUTION_RUN,

      async loadDocument(documentId) {
        const detail = await getNativeRuleDocument({ document_id: documentId });
        if (!detail) throw new SessionError('document_not_found');
        set({
          summary: detail.summary,
          effectiveSemanticRevision: detail.effective_semantic_revision,
          core: stateFromDetail(detail),
        });
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

          const { summary, effectiveSemanticRevision } = get();
          if (summary) {
            const nextEffectiveRevision =
              outcome.semantic?.activation === 'effective'
                ? outcome.semantic.revision
                : effectiveSemanticRevision;
            set({
              effectiveSemanticRevision: nextEffectiveRevision,
              summary: {
                ...summary,
                semantic_revision: outcome.semantic?.revision ?? summary.semantic_revision,
                layout_revision: outcome.layout?.revision ?? summary.layout_revision,
              },
            });
          }
          return outcome;
        } catch (error) {
          step({
            kind: 'saveFailure',
            epoch: inFlight.epoch,
            diagnostics: saveFailureDiagnostics(error),
          });
          throw new SessionError('save_failed', saveFailureDetail(error));
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

      async startPreviewRun(request) {
        await runPreview(request, null);
      },

      async startReplayRun({ archivedExecutionId, ...request }) {
        await runPreview(request, archivedExecutionId);
      },

      async cancelPreviewRun() {
        const { executionId } = get().execution;
        if (executionId === null) return;
        try {
          // 取消是请求，不是状态转换：终态由 runtime 的 `cancelled` 事件落定。
          await cancelExecution({ execution_id: executionId });
        } catch (error) {
          set({ execution: recordExecutionRequestFailure(get().execution, error) });
        }
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
      revealNode: (nodeId) => {
        step({ kind: 'selection', nodeId });
        const previous = get().revealRequest;
        set({
          revealRequest: {
            nodeId,
            sequence: previous ? previous.sequence + 1 : 1,
          },
        });
      },
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

      pasteSubgraph(source, offset = PASTE_OFFSET) {
        if (source.nodes.length === 0) return;
        const idMap = new Map(source.nodes.map((node) => [node.id, newNodeId()]));
        const nodes = source.nodes.map((node) => ({
          ...cloneJson(node),
          id: idMap.get(node.id) ?? node.id,
        }));
        const edges = source.edges.flatMap((edge) => {
          const from = idMap.get(edge.from.node_id);
          const to = idMap.get(edge.to.node_id);
          // 只保留两端都在来源子图内的边；跨界的边粘过来会指回原节点。
          return from && to
            ? [{ from: { ...edge.from, node_id: from }, to: { ...edge.to, node_id: to } }]
            : [];
        });
        const positions: Record<string, Position> = {};
        for (const [sourceId, nextId] of idMap) {
          const at = source.positions[sourceId] ?? { x: 0, y: 0 };
          positions[nextId] = { x: at.x + offset.x, y: at.y + offset.y };
        }
        get().dispatchAll([
          { kind: 'pasteSubgraph', nodes, edges, positions },
          { kind: 'selection', nodeId: nodes[0]?.id ?? null },
        ]);
      },

      duplicateNodes(nodeIds, offset = PASTE_OFFSET) {
        const source = subgraphFromProjection(selectFlowProjection(get()), nodeIds);
        if (source) get().pasteSubgraph(source, offset);
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

/** 预览运行态。 */
export function selectExecutionRun(state: SessionState): ExecutionRunState {
  return state.execution;
}

/**
 * 可重放的历史 execution id；没有时为 null。
 *
 * 只认本会话里以 live 跑完并成功的运行：后端要求被重放的 execution 以 Completed
 * 结束，并且重放自己的运行不是可重放基线（历史 archive 固定在最初那次 live 运行上）。
 */
export function selectReplaySource(state: SessionState): ExecutionId | null {
  const { execution } = state;
  if (execution.status !== 'succeeded' || execution.mode !== 'live') return null;
  return execution.executionId;
}
