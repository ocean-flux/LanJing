//! Loop 区域检查器：结构状态与折叠开关。
//!
//! 区域不是 Definition 里的实体，是从 flat flow graph 即时推导的；折叠只写
//! layout，语义一个字节都不动。

import { Icon } from '@/components/Icon';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
import { useMessages } from '@/shared/i18n/messages';
import type { FlowPortRef } from '@/shared/tauri/rules';
import type { LoopRegionView } from '../model/flow-adapter';
import { useRuleEditorSessionStore } from '../use-session';

function portRefText(ref: FlowPortRef | null, fallback: string): string {
  return ref ? `${ref.node_id} · ${ref.handle}` : fallback;
}

export function LoopRegionInspector({ region }: { region: LoopRegionView }) {
  const m = useMessages();
  const store = useRuleEditorSessionStore();
  const unconnected = m.rules_loop_region_unconnected();

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-start gap-2">
        <span className="flex size-7 shrink-0 items-center justify-center bg-lantern-soft text-lantern-strong">
          <Icon name="arrow-counter-clockwise" />
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-ui-sm font-semibold text-ink">{m.rules_loop_region_title()}</p>
          <code className="block truncate font-mono text-[10px] text-ink-muted">
            {region.loopNodeId}
          </code>
        </div>
        <Badge variant="outline">
          {region.status === 'valid' ? m.rules_loop_region_valid() : m.rules_loop_region_invalid()}
        </Badge>
      </div>

      <Separator />

      <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-2 text-ui-sm">
        <dt className="text-ink-muted">{m.rules_loop_region_body_entry()}</dt>
        <dd className="truncate text-right font-mono text-ink">
          {portRefText(region.bodyEntry, unconnected)}
        </dd>
        <dt className="text-ink-muted">{m.rules_loop_region_yield_source()}</dt>
        <dd className="truncate text-right font-mono text-ink">
          {portRefText(region.yieldSource, unconnected)}
        </dd>
        <dt className="text-ink-muted">{m.rules_loop_region_body_nodes()}</dt>
        <dd className="text-right font-mono text-ink">{region.bodyNodes.length}</dd>
        <dt className="text-ink-muted">{m.rules_loop_region_boundary_edges()}</dt>
        <dd className="text-right font-mono text-ink">{region.boundaryEdgeIds.length}</dd>
      </dl>

      {region.diagnostics.length > 0 ? (
        <section className="flex flex-col gap-1.5">
          <h3 className="text-ui-sm font-semibold text-ink">{m.rules_loop_region_diagnostics()}</h3>
          <ul className="flex flex-col gap-1 text-ui-sm leading-4 text-warning">
            {region.diagnostics.map((diagnostic) => (
              <li key={diagnostic.code} className="flex items-start gap-1.5">
                <Icon name="warning-circle" className="mt-0.5 shrink-0" />
                <span className="font-mono">{diagnostic.code}</span>
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      <Button
        variant="outline"
        size="sm"
        className="w-full"
        onClick={() => store.getState().collapseLoopRegion(region.loopNodeId, !region.collapsed)}
      >
        <Icon name={region.collapsed ? 'caret-down' : 'caret-up'} />
        <span>
          {region.collapsed ? m.rules_loop_region_expand() : m.rules_loop_region_collapse()}
        </span>
      </Button>

      <p className="text-ui-sm leading-4 text-ink-subtle">{m.rules_loop_region_hint()}</p>
    </div>
  );
}
