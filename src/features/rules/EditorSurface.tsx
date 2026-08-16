//! 已打开文档的编辑面：画布 + 诊断 + 检查器的可拖拽分栏。
//!
//! ReactFlowProvider 落在这里而不是外层：所有需要 flow 上下文的组件
//! （FlowTopBar / NodePalette / NodeShell）都在画布子树内。

import { ReactFlowProvider } from '@xyflow/react';
import { useEffect, useState } from 'react';
import { useDefaultLayout } from 'react-resizable-panels';
import { Icon } from '@/components/Icon';
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable';
import { Spinner } from '@/components/ui/spinner';
import { useMessages } from '@/shared/i18n/messages';
import { DiagnosticList } from './DiagnosticList';
import { Inspector } from './inspector/Inspector';
import { RuleFlowCanvas } from './RuleFlowCanvas';
import { sessionErrorText } from './session-error';
import { useRuleEditorSessionStore } from './use-session';

/** 分栏 panel id；持久化的 layout 以它们为键，改名会丢用户已保存的宽度。 */
const CANVAS_PANEL_ID = 'canvas';
const INSPECTOR_PANEL_ID = 'inspector';

export function EditorSurface({ documentId }: { documentId: string }) {
  const m = useMessages();
  const store = useRuleEditorSessionStore();
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading');
  const [message, setMessage] = useState('');
  // 分栏宽度是编辑器偏好，不进文档 layout：它跟着人走，不跟着规则走。
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({
    id: 'lanjing-rules-workspace',
    panelIds: [CANVAS_PANEL_ID, INSPECTOR_PANEL_ID],
    onlySaveAfterUserInteractions: true,
  });

  useEffect(() => {
    let cancelled = false;
    setStatus('loading');
    setMessage('');
    store
      .getState()
      .loadDocument(documentId)
      .then(() => {
        if (!cancelled) setStatus('ready');
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        setStatus('error');
        setMessage(sessionErrorText(m, error));
      });
    return () => {
      cancelled = true;
    };
  }, [documentId, store, m]);

  if (status === 'loading') {
    return (
      <Empty className="h-full">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <Spinner />
          </EmptyMedia>
          <EmptyTitle>{m.rules_canvas_loading()}</EmptyTitle>
        </EmptyHeader>
      </Empty>
    );
  }

  if (status === 'error') {
    return (
      <Empty className="h-full">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <Icon name="warning-circle" className="text-danger" />
          </EmptyMedia>
          <EmptyTitle>{m.rules_load_error()}</EmptyTitle>
          <EmptyDescription>{message}</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  }

  return (
    // 数值尺寸是 px，百分比要写成字符串；检查器给 px 下限，窄窗口下也放得下一列表单。
    <ResizablePanelGroup
      orientation="horizontal"
      defaultLayout={defaultLayout}
      onLayoutChanged={onLayoutChanged}
    >
      <ResizablePanel id={CANVAS_PANEL_ID} defaultSize="70%" minSize={420}>
        <div className="flex h-full min-h-0 flex-col">
          <div className="min-h-0 flex-1">
            <ReactFlowProvider>
              <RuleFlowCanvas />
            </ReactFlowProvider>
          </div>
          <DiagnosticList />
        </div>
      </ResizablePanel>
      <ResizableHandle withHandle aria-label={m.rules_inspector_resize()} />
      <ResizablePanel id={INSPECTOR_PANEL_ID} defaultSize="30%" minSize={280} maxSize={560}>
        <Inspector />
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}
