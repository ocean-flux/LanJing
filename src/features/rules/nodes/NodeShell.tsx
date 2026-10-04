//! 节点卡外壳：扁平卡片、端口行与诊断状态的唯一视觉 owner。
//!
//! 端口数据来自 `getNodePorts`；画布只负责连接 gate 与语义 action。
//! 端口行与 Handle 是两套元素：行负责可读与键盘可达，Handle 负责实际连线。

import { Handle, Position, useUpdateNodeInternals } from '@xyflow/react';
import { useEffect, useRef, type ReactNode } from 'react';
import { Icon } from '@/components/Icon';
import { cn } from '@/shared/utils';
import { useMessages } from '@/shared/i18n/messages';
import type { FlowNodeKind, InstallDiagnostic, StandardIntent } from '@/shared/tauri/rules';
import type { FlowHandleDirection } from '../model/flow-adapter';
import { nodeIcon } from '../model/meta';
import {
  portPositionStyle,
  type InputPortDef,
  type OutputPortDef,
  type PortSide,
  type PortType,
} from '../model/ports';
import { intentLabel, nodeKindLabel, portLabelText } from '../labels';

/** Port value kind 的短标记；是 compiler 术语，不本地化。 */
const PORT_TYPE_LABELS: Record<PortType, string> = {
  intent_input: 'IntentInput',
  http_response: 'HTTP',
  json: 'JSON',
  raw: 'RAW',
  delta: 'DELTA',
  loop_binding: 'LoopBinding',
};

const SIDE_POSITION: Record<PortSide, Position> = {
  left: Position.Left,
  right: Position.Right,
  top: Position.Top,
  bottom: Position.Bottom,
};

type PortDef = InputPortDef | OutputPortDef;

/** 默认空集提到模块级，避免每次渲染都造新数组破坏引用相等。 */
const NO_DIAGNOSTICS: readonly InstallDiagnostic[] = [];
const NO_INTENTS: readonly StandardIntent[] = [];

function portTypeLabel(port: PortDef): string {
  const types = 'accepts' in port ? port.accepts : [port.emits];
  return types.map((type) => PORT_TYPE_LABELS[type]).join(' / ');
}

/** 把 `top: 40%;` 这类内联样式串解析成 React 的 style 对象。 */
function positionStyle(side: PortSide, index: number, count: number) {
  const raw = portPositionStyle(side, index, count);
  const [property, value] = raw.replace(';', '').split(':');
  return property.trim() === 'top' ? { top: value.trim() } : { left: value.trim() };
}

/**
 * 节点尺寸变化时让 xyflow 重测 handle 位置。
 *
 * 端口行是绝对定位的，配置变化会改节点高度；不重测的话连线端点会漂移。
 */
function useHandleRemeasure(nodeId: string) {
  const ref = useRef<HTMLElement>(null);
  const updateNodeInternals = useUpdateNodeInternals();

  useEffect(() => {
    const element = ref.current;
    if (!element) return;

    let queued = false;
    let disposed = false;
    const refresh = () => {
      if (disposed || queued) return;
      queued = true;
      queueMicrotask(() => {
        queued = false;
        if (!disposed) updateNodeInternals(nodeId);
      });
    };

    const resizeObserver = new ResizeObserver(refresh);
    resizeObserver.observe(element);
    const mutationObserver = new MutationObserver(refresh);
    mutationObserver.observe(element, { childList: true, subtree: true });
    refresh();

    return () => {
      disposed = true;
      resizeObserver.disconnect();
      mutationObserver.disconnect();
    };
  }, [nodeId, updateNodeInternals]);

  return ref;
}

export type NodeShellProps = {
  nodeId: string;
  kind: FlowNodeKind;
  /** 配置摘要（已本地化的一行文本）。 */
  summary: string;
  /** 节点输入端口，顺序只影响展示，不承载语义。 */
  inputs: readonly InputPortDef[];
  /** 节点输出端口，顺序只影响展示，不承载语义。 */
  outputs: readonly OutputPortDef[];
  diagnostics?: readonly InstallDiagnostic[];
  /** 意图焦点可达（高亮态）。 */
  focused?: boolean;
  /** 意图焦点不可达（淡化态）。 */
  dimmed?: boolean;
  selected?: boolean;
  /** 附加配置行。 */
  extra?: ReactNode;
  /** 以该节点为入口的标准意图。 */
  entryIntents?: readonly StandardIntent[];
  /** 端口选择回调；不改变节点选中态。 */
  onPortSelect?: (direction: FlowHandleDirection, id: string) => void;
  selectedPort?: { direction: FlowHandleDirection; id: string };
};

export function NodeShell({
  nodeId,
  kind,
  summary,
  inputs,
  outputs,
  diagnostics = NO_DIAGNOSTICS,
  focused = true,
  dimmed = false,
  selected = false,
  extra,
  entryIntents = NO_INTENTS,
  onPortSelect,
  selectedPort,
}: NodeShellProps) {
  const m = useMessages();
  const ref = useHandleRemeasure(nodeId);

  const errorCount = diagnostics.filter((item) => item.severity === 'error').length;
  const warningCount = diagnostics.filter((item) => item.severity === 'warning').length;
  const label = nodeKindLabel(m, kind);
  const entryLabels = entryIntents.map((intent) => intentLabel(m, intent)).join('、');

  const sidePorts = (side: PortSide): PortDef[] =>
    [...inputs, ...outputs].filter((port) => port.side === side);

  const styleFor = (port: PortDef, direction: FlowHandleDirection) => {
    const ports = sidePorts(port.side);
    const index = ports.findIndex(
      (item) =>
        item.id === port.id &&
        ('accepts' in item ? direction === 'target' : direction === 'source'),
    );
    return positionStyle(port.side, Math.max(index, 0), ports.length);
  };

  const ariaFor = (port: PortDef): string =>
    'accepts' in port
      ? m.rules_port_aria_input({
          label: portLabelText(m, port.label),
          types: portTypeLabel(port),
        })
      : m.rules_port_aria_output({
          label: portLabelText(m, port.label),
          types: portTypeLabel(port),
        });

  const select = (direction: FlowHandleDirection, id: string) => onPortSelect?.(direction, id);

  const renderPortRow = (port: PortDef, direction: FlowHandleDirection) => (
    <div
      key={`${direction}:${port.id}`}
      className={cn(
        'flow-node-port-row',
        direction === 'target' ? 'flow-node-port-row-input' : 'flow-node-port-row-output',
      )}
      style={styleFor(port, direction)}
      data-port-id={port.id}
      data-port-side={direction === 'target' ? 'input' : 'output'}
      data-port-position={port.side}
      data-port-role={port.role ?? 'data'}
      data-port-selected={selectedPort?.direction === direction && selectedPort.id === port.id}
      role="button"
      tabIndex={0}
      aria-label={ariaFor(port)}
      onClick={(event) => {
        // Handle 自己也会响应点击，避免同一次点击触发两次选择。
        if (event.target instanceof Element && event.target.closest('.react-flow__handle')) return;
        event.stopPropagation();
        select(direction, port.id);
      }}
      onKeyDown={(event) => {
        if (event.key !== 'Enter' && event.key !== ' ') return;
        event.preventDefault();
        event.stopPropagation();
        select(direction, port.id);
      }}
    >
      <span className="flow-node-port-label">{portLabelText(m, port.label)}</span>
      <span className="flow-node-port-type">{portTypeLabel(port)}</span>
    </div>
  );

  const renderHandle = (port: PortDef, direction: FlowHandleDirection) => (
    <Handle
      key={`handle:${direction}:${port.id}`}
      type={direction}
      position={SIDE_POSITION[port.side]}
      style={styleFor(port, direction)}
      id={port.id}
      className="flow-node-handle"
      data-port-side={port.side}
      data-port-direction={direction}
      data-port-role={port.role ?? 'data'}
      data-port-selected={selectedPort?.direction === direction && selectedPort.id === port.id}
      aria-label={ariaFor(port)}
      title={ariaFor(port)}
      onClick={(event) => {
        event.stopPropagation();
        select(direction, port.id);
      }}
    />
  );

  return (
    <section
      ref={ref}
      data-slot="flow-node"
      data-node-kind={kind}
      data-focused={focused}
      data-dimmed={dimmed}
      data-selected={selected}
      data-has-errors={errorCount > 0}
      data-has-warnings={warningCount > 0}
      role="group"
      aria-label={label}
      className="flow-node-card"
    >
      {entryIntents.length > 0 ? (
        <div
          className="flow-node-entry"
          data-flow-entry="true"
          aria-label={m.rules_node_entry_aria({ intents: entryLabels })}
        >
          <span className="flow-node-entry-marker" aria-hidden="true" />
          <span>{m.rules_node_entry_label()}</span>
          <span className="flow-node-entry-intents">{entryLabels}</span>
        </div>
      ) : null}

      <header className="flow-node-header">
        <div className="flex min-w-0 items-center gap-2">
          <span className="flow-node-icon" aria-hidden="true">
            <Icon name={nodeIcon(kind)} className="text-base" />
          </span>
          <div className="min-w-0">
            <p className="flow-node-kind">{kind}</p>
            <h3 className="truncate text-[13px] leading-4 font-semibold">{label}</h3>
          </div>
        </div>
        {errorCount > 0 || warningCount > 0 ? (
          <span
            className="flow-node-issue ml-auto flex shrink-0 items-center gap-1"
            role="status"
            aria-label={
              errorCount > 0
                ? m.rules_node_issue_errors({ errors: errorCount, warnings: warningCount })
                : m.rules_node_issue_warnings({ warnings: warningCount })
            }
          >
            <Icon
              name="warning-circle"
              className={errorCount > 0 ? 'text-danger' : 'text-warning'}
            />
            <span className="tabular-nums">{errorCount > 0 ? errorCount : warningCount}</span>
          </span>
        ) : null}
      </header>

      <p className="flow-node-summary">{summary}</p>

      {extra ? <div className="flow-node-extra">{extra}</div> : null}

      {inputs.length > 0 || outputs.length > 0 ? (
        <>
          <div className="flow-node-ports" aria-label={m.rules_node_ports_aria()}>
            {inputs.map((port) => renderPortRow(port, 'target'))}
            {outputs.map((port) => renderPortRow(port, 'source'))}
          </div>
          {inputs.map((port) => renderHandle(port, 'target'))}
          {outputs.map((port) => renderHandle(port, 'source'))}
        </>
      ) : null}

      {diagnostics.length > 0 ? (
        <ul className="flow-node-diagnostics" aria-label={m.rules_node_diagnostics_aria()}>
          {diagnostics.map((diagnostic) => (
            <li
              key={`${diagnostic.code}:${diagnostic.message}`}
              className={cn(
                'flex items-start gap-1.5 text-[11px] leading-4',
                diagnostic.severity === 'error'
                  ? 'text-danger'
                  : diagnostic.severity === 'warning'
                    ? 'text-warning'
                    : 'text-ink-subtle',
              )}
            >
              <Icon
                name={diagnostic.severity === 'info' ? 'check-circle' : 'warning-circle'}
                className="mt-0.5 shrink-0 text-[0.75rem]"
              />
              <span className="min-w-0">{diagnostic.message}</span>
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}
