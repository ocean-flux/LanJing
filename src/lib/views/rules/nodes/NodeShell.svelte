<script lang="ts">
  //! 节点卡外壳：扁平卡片、端口行与诊断状态的唯一视觉 owner。
  //! 端口数据来自 getNodePorts；画布仍只负责连接 gate 与语义 action。

  import { Handle, Position, useUpdateNodeInternals } from '@xyflow/svelte';
  import type { Snippet } from 'svelte';
  import Icon from '$lib/components/Icon.svelte';
  import { cn } from '$lib/utils.js';
  import type { FlowNodeKind, StandardIntent } from '$lib/rules/native-authoring/wire';
  import type { FlowHandleDirection } from '$lib/rules/native-authoring/flow-adapter';
  import { INTENT_LABELS } from '../intents';
  import { NODE_KIND_META } from './meta';
  import {
    portPositionStyle,
    type InputPortDef,
    type OutputPortDef,
    type PortSide,
    type PortType,
  } from './ports';
  import type { NodeDiagnosticView } from './types';

  type Props = {
    kind: FlowNodeKind;
    /** 配置摘要（一行中文）。 */
    summary: string;
    /** 节点输入端口，顺序只影响展示，不承载语义。 */
    inputs: readonly InputPortDef[];
    /** 节点输出端口，顺序只影响展示，不承载语义。 */
    outputs: readonly OutputPortDef[];
    diagnostics?: NodeDiagnosticView[];
    /** 意图焦点可达（高亮态）。 */
    focused?: boolean;
    /** 意图焦点不可达（淡化态）。 */
    dimmed?: boolean;
    /** Svelte Flow 选中态。 */
    selected?: boolean;
    /** 附加配置行（可选）。 */
    extra?: Snippet;
    /** 以该节点为入口的标准意图。 */
    entryIntents?: readonly StandardIntent[];
    /** 端口选择回调；不改变节点选中态。 */
    onPortSelect?: (direction: FlowHandleDirection, id: string) => void;
    selectedPort?: { direction: FlowHandleDirection; id: string };
  };

  let {
    kind,
    summary,
    inputs,
    outputs,
    diagnostics = [],
    focused = true,
    dimmed = false,
    selected = false,
    extra,
    entryIntents = [],
    onPortSelect,
    selectedPort,
  }: Props = $props();

  const meta = $derived(NODE_KIND_META[kind]);
  const errorCount = $derived(
    diagnostics.filter((diagnostic) => diagnostic.severity === 'error').length,
  );
  const warningCount = $derived(
    diagnostics.filter((diagnostic) => diagnostic.severity === 'warning').length,
  );
  const hasIssues = $derived(errorCount > 0 || warningCount > 0);
  const entryLabels = $derived(
    entryIntents.map((intent) => INTENT_LABELS[intent] ?? intent).join('、'),
  );
  const updateNodeInternals = useUpdateNodeInternals();

  function observeNode(nodeElement: HTMLElement) {
    let refreshQueued = false;
    let disposed = false;

    const refresh = () => {
      if (disposed || refreshQueued) return;
      refreshQueued = true;
      queueMicrotask(() => {
        refreshQueued = false;
        if (!disposed) updateNodeInternals();
      });
    };
    const resizeObserver =
      typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(() => refresh());
    const mutationObserver =
      typeof MutationObserver === 'undefined'
        ? undefined
        : new MutationObserver(() => {
            refresh();
          });

    const syncResizeTargets = () => {
      if (!resizeObserver) return;
      resizeObserver.disconnect();
      resizeObserver.observe(nodeElement);
    };

    syncResizeTargets();
    mutationObserver?.observe(nodeElement, {
      childList: true,
      subtree: true,
    });
    refresh();

    return () => {
      disposed = true;
      refreshQueued = false;
      resizeObserver?.disconnect();
      mutationObserver?.disconnect();
    };
  }

  const PORT_TYPE_LABELS: Record<PortType, string> = {
    http_response: 'HTTP',
    json: 'JSON',
    raw: 'RAW',
    delta: 'DELTA',
    loop_binding: 'LoopBinding',
  };

  function portTypeLabel(port: InputPortDef | OutputPortDef): string {
    const types = 'accepts' in port ? port.accepts : [port.emits];
    return types.map((type) => PORT_TYPE_LABELS[type]).join(' / ');
  }

  function inputAriaLabel(port: InputPortDef): string {
    return `${port.label}，可接收 ${portTypeLabel(port)}`;
  }

  function outputAriaLabel(port: OutputPortDef): string {
    return `${port.label}，输出 ${portTypeLabel(port)}`;
  }

  type PortDef = InputPortDef | OutputPortDef;

  function positionForSide(side: PortSide) {
    switch (side) {
      case 'left':
        return Position.Left;
      case 'right':
        return Position.Right;
      case 'top':
        return Position.Top;
      case 'bottom':
        return Position.Bottom;
    }
  }

  function portsOnSide(side: PortSide): PortDef[] {
    return [...inputs, ...outputs].filter((port) => port.side === side);
  }

  function portStyle(side: PortSide, direction: FlowHandleDirection, id: string): string {
    const sidePorts = portsOnSide(side);
    const index = sidePorts.findIndex(
      (port) =>
        port.id === id && ('accepts' in port ? direction === 'target' : direction === 'source'),
    );
    return portPositionStyle(side, Math.max(index, 0), sidePorts.length);
  }

  function handlePortClick(event: MouseEvent, direction: FlowHandleDirection, id: string): void {
    if (event.target instanceof Element && event.target.closest('.svelte-flow__handle')) return;
    event.stopPropagation();
    onPortSelect?.(direction, id);
  }

  function handlePortKeydown(
    event: KeyboardEvent,
    direction: FlowHandleDirection,
    id: string,
  ): void {
    if (event.key !== 'Enter' && event.key !== ' ') return;
    event.preventDefault();
    event.stopPropagation();
    onPortSelect?.(direction, id);
  }

  function handleHandleClick(event: MouseEvent, direction: FlowHandleDirection, id: string): void {
    event.stopPropagation();
    onPortSelect?.(direction, id);
  }
</script>

<section
  {@attach observeNode}
  data-slot="flow-node"
  data-node-kind={kind}
  data-focused={focused}
  data-dimmed={dimmed}
  data-selected={selected}
  data-has-errors={errorCount > 0}
  data-has-warnings={warningCount > 0}
  data-has-inputs={inputs.length > 0}
  data-has-outputs={outputs.length > 0}
  role="group"
  aria-label={meta.label}
  title={meta.description}
  class={cn(
    'flow-node-card relative w-[252px] border text-ink transition-[opacity,border-color,box-shadow] duration-(--motion-fast)',
    selected && 'flow-node-card-selected',
    dimmed && 'flow-node-card-dimmed',
  )}
>
  {#if entryIntents.length > 0}
    <div class="flow-node-entry" data-flow-entry="true" aria-label={`规则入口：${entryLabels}`}>
      <span class="flow-node-entry-marker" aria-hidden="true"></span>
      <span class="flow-node-entry-label">规则入口</span>
      <span class="flow-node-entry-intents">{entryLabels}</span>
    </div>
  {/if}
  <header class="flow-node-header">
    <div class="flex min-w-0 items-center gap-2">
      <span class="flow-node-icon" aria-hidden="true">
        <Icon name={meta.icon} class="size-4" />
      </span>
      <div class="min-w-0">
        <p class="flow-node-kind">{kind}</p>
        <h3 class="truncate text-[13px] leading-4 font-semibold text-ink">{meta.label}</h3>
      </div>
    </div>
    {#if hasIssues}
      <span
        class="flow-node-issue ml-auto flex shrink-0 items-center gap-1"
        role="status"
        aria-label={errorCount > 0
          ? `${errorCount} 个错误${warningCount > 0 ? `，${warningCount} 个警告` : ''}`
          : `${warningCount} 个警告`}
      >
        <Icon
          name="warning-circle"
          class={errorCount > 0 ? 'text-[var(--danger)]' : 'text-warning'}
        />
        <span class="tabular-nums">{errorCount > 0 ? errorCount : warningCount}</span>
      </span>
    {/if}
  </header>

  <p class="flow-node-summary">{summary}</p>

  {#if extra}
    <div class="flow-node-extra">{@render extra()}</div>
  {/if}

  {#if inputs.length > 0 || outputs.length > 0}
    <div class="flow-node-ports" aria-label="节点端口">
      {#each inputs as input (input.id)}
        <div
          class="flow-node-port-row flow-node-port-row-input"
          style={portStyle(input.side, 'target', input.id)}
          data-port-id={input.id}
          data-port-side="input"
          data-port-position={input.side}
          data-port-type={portTypeLabel(input)}
          data-port-role={input.role ?? 'data'}
          data-port-selected={selectedPort?.direction === 'target' && selectedPort?.id === input.id}
          role="button"
          tabindex="0"
          aria-label={inputAriaLabel(input)}
          onclick={(event) => handlePortClick(event, 'target', input.id)}
          onkeydown={(event) => handlePortKeydown(event, 'target', input.id)}
        >
          <span class="flow-node-port-label">{input.label}</span>
          <span class="flow-node-port-type">{portTypeLabel(input)}</span>
        </div>
      {/each}

      {#each outputs as output (output.id)}
        <div
          class="flow-node-port-row flow-node-port-row-output"
          style={portStyle(output.side, 'source', output.id)}
          data-port-id={output.id}
          data-port-side="output"
          data-port-position={output.side}
          data-port-type={portTypeLabel(output)}
          data-port-role={output.role ?? 'data'}
          data-port-selected={selectedPort?.direction === 'source' &&
            selectedPort?.id === output.id}
          role="button"
          tabindex="0"
          aria-label={outputAriaLabel(output)}
          onclick={(event) => handlePortClick(event, 'source', output.id)}
          onkeydown={(event) => handlePortKeydown(event, 'source', output.id)}
        >
          <span class="flow-node-port-label">{output.label}</span>
          <span class="flow-node-port-type">{portTypeLabel(output)}</span>
        </div>
      {/each}
    </div>

    {#each inputs as input (input.id)}
      <Handle
        type="target"
        position={positionForSide(input.side)}
        style={portStyle(input.side, 'target', input.id)}
        id={input.id}
        class="flow-node-handle"
        data-port-side={input.side}
        data-port-direction="target"
        data-port-role={input.role ?? 'data'}
        data-port-selected={selectedPort?.direction === 'target' && selectedPort?.id === input.id}
        aria-label={inputAriaLabel(input)}
        title={inputAriaLabel(input)}
        onclick={(event) => handleHandleClick(event, 'target', input.id)}
      />
    {/each}

    {#each outputs as output (output.id)}
      <Handle
        type="source"
        position={positionForSide(output.side)}
        style={portStyle(output.side, 'source', output.id)}
        id={output.id}
        class="flow-node-handle"
        data-port-side={output.side}
        data-port-direction="source"
        data-port-role={output.role ?? 'data'}
        data-port-selected={selectedPort?.direction === 'source' && selectedPort?.id === output.id}
        aria-label={outputAriaLabel(output)}
        title={outputAriaLabel(output)}
        onclick={(event) => handleHandleClick(event, 'source', output.id)}
      />
    {/each}
  {/if}

  {#if diagnostics.length > 0}
    <ul class="flow-node-diagnostics" aria-label="节点诊断">
      {#each diagnostics as diagnostic (diagnostic.code + diagnostic.message)}
        <li
          class={cn(
            'flex items-start gap-1.5 text-[11px] leading-4',
            diagnostic.severity === 'error'
              ? 'text-[var(--danger)]'
              : diagnostic.severity === 'warning'
                ? 'text-warning'
                : 'text-ink-subtle',
          )}
        >
          <Icon
            name={diagnostic.severity === 'info' ? 'check-circle' : 'warning-circle'}
            class="mt-0.5 size-3 shrink-0"
          />
          <span class="min-w-0">{diagnostic.message}</span>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  /* 卡片只使用 semantic surface；端口行保持平面，避免节点内部再套卡片。 */
  .flow-node-card {
    --node-accent: var(--lantern-strong);
    min-height: 142px;
    overflow: visible;
    border-radius: 6px;
    border-color: var(--hairline-strong);
    border-top: 2px solid var(--node-accent);
    background: var(--surface-1);
    box-shadow: var(--surface-control-shadow);
  }

  .flow-node-card[data-node-kind='js'] {
    --node-accent: var(--ink-muted);
  }

  .flow-node-card[data-node-kind='condition'] {
    --node-accent: var(--lantern);
  }

  .flow-node-card[data-node-kind='loop'] {
    --node-accent: var(--lantern);
  }

  .flow-node-card-selected {
    border-color: var(--lantern-strong);
    box-shadow:
      0 0 0 2px var(--ring),
      var(--surface-control-shadow);
  }

  .flow-node-card-dimmed {
    opacity: 0.38;
  }

  .flow-node-card[data-selected='true'] {
    opacity: 1;
  }

  .flow-node-header {
    display: flex;
    min-height: 48px;
    align-items: center;
    gap: 8px;
    padding: 9px 11px 8px;
    border-bottom: 1px solid var(--hairline);
  }

  .flow-node-entry {
    display: flex;
    min-height: 25px;
    align-items: center;
    gap: 6px;
    padding: 5px 11px 4px;
    border-bottom: 1px solid var(--hairline);
    color: var(--lantern-strong);
    font-size: 10px;
    font-weight: 600;
  }

  .flow-node-entry-marker {
    width: 7px;
    height: 7px;
    flex: 0 0 auto;
    border: 2px solid var(--surface-1);
    border-radius: 9999px;
    background: var(--lantern);
    box-shadow: 0 0 0 1px var(--lantern-strong);
  }

  .flow-node-entry-intents {
    overflow: hidden;
    color: var(--ink-muted);
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .flow-node-icon {
    display: grid;
    width: 28px;
    height: 28px;
    flex: 0 0 auto;
    place-items: center;
    border: 1px solid color-mix(in oklab, var(--node-accent) 48%, var(--hairline-strong));
    border-radius: 4px;
    color: var(--node-accent);
    background: color-mix(in oklab, var(--node-accent) 12%, var(--surface-1));
  }

  .flow-node-kind {
    margin: 0 0 1px;
    color: var(--ink-subtle);
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.08em;
    line-height: 1;
    text-transform: uppercase;
  }

  .flow-node-issue {
    color: var(--ink-muted);
    font-size: 11px;
    font-weight: 600;
  }

  .flow-node-issue :global(.iconify) {
    width: 14px;
    height: 14px;
  }

  .flow-node-summary {
    min-height: 36px;
    margin: 0;
    padding: 8px 11px 7px;
    color: var(--ink-muted);
    font-size: 11px;
    line-height: 1.45;
    overflow-wrap: anywhere;
  }

  .flow-node-extra {
    margin: 0 11px 8px;
    padding-top: 7px;
    border-top: 1px solid var(--hairline);
  }

  .flow-node-ports {
    position: absolute;
    z-index: 2;
    inset: 0;
    pointer-events: none;
  }

  .flow-node-port-row {
    position: absolute;
    display: flex;
    z-index: 4;
    width: max-content;
    max-width: 180px;
    min-height: 20px;
    align-items: center;
    gap: 5px;
    padding: 2px 4px;
    border: 1px solid var(--hairline);
    border-radius: 4px;
    color: var(--ink-muted);
    background: var(--surface-panel);
    font-size: 10px;
    line-height: 1.2;
    white-space: nowrap;
    opacity: 0;
    pointer-events: none;
    transition:
      opacity var(--motion-fast) var(--motion-standard),
      transform var(--motion-fast) var(--motion-standard);
  }

  .flow-node-card[data-selected='true'] .flow-node-port-row,
  .flow-node-port-row[data-port-selected='true'],
  .flow-node-port-row:focus-visible {
    opacity: 1;
    pointer-events: auto;
  }

  .flow-node-port-row[data-port-position='left'] {
    right: calc(100% + 12px);
    justify-content: flex-end;
    transform: translateY(-50%);
  }

  .flow-node-port-row[data-port-position='right'] {
    left: calc(100% + 12px);
    justify-content: flex-start;
    transform: translateY(-50%);
  }

  .flow-node-port-row[data-port-position='top'] {
    bottom: calc(100% + 12px);
    justify-content: center;
    transform: translate(-50%, 0);
  }

  .flow-node-port-row[data-port-position='bottom'] {
    top: calc(100% + 12px);
    justify-content: center;
    transform: translate(-50%, 0);
  }

  .flow-node-port-row:hover {
    color: var(--ink);
  }

  .flow-node-port-row:focus-visible {
    z-index: 5;
    outline: 2px solid var(--ring);
    outline-offset: -1px;
  }

  .flow-node-port-row[data-port-selected='true'] {
    color: var(--ink);
    background: var(--lantern-soft);
  }

  .flow-node-port-label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .flow-node-port-type {
    flex: 0 0 auto;
    color: var(--ink-subtle);
    font-family: var(--font-mono);
    font-size: 8px;
    font-weight: 600;
    letter-spacing: 0.04em;
    white-space: nowrap;
  }

  /* Handle 保持小而清晰；连接态由 Svelte Flow 自身 class 提升可见性。 */
  :global(.flow-node-handle) {
    z-index: 3;
    width: 11px;
    height: 11px;
    min-width: 11px;
    min-height: 11px;
    border: 2px solid var(--surface-1);
    border-radius: 9999px;
    background: var(--node-accent);
    box-shadow: 0 0 0 1px color-mix(in oklab, var(--node-accent) 42%, transparent);
    transition:
      width var(--motion-fast) var(--motion-standard),
      height var(--motion-fast) var(--motion-standard),
      background-color var(--motion-fast) var(--motion-standard),
      box-shadow var(--motion-fast) var(--motion-standard);
  }

  :global(.flow-node-port-row:hover .flow-node-handle),
  :global(.flow-node-handle.connectingfrom),
  :global(.flow-node-handle.connectingto),
  :global(.flow-node-handle.valid) {
    width: 15px;
    height: 15px;
    min-width: 15px;
    min-height: 15px;
    background: var(--lantern);
    box-shadow: 0 0 0 3px var(--lantern-soft);
  }

  :global(.flow-node-handle.invalid) {
    background: var(--surface-3);
    box-shadow: 0 0 0 1px var(--hairline-strong);
  }

  :global(.flow-node-handle[data-port-role='control']) {
    background: var(--lantern);
  }

  :global(.flow-node-handle[data-port-role='binding']) {
    background: var(--ink-muted);
    box-shadow: 0 0 0 1px color-mix(in oklab, var(--ink-muted) 42%, transparent);
  }

  .flow-node-diagnostics {
    display: grid;
    gap: 4px;
    margin: 0 11px 10px;
    padding: 8px 0 0;
    border-top: 1px solid var(--hairline);
    list-style: none;
  }

  @media (pointer: coarse) {
    .flow-node-port-row {
      min-height: var(--density-touch-target);
    }

    :global(.flow-node-handle) {
      width: 14px;
      height: 14px;
      min-width: 14px;
      min-height: 14px;
    }
  }
</style>
