//! NodePalette 测试：推荐/全部节点同层可达、不兼容项禁用并解释原因、
//! addNode 派发、Esc 与外部点击关闭。

import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

vi.mock('@xyflow/svelte', () => import('../../../test/xyflow-stubs/mock'));

import type { NativeRuleEditorSession } from '$lib/rules/native-authoring/session.svelte';

import NodePalette from './NodePalette.svelte';

type Session = {
  definition: { intent_exports: Record<string, { flow_entry: string } | undefined> };
  flowProjection: { nodes: unknown[]; edges: unknown[] };
  intentFocus: string | null;
  selection: string | null;
  addNode: ReturnType<typeof vi.fn>;
};

function createSession(overrides: Partial<Session> = {}): NativeRuleEditorSession {
  return {
    definition: { intent_exports: {} },
    flowProjection: { nodes: [], edges: [] },
    intentFocus: null,
    selection: null,
    addNode: vi.fn(() => 'new-node'),
    ...overrides,
  } as unknown as NativeRuleEditorSession;
}

function flowNode(id: string, kind: string) {
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

/** 读取指定分区的按钮（推荐区 / 全部节点区）。 */
function sectionButtons(container: HTMLElement, section: 'recommended' | 'all'): HTMLElement[] {
  const title = container.querySelector(`[data-palette-section="${section}"]`);
  expect(title, `缺少 ${section} 分区标题`).toBeTruthy();
  const list = title?.nextElementSibling as HTMLElement;
  return [...list.querySelectorAll('button')] as HTMLElement[];
}

describe('NodePalette：推荐与全部节点', () => {
  it('推荐区展示兼容项，全部节点区禁用不兼容项并解释原因', () => {
    const session = createSession({
      selection: 'extract-a',
      flowProjection: { nodes: [flowNode('extract-a', 'extract')], edges: [] },
    });
    const { container } = render(NodePalette, { props: { session, onClose: vi.fn() } });

    // extract.json 可接入 mapper/merge/condition/loop/js → 推荐区 5 项
    const recommended = sectionButtons(container, 'recommended');
    expect(recommended.length).toBeGreaterThan(0);
    expect(recommended.map((b) => b.textContent)).toEqual(
      expect.arrayContaining([expect.stringContaining('映射')]),
    );

    // 全部节点区：http/extract 不兼容且禁用，原因可见
    const all = sectionButtons(container, 'all');
    expect(all).toHaveLength(7);
    const httpButton = all.find((b) => b.textContent?.includes('HTTP 请求'));
    expect(httpButton?.hasAttribute('disabled')).toBe(true);
    expect(screen.getAllByText(/端口类型不兼容/).length).toBeGreaterThan(0);
  });

  it('无选中节点时七类全部可添加（推荐区不展示，避免重复列表）', () => {
    const session = createSession({});
    const { container } = render(NodePalette, { props: { session, onClose: vi.fn() } });
    expect(container.querySelector('[data-palette-section="recommended"]')).toBeNull();
    const all = sectionButtons(container, 'all');
    expect(all).toHaveLength(7);
    for (const button of all) {
      expect(button.hasAttribute('disabled')).toBe(false);
    }
  });

  it('意图焦点提示展示当前意图', () => {
    const session = createSession({ intentFocus: 'Search' });
    render(NodePalette, { props: { session, onClose: vi.fn() } });
    expect(screen.getByText(/当前聚焦：搜索/)).toBeTruthy();
  });
});

describe('NodePalette：添加节点', () => {
  it('点击推荐项派发 session.addNode 并关闭', () => {
    const session = createSession({
      selection: 'extract-a',
      flowProjection: { nodes: [flowNode('extract-a', 'extract')], edges: [] },
    });
    const onClose = vi.fn();
    const { container } = render(NodePalette, { props: { session, onClose } });
    const mapperButton = sectionButtons(container, 'recommended').find((b) =>
      b.textContent?.includes('映射'),
    );
    fireEvent.click(mapperButton as HTMLElement);
    expect(session.addNode).toHaveBeenCalledWith('mapper');
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('点击不兼容项不派发', () => {
    const session = createSession({
      selection: 'extract-a',
      flowProjection: { nodes: [flowNode('extract-a', 'extract')], edges: [] },
    });
    const onClose = vi.fn();
    const { container } = render(NodePalette, { props: { session, onClose } });
    const httpButton = sectionButtons(container, 'all').find((b) =>
      b.textContent?.includes('HTTP 请求'),
    );
    fireEvent.click(httpButton as HTMLElement);
    expect(session.addNode).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe('NodePalette：关闭路径', () => {
  it('Esc 键关闭', () => {
    const session = createSession({});
    const onClose = vi.fn();
    render(NodePalette, { props: { session, onClose } });
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('点击面板外部关闭', () => {
    const session = createSession({});
    const onClose = vi.fn();
    render(NodePalette, { props: { session, onClose } });
    fireEvent.pointerDown(document.body);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('点击面板内部非交互区不关闭', () => {
    const session = createSession({});
    const onClose = vi.fn();
    const { container } = render(NodePalette, { props: { session, onClose } });
    fireEvent.click(container.querySelector('header') as HTMLElement);
    expect(onClose).not.toHaveBeenCalled();
  });
});
