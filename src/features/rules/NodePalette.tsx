//! 添加节点弹层：按当前选中节点与意图焦点给出兼容推荐。
//!
//! 不兼容项禁用并解释原因，但「全部节点」仍同层可达 —— 推荐是引导，不是限制。
//! 支持点击添加（键盘可达）与拖拽落点添加（落点即位置，省掉二次拖动）。

import { Panel, useReactFlow } from '@xyflow/react';
import { useCallback, useEffect, useRef } from 'react';
import { Icon } from '@/components/Icon';
import { Button } from '@/components/ui/button';
import { cn } from '@/shared/utils';
import { useMessages } from '@/shared/i18n/messages';
import type { FlowNodeKind } from '@/shared/tauri/rules';
import {
  flowEntryByIntentFromDefinition,
  recommendKinds,
  type KindRecommendation,
} from './model/connection-gate';
import { nodeIcon } from './model/meta';
import { selectFlowProjection } from './model/session';
import { intentLabel, nodeKindDescription, nodeKindLabel, recommendBlockerText } from './labels';
import { useRuleEditorSession, useRuleEditorSessionStore } from './use-session';

/** 拖拽载荷的 MIME；用私有类型避免和外部拖入的内容混淆。 */
export const PALETTE_DRAG_TYPE = 'application/x-lanjing-rule-node';

export function NodePalette({ onClose }: { onClose: () => void }) {
  const m = useMessages();
  const store = useRuleEditorSessionStore();
  const panelRef = useRef<HTMLDivElement>(null);

  const projection = useRuleEditorSession(selectFlowProjection);
  const definition = useRuleEditorSession((state) => state.core.definition);
  const selection = useRuleEditorSession((state) => state.core.selection);
  const intentFocus = useRuleEditorSession((state) => state.core.intentFocus);

  const recommendations = recommendKinds({
    nodes: projection.nodes.map((node) => ({ id: node.id, kind: node.data.kind })),
    edges: projection.edges.map((edge) => edge.data.edge),
    selection,
    focusEntry: intentFocus
      ? (flowEntryByIntentFromDefinition(definition)[intentFocus] ?? null)
      : null,
  });
  const compatible = recommendations.filter((item) => item.compatible);
  // 推荐区只在确实起到过滤作用时出现，否则与「全部节点」完全重复。
  const showRecommended = compatible.length > 0 && compatible.length < recommendations.length;

  // Esc 或点击面板外关闭。
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    const onPointer = (event: PointerEvent) => {
      const panel = panelRef.current;
      if (panel && event.target instanceof Node && !panel.contains(event.target)) onClose();
    };
    document.addEventListener('keydown', onKey);
    document.addEventListener('pointerdown', onPointer);
    return () => {
      document.removeEventListener('keydown', onKey);
      document.removeEventListener('pointerdown', onPointer);
    };
  }, [onClose]);

  const addNode = useCallback(
    (kind: FlowNodeKind) => {
      store.getState().addNode(kind);
      onClose();
    },
    [store, onClose],
  );

  const renderItem = (item: KindRecommendation) => (
    <li key={item.kind}>
      <Button
        variant="ghost"
        size="sm"
        disabled={!item.compatible}
        draggable={item.compatible}
        onDragStart={(event) => {
          event.dataTransfer.setData(PALETTE_DRAG_TYPE, item.kind);
          event.dataTransfer.effectAllowed = 'copy';
        }}
        className={cn(
          'h-auto w-full items-start justify-start gap-2 border px-2.5 py-1.5 text-left',
          item.compatible
            ? 'border-transparent bg-surface-2/60 hover:border-hairline-strong hover:bg-surface-2'
            : 'cursor-not-allowed border-transparent opacity-55',
        )}
        onClick={() => addNode(item.kind)}
      >
        <Icon
          name={nodeIcon(item.kind)}
          className={cn(
            'mt-0.5 shrink-0',
            item.compatible ? 'text-lantern-strong' : 'text-ink-subtle',
          )}
        />
        <span className="min-w-0">
          <span className="block font-medium text-ink">{nodeKindLabel(m, item.kind)}</span>
          <span className="mt-0.5 block text-[11px] leading-3.5 text-ink-muted">
            {nodeKindDescription(m, item.kind)}
          </span>
        </span>
      </Button>
      {item.blockers.length > 0 ? (
        <p className="mt-1 px-2 text-[11px] leading-3.5 text-ink-subtle">
          {item.blockers.map((blocker) => recommendBlockerText(m, blocker)).join('；')}
        </p>
      ) : null}
    </li>
  );

  return (
    <Panel position="top-right">
      <div
        ref={panelRef}
        role="dialog"
        aria-label={m.rules_palette_title()}
        className="w-64 border border-hairline-strong bg-surface-1/95 p-2 backdrop-blur-(--material-blur)"
      >
        <header className="flex items-center justify-between px-2 pt-1 pb-1.5">
          <h2 className="font-medium text-ink">{m.rules_palette_title()}</h2>
          <Button variant="ghost" size="icon-xs" aria-label={m.action_close()} onClick={onClose}>
            <Icon name="x" />
          </Button>
        </header>

        <div className="max-h-[min(60vh,26rem)] overflow-y-auto px-1 pb-1">
          {showRecommended ? (
            <>
              <p className="px-1 pt-1 pb-1 text-[11px] font-medium tracking-wide text-ink-subtle">
                {m.rules_palette_recommended()}
              </p>
              <ul className="space-y-1">{compatible.map(renderItem)}</ul>
            </>
          ) : null}

          <p className="px-1 pt-2 pb-1 text-[11px] font-medium tracking-wide text-ink-subtle">
            {m.rules_palette_all()}
          </p>
          <ul className="space-y-1">{recommendations.map(renderItem)}</ul>
        </div>

        <footer className="border-t border-hairline px-2 pt-1.5 pb-0.5">
          <p className="text-[11px] leading-3.5 text-ink-subtle">
            {intentFocus
              ? m.rules_palette_focus_hint({ intent: intentLabel(m, intentFocus) })
              : m.rules_palette_all_hint()}
          </p>
        </footer>
      </div>
    </Panel>
  );
}

/** 画布侧的拖拽落点处理；把 palette 的拖拽转成带位置的 addNode。 */
export function usePaletteDrop() {
  const store = useRuleEditorSessionStore();
  const { screenToFlowPosition } = useReactFlow();

  const onDragOver = useCallback((event: React.DragEvent) => {
    if (!event.dataTransfer.types.includes(PALETTE_DRAG_TYPE)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'copy';
  }, []);

  const onDrop = useCallback(
    (event: React.DragEvent) => {
      const kind = event.dataTransfer.getData(PALETTE_DRAG_TYPE) as FlowNodeKind | '';
      if (!kind) return;
      event.preventDefault();
      const state = store.getState();
      const nodeId = state.addNode(kind);
      const position = screenToFlowPosition({ x: event.clientX, y: event.clientY });
      state.moveNode(nodeId, position);
    },
    [store, screenToFlowPosition],
  );

  return { onDragOver, onDrop };
}
