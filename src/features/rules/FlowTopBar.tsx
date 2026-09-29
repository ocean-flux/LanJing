//! 画布顶栏：意图聚焦切换 + 自动布局 + 添加节点入口。
//!
//! 聚焦只改 adapter 投影的 focused/dimmed 标记并对齐视口，绝不复制节点。

import { Panel, useReactFlow } from '@xyflow/react';
import { useCallback, useState } from 'react';
import { Icon } from '@/components/Icon';
import { Button } from '@/components/ui/button';
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group';
import { cn } from '@/shared/utils';
import { useMessages } from '@/shared/i18n/messages';
import type { StandardIntent } from '@/shared/tauri/rules';
import { flowEntryByIntentFromDefinition, reachableNodes } from './model/connection-gate';
import { STANDARD_INTENTS } from './model/intents';
import { layoutNativeRuleFlow, type FlowNodeDimensions } from './model/flow-layout';
import { selectFlowProjection } from './model/session';
import { intentLabel } from './labels';
import { ShortcutHints } from './ShortcutHints';
import { useRuleEditorSession, useRuleEditorSessionStore } from './use-session';

const TOGGLE_ITEM_CLASS =
  'shrink-0 border border-hairline bg-surface-1 text-ink-muted hover:bg-surface-2 hover:text-ink data-pressed:border-lantern-strong/50 data-pressed:bg-lantern-soft data-pressed:text-ink';

export function FlowTopBar({
  paletteOpen,
  onTogglePalette,
  onLayoutApplied,
}: {
  paletteOpen: boolean;
  onTogglePalette: () => void;
  /** 自动布局写入新坐标的时机；画布据此开一段位移过渡。 */
  onLayoutApplied: () => void;
}) {
  const m = useMessages();
  const store = useRuleEditorSessionStore();
  const flow = useReactFlow();
  const [layoutPending, setLayoutPending] = useState(false);

  const intentFocus = useRuleEditorSession((state) => state.core.intentFocus);
  const nodeCount = useRuleEditorSession((state) => state.core.definition.flow.nodes.length);

  /** 聚焦切换：派发 focusIntent，并把可达子图带进视野。 */
  const focusOn = useCallback(
    (intent: StandardIntent | null) => {
      const state = store.getState();
      state.focusIntent(intent);
      const entry = intent
        ? (flowEntryByIntentFromDefinition(state.core.definition)[intent] ?? null)
        : null;
      if (!entry) {
        void flow.fitView({ padding: 0.3, duration: 240 });
        return;
      }
      const reachable = reachableNodes(entry, state.core.definition.flow.edges);
      void flow.fitView({
        nodes: [...reachable].map((id) => ({ id })),
        padding: 0.3,
        duration: 240,
      });
    },
    [store, flow],
  );

  /** 自动布局用实测节点尺寸；ELK 只有拿到真实盒子才能排得紧凑。 */
  const runAutoLayout = useCallback(async () => {
    if (layoutPending || nodeCount === 0) return;
    setLayoutPending(true);
    try {
      const state = store.getState();
      const dimensions: FlowNodeDimensions = Object.fromEntries(
        selectFlowProjection(state).nodes.flatMap((node) => {
          const internal = flow.getInternalNode(node.id);
          const width = internal?.measured.width ?? internal?.width;
          const height = internal?.measured.height ?? internal?.height;
          return width || height ? [[node.id, { width, height }]] : [];
        }),
      );
      const positions = await layoutNativeRuleFlow(state.core.definition, dimensions);
      onLayoutApplied();
      store.getState().layoutNodes(positions);
      // 等 xyflow 应用完新坐标再对齐视口。
      requestAnimationFrame(() => void flow.fitView({ padding: 0.24, duration: 240 }));
    } finally {
      setLayoutPending(false);
    }
  }, [layoutPending, nodeCount, store, flow, onLayoutApplied]);

  return (
    <Panel position="top-left">
      <div className="flex max-w-full items-center gap-1.5">
        <ToggleGroup
          value={[intentFocus ?? 'all']}
          onValueChange={(value) => {
            const [next] = value;
            if (!next || next === 'all') {
              focusOn(null);
              return;
            }
            if (STANDARD_INTENTS.includes(next as StandardIntent)) focusOn(next as StandardIntent);
          }}
          className="flex max-w-full items-center gap-1 overflow-x-auto border border-hairline bg-surface-1/90 p-1 backdrop-blur-(--material-blur)"
          aria-label={m.rules_canvas_focus_label()}
        >
          <ToggleGroupItem value="all" size="sm" className={TOGGLE_ITEM_CLASS}>
            {m.rules_canvas_focus_all()}
          </ToggleGroupItem>
          {STANDARD_INTENTS.map((intent) => (
            <ToggleGroupItem key={intent} value={intent} size="sm" className={TOGGLE_ITEM_CLASS}>
              {intentLabel(m, intent)}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>

        <Button
          variant="outline"
          size="sm"
          aria-busy={layoutPending}
          disabled={layoutPending || nodeCount === 0}
          title={m.rules_canvas_auto_layout()}
          onClick={() => void runAutoLayout()}
        >
          <Icon name={layoutPending ? 'arrow-clockwise' : 'tree-structure'} />
          <span>{m.rules_canvas_auto_layout()}</span>
        </Button>

        <ShortcutHints />

        <Button
          variant={paletteOpen ? 'secondary' : 'outline'}
          size="sm"
          aria-expanded={paletteOpen}
          aria-haspopup="dialog"
          className={cn('shrink-0', paletteOpen && 'border-lantern-strong/50 bg-lantern-soft')}
          onClick={onTogglePalette}
        >
          <Icon name="plus" />
          <span>{m.rules_palette_title()}</span>
        </Button>
      </div>
    </Panel>
  );
}
