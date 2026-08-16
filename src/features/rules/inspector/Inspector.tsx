//! 检查器路由：把 core 的单一 selection token 分发到四种检查器。
//!
//! selection 是一个字符串（node id / `edge:` / `port:` / `loop-region:` 前缀），
//! 解析在 flow-adapter 里，这里只负责按 kind 取数据并渲染。

import { Icon } from '@/components/Icon';
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import { ScrollArea } from '@/components/ui/scroll-area';
import { useMessages } from '@/shared/i18n/messages';
import type { FlowNode } from '@/shared/tauri/rules';
import { parseEditorSelection } from '../model/flow-adapter';
import { selectFlowProjection } from '../model/session';
import { useRuleEditorSession } from '../use-session';
import { EdgeInspector } from './EdgeInspector';
import { LoopRegionInspector } from './LoopRegionInspector';
import { NodeInspector } from './NodeInspector';
import { PortInspector } from './PortInspector';

function InspectorEmpty() {
  const m = useMessages();
  return (
    <Empty className="h-full">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <Icon name="list-bullets" />
        </EmptyMedia>
        <EmptyTitle>{m.rules_inspector_title()}</EmptyTitle>
        <EmptyDescription>{m.rules_inspector_no_selection()}</EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}

function InspectorBody() {
  const projection = useRuleEditorSession(selectFlowProjection);
  const selectionToken = useRuleEditorSession((state) => state.core.selection);
  const definitionNodes = useRuleEditorSession((state) => state.core.definition.flow.nodes);
  const selection = parseEditorSelection(selectionToken);

  const findNode = (nodeId: string): FlowNode | null =>
    definitionNodes.find((node) => node.id === nodeId) ?? null;

  switch (selection.kind) {
    case 'node': {
      const node = projection.nodes.find((item) => item.id === selection.nodeId);
      return node ? <NodeInspector node={node} /> : <InspectorEmpty />;
    }
    case 'edge': {
      const edge = projection.edges.find((item) => item.id === selection.edgeId);
      if (!edge) return <InspectorEmpty />;
      return (
        <EdgeInspector
          edge={edge}
          sourceNode={findNode(edge.data.edge.from.node_id)}
          targetNode={findNode(edge.data.edge.to.node_id)}
        />
      );
    }
    case 'port': {
      const node = findNode(selection.nodeId);
      if (!node) return <InspectorEmpty />;
      return (
        <PortInspector
          node={node}
          direction={selection.direction}
          handle={selection.handle}
          edges={projection.edges}
        />
      );
    }
    case 'loopRegion': {
      const region = projection.loopRegions.find(
        (item) => item.loopNodeId === selection.loopNodeId,
      );
      return region ? <LoopRegionInspector region={region} /> : <InspectorEmpty />;
    }
    case 'none': {
      return <InspectorEmpty />;
    }
  }
}

export function Inspector() {
  const m = useMessages();
  return (
    <aside
      aria-label={m.rules_inspector_title()}
      className="flex h-full min-h-0 flex-col border-l border-hairline bg-surface-1"
    >
      <header className="flex h-(--density-row) shrink-0 items-center border-b border-hairline px-(--density-panel-padding)">
        <h2 className="font-medium">{m.rules_inspector_title()}</h2>
      </header>
      <ScrollArea className="min-h-0 flex-1">
        <div className="p-(--density-panel-padding)">
          <InspectorBody />
        </div>
      </ScrollArea>
    </aside>
  );
}
