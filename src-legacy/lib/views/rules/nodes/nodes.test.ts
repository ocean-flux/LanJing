//! 节点组件行为测试：七类节点渲染的 Handle 端口集合与摘要/诊断展示。
//!
//! 通过 vi.mock('@xyflow/svelte') 以 HandleStub 替代真实 Handle（jsdom 无 provider）。

import { fireEvent, render, within } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

vi.mock('@xyflow/svelte', () => import('../../../../test/xyflow-stubs/mock'));

import HttpNode from './HttpNode.svelte';
import JsNode from './JsNode.svelte';
import ExtractNode from './ExtractNode.svelte';
import MapperNode from './MapperNode.svelte';
import MergeNode from './MergeNode.svelte';
import ConditionNode from './ConditionNode.svelte';
import LoopNode from './LoopNode.svelte';
import type { FlowNodeData } from './types';
import { getNodePorts } from './ports';
import { summarizeNode } from './summary';

/** 构造节点 data。 */
function data(
  kind: FlowNodeData['kind'],
  config: Record<string, unknown> = {},
  diagnostics: FlowNodeData['diagnostics'] = [],
): FlowNodeData {
  return {
    nodeId: 'n-1',
    kind,
    config,
    diagnostics,
    focused: true,
    dimmed: false,
    collapsed: false,
  };
}

/** 读取渲染出的 handle 端口（id → type）。 */
function readHandles(container: HTMLElement): Map<string, string> {
  const handles = new Map<string, string>();
  for (const el of container.querySelectorAll('[data-testid="flow-handle"]')) {
    handles.set(el.getAttribute('data-handle-id') ?? '', el.getAttribute('data-handle-type') ?? '');
  }
  return handles;
}

/** 读取 Handle 的侧位与相对坐标；替身不需要真实 Svelte Flow provider。 */
function readHandleLayout(
  container: HTMLElement,
): Map<string, { type: string; position: string; style: string }> {
  const layouts = new Map<string, { type: string; position: string; style: string }>();
  for (const el of container.querySelectorAll<HTMLElement>('[data-testid="flow-handle"]')) {
    layouts.set(el.getAttribute('data-handle-id') ?? '', {
      type: el.getAttribute('data-handle-type') ?? '',
      position: el.getAttribute('data-position') ?? '',
      style: el.getAttribute('style') ?? '',
    });
  }
  return layouts;
}

describe('七类节点组件的 Handle 端口集合', () => {
  it('Http：只有 source http_response，入口由规则入口标识表达', () => {
    const { container } = render(HttpNode, { props: { data: data('http') } });
    expect(readHandles(container)).toEqual(new Map([['http_response', 'source']]));
  });

  it('Js：target in + 默认 source json', () => {
    const { container } = render(JsNode, { props: { data: data('js') } });
    expect(readHandles(container)).toEqual(
      new Map([
        ['in', 'target'],
        ['json', 'source'],
      ]),
    );
  });

  it('Js：raw 配置只显示 raw source', () => {
    const { container } = render(JsNode, { props: { data: data('js', { output: 'raw' }) } });
    expect(readHandles(container)).toEqual(
      new Map([
        ['in', 'target'],
        ['raw', 'source'],
      ]),
    );
  });

  it('Extract：target source + source json', () => {
    const { container } = render(ExtractNode, { props: { data: data('extract') } });
    expect(readHandles(container)).toEqual(
      new Map([
        ['source', 'target'],
        ['json', 'source'],
      ]),
    );
  });

  it('Mapper：target in + source delta', () => {
    const { container } = render(MapperNode, { props: { data: data('mapper') } });
    expect(readHandles(container)).toEqual(
      new Map([
        ['in', 'target'],
        ['delta', 'source'],
      ]),
    );
  });

  it('Merge：多命名 target 输入 + source json', () => {
    const { container } = render(MergeNode, { props: { data: data('merge') } });
    const handles = readHandles(container);
    const ports = getNodePorts('merge', {});
    for (const input of ports.inputs) {
      expect(handles.get(input.id)).toBe('target');
    }
    expect(handles.get('json')).toBe('source');
    expect([...handles.keys()].filter((id) => id.startsWith('in:'))).toHaveLength(
      ports.inputs.length,
    );
  });

  it('Condition：target in + 多 branch source 输出', () => {
    const { container } = render(ConditionNode, { props: { data: data('condition') } });
    const handles = readHandles(container);
    const ports = getNodePorts('condition', {});
    expect(handles.get('in')).toBe('target');
    for (const output of ports.outputs) {
      expect(handles.get(output.id)).toBe('source');
    }
    expect([...handles.keys()].filter((id) => id.startsWith('branch:'))).toHaveLength(
      ports.outputs.length,
    );
  });

  it('Loop：target in|yield + source body|done', () => {
    const { container } = render(LoopNode, { props: { data: data('loop') } });
    expect(readHandles(container)).toEqual(
      new Map([
        ['in', 'target'],
        ['yield', 'target'],
        ['body', 'source'],
        ['done', 'source'],
      ]),
    );
  });
});

describe('节点端口侧位与相对定位', () => {
  it('默认数据输入在左、数据输出在右，Handle 直接挂在节点根 section', () => {
    const { container } = render(JsNode, { props: { data: data('js') } });
    const layouts = readHandleLayout(container);

    expect(layouts.get('in')).toEqual({ type: 'target', position: 'left', style: 'top: 50%;' });
    expect(layouts.get('json')).toEqual({
      type: 'source',
      position: 'right',
      style: 'top: 50%;',
    });
    for (const handle of container.querySelectorAll('[data-testid="flow-handle"]')) {
      expect(handle.parentElement?.getAttribute('data-slot')).toBe('flow-node');
      expect(handle.getAttribute('aria-label')).toBeTruthy();
    }
  });

  it('多输入按相对比例分布，Condition 分支仍从右侧输出', () => {
    const { container: mergeContainer } = render(MergeNode, { props: { data: data('merge') } });
    const mergeLayouts = readHandleLayout(mergeContainer);
    expect(mergeLayouts.get('in:0')).toMatchObject({
      position: 'left',
      style: 'top: 33.33333333333333%;',
    });
    expect(mergeLayouts.get('in:1')).toMatchObject({
      position: 'left',
      style: 'top: 66.66666666666666%;',
    });

    const { container: conditionContainer } = render(ConditionNode, {
      props: { data: data('condition') },
    });
    const conditionLayouts = readHandleLayout(conditionContainer);
    expect(conditionLayouts.get('branch:0')).toMatchObject({
      position: 'right',
      style: 'top: 33.33333333333333%;',
    });
    expect(conditionLayouts.get('branch:1')).toMatchObject({
      position: 'right',
      style: 'top: 66.66666666666666%;',
    });
  });

  it('Loop collection/yield/body/done 分别位于左/上/下/右', () => {
    const { container } = render(LoopNode, { props: { data: data('loop') } });
    expect(readHandleLayout(container)).toEqual(
      new Map([
        ['in', { type: 'target', position: 'left', style: 'top: 50%;' }],
        ['yield', { type: 'target', position: 'top', style: 'left: 50%;' }],
        ['body', { type: 'source', position: 'bottom', style: 'left: 50%;' }],
        ['done', { type: 'source', position: 'right', style: 'top: 50%;' }],
      ]),
    );
  });
});

describe('节点卡展示', () => {
  it('端口行独立选择，不触发节点选择', () => {
    const onPortSelect = vi.fn();
    const { container } = render(JsNode, {
      props: { data: { ...data('js'), onPortSelect } },
    });
    const output = container.querySelector('[data-port-side="output"]') as HTMLElement;
    fireEvent.click(output);
    expect(onPortSelect).toHaveBeenCalledWith('source', 'json');
    expect(output.dataset.portSelected).not.toBe('true');
  });

  it('Handle 单击选择对应语义端口', () => {
    const onPortSelect = vi.fn();
    const { container } = render(JsNode, {
      props: { data: { ...data('js'), onPortSelect } },
    });
    const handle = container.querySelector('[data-testid="flow-handle"][data-handle-id="json"]');
    expect(handle).toBeTruthy();

    fireEvent.click(handle as HTMLElement);

    expect(onPortSelect).toHaveBeenCalledWith('source', 'json');
  });

  it('展示中文类型标题与配置摘要', () => {
    const { container } = render(HttpNode, {
      props: { data: data('http', { method: 'GET', url: 'https://example.test/api' }) },
    });
    expect(container.textContent).toContain('HTTP 请求');
    expect(container.textContent).toContain('GET https://example.test/api');
  });

  it('摘要缺省回退到类型化文案', () => {
    const { container } = render(MapperNode, { props: { data: data('mapper', {}) } });
    expect(container.textContent).toContain('JSON 映射为 delta');
  });

  it('诊断标记：错误计数与消息展示，info 不计数', () => {
    const { container } = render(JsNode, {
      props: {
        data: data('js', {}, [
          { code: 'E1', severity: 'error', message: '脚本语法错误' },
          { code: 'W1', severity: 'warning', message: '未使用的变量' },
          { code: 'I1', severity: 'info', message: '已就绪' },
        ]),
      },
    });
    const card = container.querySelector('[data-node-kind="js"]') as HTMLElement;
    expect(card.dataset.hasErrors).toBe('true');
    expect(card.dataset.hasWarnings).toBe('true');
    expect(within(card).getByText('脚本语法错误')).toBeTruthy();
    expect(within(card).getByText('未使用的变量')).toBeTruthy();
    expect(card.textContent).toContain('已就绪');
  });

  it('选中态与淡化态通过 data 属性表达', () => {
    const { container } = render(HttpNode, {
      props: {
        data: { ...data('http'), focused: false, dimmed: true },
        selected: true,
      },
    });
    const card = container.querySelector('[data-node-kind="http"]') as HTMLElement;
    expect(card.dataset.selected).toBe('true');
    expect(card.dataset.dimmed).toBe('true');
    expect(card.dataset.focused).toBe('false');
  });
});

describe('summarizeNode：配置摘要', () => {
  it('HTTP：method + url（缺省 GET）', () => {
    expect(summarizeNode('http', { method: 'post', url: 'https://a.test' })).toBe(
      'POST https://a.test',
    );
    expect(summarizeNode('http', { url: 'https://a.test' })).toBe('GET https://a.test');
    expect(summarizeNode('http', {})).toBe('发起 HTTP 请求');
  });

  it('JS：首行代码片段；Extract/Mapper 读取 canonical 字段', () => {
    expect(summarizeNode('js', { code: '  \n return book.name;  ' })).toBe('return book.name;');
    expect(summarizeNode('js', {})).toBe('运行 JS 脚本');
    expect(
      summarizeNode('extract', {
        rules: [{ CssSelector: { selector: '.title', extract_type: 'Text', regex_clean: null } }],
      }),
    ).toBe('1 条提取规则');
    expect(summarizeNode('mapper', { identity_fields: ['title'] })).toBe('1 个身份字段');
  });

  it('Merge/Condition/Loop：类型化摘要', () => {
    expect(summarizeNode('merge', {})).toBe('合并多条输入流');
    expect(summarizeNode('condition', { branches: ['match', 'miss', 'fallback'] })).toBe(
      '3 个条件分支',
    );
    expect(summarizeNode('condition', {})).toBe('按条件分流');
    expect(summarizeNode('loop', {})).toBe('遍历集合逐项处理');
  });
});
