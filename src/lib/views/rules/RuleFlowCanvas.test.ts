//! RuleFlowCanvas 测试：连接 gate 拒绝/放行、typed action 派发、
//! 意图聚焦不复制节点、键盘删除与挂载 teardown。
//!
//! @xyflow/svelte 以测试替身替代（SvelteFlowStub 捕获 props 供测试调用回调）。

import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@xyflow/svelte', () => import('../../../test/xyflow-stubs/mock'));
vi.mock('$lib/rules/native-authoring/flow-layout', () => ({
  layoutNativeRuleFlow: vi.fn(async () => ({
    'http-a': { x: 48, y: 48 },
    'extract-a': { x: 468, y: 48 },
  })),
}));

import RuleFlowCanvas from './RuleFlowCanvas.svelte';
import { captured } from '../../../test/xyflow-stubs/capture';
import { flowHelpers } from '../../../test/xyflow-stubs/mock';
import type { NativeRuleEditorSession } from '$lib/rules/native-authoring/session.svelte';
import type { FlowEdge, FlowNodeKind } from '$lib/rules/native-authoring/wire';
import type { FlowConnection } from './connection-gate';

type Session = {
  definition: { intent_exports: Record<string, { flow_entry: string } | undefined> };
  flowProjection: {
    nodes: unknown[];
    edges: unknown[];
    loopRegions?: unknown[];
  };
  intentFocus: string | null;
  selection: string | null;
  connect: ReturnType<typeof vi.fn>;
  reconnect: ReturnType<typeof vi.fn>;
  disconnect: ReturnType<typeof vi.fn>;
  moveNode: ReturnType<typeof vi.fn>;
  layoutNodes: ReturnType<typeof vi.fn>;
  addNode: ReturnType<typeof vi.fn>;
  deleteNode: ReturnType<typeof vi.fn>;
  focusIntent: ReturnType<typeof vi.fn>;
  selectNode: ReturnType<typeof vi.fn>;
  selectEdge: ReturnType<typeof vi.fn>;
};

function createSession(overrides: Partial<Session> = {}): NativeRuleEditorSession {
  return {
    definition: { intent_exports: {} },
    flowProjection: { nodes: [], edges: [] },
    intentFocus: null,
    selection: null,
    connect: vi.fn(),
    reconnect: vi.fn(),
    disconnect: vi.fn(),
    moveNode: vi.fn(),
    layoutNodes: vi.fn(),
    addNode: vi.fn(() => 'new-node'),
    deleteNode: vi.fn(),
    focusIntent: vi.fn(),
    selectNode: vi.fn(),
    selectEdge: vi.fn(),
    ...overrides,
  } as unknown as NativeRuleEditorSession;
}

function flowNode(id: string, kind: FlowNodeKind) {
  return {
    id,
    type: kind,
    position: { x: 0, y: 0 },
    data: {
      nodeId: id,
      kind,
      config: {},
      diagnostics: [],
      focused: true,
      dimmed: false,
      collapsed: false,
    },
  };
}

function flowEdge(id: string, from: string, fromHandle: string, to: string, toHandle: string) {
  return {
    id,
    data: {
      edge: {
        from: { node_id: from, handle: fromHandle },
        to: { node_id: to, handle: toHandle },
      } satisfies FlowEdge,
    },
  };
}

function conn(
  source: string,
  sourceHandle: string,
  target: string,
  targetHandle: string,
): FlowConnection {
  return { source, target, sourceHandle, targetHandle };
}

/** 读取 SvelteFlow 替身捕获的 props。 */
function flowProps(): Record<string, unknown> {
  expect(captured.props, 'SvelteFlow 替身未捕获到 props').toBeDefined();
  return captured.props as Record<string, unknown>;
}

describe('RuleFlowCanvas：投影渲染', () => {
  it('把 session.flowProjection 的节点/边传入 SvelteFlow（受控）', () => {
    const session = createSession({
      flowProjection: {
        nodes: [flowNode('http-a', 'http'), flowNode('extract-a', 'extract')],
        edges: [flowEdge('e1', 'http-a', 'http_response', 'extract-a', 'source')],
      },
    });
    const { container } = render(RuleFlowCanvas, { props: { session } });
    expect(container.querySelector('[data-slot="rule-flow-canvas"]')).toBeTruthy();
    expect(flowProps().nodes).toHaveLength(2);
    expect(flowProps().edges).toHaveLength(1);
  });

  it('Loop 区域底板在 back，标题控件提升到 front', () => {
    const session = createSession({
      flowProjection: {
        nodes: [flowNode('loop-a', 'loop')],
        edges: [],
        loopRegions: [
          {
            loopNodeId: 'loop-a',
            status: 'valid',
            collapsed: false,
            bodyEntry: null,
            yieldSource: null,
            bodyNodes: [],
            collectionEdgeIds: [],
            bodyEdgeIds: [],
            yieldEdgeIds: [],
            doneEdgeIds: [],
            crossRegionEdgeIds: [],
            boundaryEdgeIds: [],
            diagnostics: [],
          },
        ],
      },
    });
    const { container } = render(RuleFlowCanvas, { props: { session } });

    expect(
      [...container.querySelectorAll('[data-testid="viewport-portal"]')].map((portal) =>
        portal.getAttribute('data-target'),
      ),
    ).toEqual(['back', 'front']);
  });
});

describe('RuleFlowCanvas：连接 gate', () => {
  it('合法连接：onconnect 派发 typed action 到 session.connect', () => {
    const session = createSession({
      flowProjection: {
        nodes: [flowNode('http-a', 'http'), flowNode('extract-a', 'extract')],
        edges: [],
      },
    });
    render(RuleFlowCanvas, { props: { session } });
    const props = flowProps();
    const c = conn('http-a', 'http_response', 'extract-a', 'source');
    expect((props.isValidConnection as (c: FlowConnection) => boolean)(c)).toBe(true);
    (props.onconnect as (c: FlowConnection) => void)(c);
    expect(session.connect).toHaveBeenCalledTimes(1);
    expect(session.connect).toHaveBeenCalledWith({
      from: { node_id: 'http-a', handle: 'output' },
      to: { node_id: 'extract-a', handle: 'input' },
    });
  });

  it('端口类型不兼容：isValidConnection=false、onbeforeconnect=false，onconnect 不写 state', () => {
    const session = createSession({
      flowProjection: {
        nodes: [flowNode('http-a', 'http'), flowNode('mapper-a', 'mapper')],
        edges: [],
      },
    });
    render(RuleFlowCanvas, { props: { session } });
    const props = flowProps();
    const c = conn('http-a', 'http_response', 'mapper-a', 'in');
    expect((props.isValidConnection as (c: FlowConnection) => boolean)(c)).toBe(false);
    expect((props.onbeforeconnect as (c: FlowConnection) => FlowConnection | false)(c)).toBe(false);
    (props.onconnect as (c: FlowConnection) => void)(c);
    expect(session.connect).not.toHaveBeenCalled();
  });

  it('回边拒绝：已存在 A→B 时，B→A 被 gate 拦截', () => {
    const session = createSession({
      flowProjection: {
        nodes: [flowNode('http-a', 'http'), flowNode('extract-a', 'extract')],
        edges: [flowEdge('e1', 'http-a', 'http_response', 'extract-a', 'source')],
      },
    });
    render(RuleFlowCanvas, { props: { session } });
    const props = flowProps();
    const c = conn('extract-a', 'json', 'http-a', 'in');
    expect((props.isValidConnection as (c: FlowConnection) => boolean)(c)).toBe(false);
    (props.onconnect as (c: FlowConnection) => void)(c);
    expect(session.connect).not.toHaveBeenCalled();
  });

  it('意图焦点：源节点不在焦点子图内 → intent-mismatch 拒绝', () => {
    const session = createSession({
      intentFocus: 'Search',
      definition: {
        intent_exports: { Search: { flow_entry: 'http-a' } },
      },
      flowProjection: {
        nodes: [
          flowNode('http-a', 'http'),
          flowNode('extract-a', 'extract'),
          flowNode('js-a', 'js'),
        ],
        edges: [flowEdge('e1', 'http-a', 'http_response', 'extract-a', 'source')],
      },
    });
    render(RuleFlowCanvas, { props: { session } });
    const props = flowProps();
    // js-a 不在 Search 焦点子图内
    const c = conn('js-a', 'json', 'extract-a', 'source');
    expect((props.isValidConnection as (c: FlowConnection) => boolean)(c)).toBe(false);
    (props.onconnect as (c: FlowConnection) => void)(c);
    expect(session.connect).not.toHaveBeenCalled();
  });

  it('重连：onreconnect 校验后派发 session.reconnect', () => {
    const session = createSession({
      flowProjection: {
        nodes: [
          flowNode('http-a', 'http'),
          flowNode('extract-a', 'extract'),
          flowNode('mapper-a', 'mapper'),
        ],
        edges: [flowEdge('e1', 'http-a', 'http_response', 'extract-a', 'source')],
      },
    });
    render(RuleFlowCanvas, { props: { session } });
    const props = flowProps();
    const oldEdge = flowEdge('e1', 'http-a', 'http_response', 'extract-a', 'source');
    const c = conn('http-a', 'http_response', 'mapper-a', 'in');
    expect((props.isValidConnection as (c: FlowConnection) => boolean)(c)).toBe(false);
    (props.onreconnect as (old: unknown, next: FlowConnection) => void)(oldEdge, c);
    expect(session.reconnect).not.toHaveBeenCalled();
    const ok = conn('extract-a', 'json', 'mapper-a', 'in');
    (props.onreconnect as (old: unknown, next: FlowConnection) => void)(oldEdge, ok);
    expect(session.reconnect).toHaveBeenCalledWith(
      {
        from: { node_id: 'http-a', handle: 'http_response' },
        to: { node_id: 'extract-a', handle: 'source' },
      },
      {
        from: { node_id: 'extract-a', handle: 'output' },
        to: { node_id: 'mapper-a', handle: 'input' },
      },
    );
  });
});

describe('RuleFlowCanvas：节点移动/选择', () => {
  it('onnodedragstop 拖动结束 → session.moveNode', () => {
    const session = createSession({
      flowProjection: { nodes: [flowNode('http-a', 'http')], edges: [] },
    });
    render(RuleFlowCanvas, { props: { session } });
    const props = flowProps();
    (
      props.onnodedragstop as (event: {
        targetNode: { id: string; position: { x: number; y: number } } | null;
        nodes: unknown[];
        event: PointerEvent;
      }) => void
    )({
      targetNode: { id: 'http-a', position: { x: 120, y: 80 } },
      nodes: [],
      event: new PointerEvent('pointerup'),
    });
    expect(session.moveNode).toHaveBeenCalledWith('http-a', { x: 120, y: 80 });
  });

  it('onnodedragstop targetNode 为 null 不发 moveNode', () => {
    const session = createSession({});
    render(RuleFlowCanvas, { props: { session } });
    const props = flowProps();
    (
      props.onnodedragstop as (event: {
        targetNode: null;
        nodes: unknown[];
        event: PointerEvent;
      }) => void
    )({
      targetNode: null,
      nodes: [],
      event: new PointerEvent('pointerup'),
    });
    expect(session.moveNode).not.toHaveBeenCalled();
  });

  it('节点点击选中、画布点击取消选中', () => {
    const session = createSession({});
    render(RuleFlowCanvas, { props: { session } });
    const props = flowProps();
    (props.onnodeclick as (p: { node: { id: string }; event: PointerEvent }) => void)({
      node: { id: 'http-a' },
      event: new PointerEvent('click'),
    });
    expect(session.selectNode).toHaveBeenCalledWith('http-a');
    (props.onpaneclick as () => void)();
    expect(session.selectNode).toHaveBeenCalledWith(null);
  });

  it('连线点击选中 edge selection token', () => {
    const session = createSession({
      flowProjection: {
        nodes: [flowNode('http-a', 'http'), flowNode('extract-a', 'extract')],
        edges: [flowEdge('e1', 'http-a', 'http_response', 'extract-a', 'source')],
      },
    });
    render(RuleFlowCanvas, { props: { session } });
    const props = flowProps();
    (props.onedgeclick as (p: { edge: { id: string }; event: PointerEvent }) => void)({
      edge: { id: 'e1' },
      event: new PointerEvent('click'),
    });
    expect(session.selectEdge).toHaveBeenCalledWith('e1');
  });

  it('Svelte Flow 内部选择变化同步到 session', () => {
    const session = createSession({
      flowProjection: {
        nodes: [flowNode('http-a', 'http')],
        edges: [flowEdge('e1', 'http-a', 'http_response', 'extract-a', 'source')],
      },
    });
    render(RuleFlowCanvas, { props: { session } });
    const props = flowProps();

    (
      props.onselectionchange as (value: { nodes: unknown[]; edges: Array<{ id: string }> }) => void
    )({
      nodes: [],
      edges: [{ id: 'e1' }],
    });
    expect(session.selectEdge).toHaveBeenCalledWith('e1');

    (
      props.onselectionchange as (value: { nodes: Array<{ id: string }>; edges: unknown[] }) => void
    )({
      nodes: [{ id: 'http-a' }],
      edges: [],
    });
    expect(session.selectNode).toHaveBeenCalledWith('http-a');
  });
});

describe('RuleFlowCanvas：键盘删除与 teardown', () => {
  it('Backspace/Delete 删除选中节点（读取 session.selection）', () => {
    const session = createSession({ selection: 'http-a' });
    render(RuleFlowCanvas, { props: { session } });
    fireEvent.keyDown(window, { key: 'Delete' });
    expect(session.deleteNode).toHaveBeenCalledWith('http-a');
    fireEvent.keyDown(window, { key: 'Backspace' });
    expect(session.deleteNode).toHaveBeenCalledTimes(2);
  });

  it('输入框聚焦时按 Delete 不触发节点删除', () => {
    const session = createSession({ selection: 'http-a' });
    render(RuleFlowCanvas, { props: { session } });
    const input = document.createElement('input');
    document.body.appendChild(input);
    fireEvent.keyDown(input, { key: 'Delete' });
    expect(session.deleteNode).not.toHaveBeenCalled();
    input.remove();
  });

  it('Delete 删除选中连线，不误删节点', () => {
    const edge = flowEdge('e1', 'http-a', 'http_response', 'extract-a', 'source');
    const session = createSession({
      selection: 'edge:e1',
      flowProjection: { nodes: [], edges: [edge] },
    });
    render(RuleFlowCanvas, { props: { session } });
    fireEvent.keyDown(window, { key: 'Delete' });
    expect(session.disconnect).toHaveBeenCalledWith(edge.data.edge);
    expect(session.deleteNode).not.toHaveBeenCalled();
  });

  it('卸载后键盘监听被 teardown（cleanup）', () => {
    const session = createSession({ selection: 'http-a' });
    const { unmount } = render(RuleFlowCanvas, { props: { session } });
    fireEvent.keyDown(window, { key: 'Delete' });
    expect(session.deleteNode).toHaveBeenCalledTimes(1);
    unmount();
    fireEvent.keyDown(window, { key: 'Delete' });
    expect(session.deleteNode).toHaveBeenCalledTimes(1);
  });
});

describe('RuleFlowCanvas：意图聚焦不复制节点', () => {
  it('点击「搜索」只派发 focusIntent + fitView，节点数不变', () => {
    const session = createSession({
      definition: {
        intent_exports: { Search: { flow_entry: 'http-a' } },
      },
      flowProjection: {
        nodes: [flowNode('http-a', 'http'), flowNode('extract-a', 'extract')],
        edges: [flowEdge('e1', 'http-a', 'http_response', 'extract-a', 'source')],
      },
    });
    render(RuleFlowCanvas, { props: { session } });
    const before = flowProps().nodes as unknown[];
    fireEvent.click(screen.getByRole('radio', { name: '搜索' }));
    expect(session.focusIntent).toHaveBeenCalledWith('Search');
    // 聚焦不复制/不增删节点：投影节点引用与数量不变
    expect(flowProps().nodes).toHaveLength(before.length);
    expect(flowProps().nodes).toEqual(before);
    // fit-view 对齐焦点可达子图
    expect(flowHelpers.fitView).toHaveBeenCalledWith(
      expect.objectContaining({
        nodes: expect.arrayContaining([{ id: 'http-a' }, { id: 'extract-a' }]),
      }),
    );
  });

  it('点击「全部」派发 focusIntent(null) 并 fit-view 全部', () => {
    const session = createSession({
      intentFocus: 'Search',
      flowProjection: { nodes: [flowNode('http-a', 'http')], edges: [] },
    });
    render(RuleFlowCanvas, { props: { session } });
    fireEvent.click(screen.getByRole('radio', { name: '全部' }));
    expect(session.focusIntent).toHaveBeenCalledWith(null);
  });
});

describe('RuleFlowCanvas：自动布局', () => {
  it('点击「一键整理布局」计算批量位置并提交一个 layout action', async () => {
    const session = createSession({
      flowProjection: {
        nodes: [flowNode('http-a', 'http'), flowNode('extract-a', 'extract')],
        edges: [],
      },
    });
    render(RuleFlowCanvas, { props: { session } });

    await fireEvent.click(screen.getByRole('button', { name: '一键整理布局' }));

    expect(session.layoutNodes).toHaveBeenCalledWith({
      'http-a': { x: 48, y: 48 },
      'extract-a': { x: 468, y: 48 },
    });
  });
});

describe('RuleFlowCanvas：palette 开关', () => {
  it('「添加节点」打开弹层，关闭按钮收起', async () => {
    const session = createSession({});
    render(RuleFlowCanvas, { props: { session } });
    expect(screen.queryByRole('dialog', { name: '添加节点' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: /添加节点/ }));
    expect(screen.getByRole('dialog', { name: '添加节点' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: '关闭' }));
    expect(screen.queryByRole('dialog', { name: '添加节点' })).toBeNull();
  });
});

beforeEach(() => {
  captured.props = undefined;
  flowHelpers.fitView.mockClear();
});
