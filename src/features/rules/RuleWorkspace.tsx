//! 规则工作区：文档切换 + 编辑器 session 的宿主。
//!
//! 每份文档一个 session 实例（用 documentId 作 key 重挂载），切换文档不会
//! 继承上一份的撤销历史。工具栏只调 session 的 action，不直接接触 core。

import { ReactFlowProvider } from '@xyflow/react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useDefaultLayout } from 'react-resizable-panels';
import { useNavigate } from 'react-router-dom';
import { toast } from 'sonner';
import { Icon } from '@/components/Icon';
import { PageToolbar } from '@/components/PageToolbar';
import { Button } from '@/components/ui/button';
import { ButtonGroup } from '@/components/ui/button-group';
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select';
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable';
import { Spinner } from '@/components/ui/spinner';
import { useMessages } from '@/shared/i18n/messages';
import { listNativeRuleDocuments, type NativeRuleDocumentSummary } from '@/shared/tauri/rules';
import { RuleFlowCanvas } from './RuleFlowCanvas';
import { DiagnosticList } from './DiagnosticList';
import { Inspector } from './inspector/Inspector';
import {
  createRuleEditorSession,
  selectCanRedo,
  selectCanUndo,
  selectHasUnsavedChanges,
  selectIsSaving,
  SessionError,
  type RuleEditorSession,
} from './model/session';
import {
  RuleEditorSessionProvider,
  useRuleEditorSession,
  useRuleEditorSessionStore,
} from './use-session';

type Messages = ReturnType<typeof useMessages>;

/** 分栏 panel id；持久化的 layout 以它们为键，改名会丢用户已保存的宽度。 */
const CANVAS_PANEL_ID = 'canvas';
const INSPECTOR_PANEL_ID = 'inspector';

/** SessionError 的稳定 code → 本地化文案。 */
function sessionErrorText(m: Messages, error: unknown): string {
  if (!(error instanceof SessionError)) {
    return error instanceof Error ? error.message : String(error);
  }
  switch (error.code) {
    case 'document_not_found': {
      return m.rules_error_document_not_found();
    }
    case 'document_semantic_missing': {
      return m.rules_error_semantic_missing();
    }
    case 'document_semantic_unsaved': {
      return m.rules_unsaved_changes();
    }
    case 'document_validation_required': {
      return m.rules_validation_required();
    }
    case 'document_not_open': {
      return m.rules_error_document_not_open();
    }
  }
}

/** 编辑器工具栏：撤销/重做 + 校验 + 保存。 */
function EditorToolbar() {
  const m = useMessages();
  const store = useRuleEditorSessionStore();
  const canUndo = useRuleEditorSession(selectCanUndo);
  const canRedo = useRuleEditorSession(selectCanRedo);
  const isSaving = useRuleEditorSession(selectIsSaving);
  const hasUnsaved = useRuleEditorSession(selectHasUnsavedChanges);
  const [validating, setValidating] = useState(false);

  const run = useCallback(
    async (action: () => Promise<unknown>, success: string) => {
      try {
        await action();
        toast.success(success);
      } catch (error) {
        toast.error(sessionErrorText(m, error));
      }
    },
    [m],
  );

  return (
    <ButtonGroup>
      <Button
        variant="outline"
        size="icon-sm"
        disabled={!canUndo}
        aria-label={m.rules_undo()}
        title={m.rules_undo()}
        onClick={() => store.getState().undo()}
      >
        <Icon name="arrow-counter-clockwise" />
      </Button>
      <Button
        variant="outline"
        size="icon-sm"
        disabled={!canRedo}
        aria-label={m.rules_redo()}
        title={m.rules_redo()}
        onClick={() => store.getState().redo()}
      >
        <Icon name="arrow-clockwise" />
      </Button>
      <Button
        variant="outline"
        size="sm"
        disabled={validating || hasUnsaved}
        title={hasUnsaved ? m.rules_unsaved_changes() : m.rules_validate()}
        onClick={() => {
          setValidating(true);
          void run(() => store.getState().validate(), m.rules_validated()).finally(() =>
            setValidating(false),
          );
        }}
      >
        {validating ? <Spinner /> : <Icon name="check-circle" />}
        <span>{m.rules_validate()}</span>
      </Button>
      <Button
        size="sm"
        disabled={!hasUnsaved || isSaving}
        onClick={() => void run(() => store.getState().save(), m.rules_saved())}
      >
        {isSaving ? <Spinner /> : <Icon name="floppy-disk" />}
        <span>{isSaving ? m.rules_saving() : m.rules_save()}</span>
      </Button>
    </ButtonGroup>
  );
}

/** 已打开文档的编辑器主体。 */
function EditorSurface({ documentId }: { documentId: string }) {
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
            <RuleFlowCanvas />
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

export function RuleWorkspace({ documentId }: { documentId?: string }) {
  const m = useMessages();
  const navigate = useNavigate();
  const [documents, setDocuments] = useState<NativeRuleDocumentSummary[]>([]);
  const [listStatus, setListStatus] = useState<'loading' | 'ready' | 'error'>('loading');
  const [listMessage, setListMessage] = useState('');
  const [creating, setCreating] = useState(false);

  const refresh = useCallback(async (): Promise<NativeRuleDocumentSummary[]> => {
    const items = await listNativeRuleDocuments();
    setDocuments(items);
    return items;
  }, []);

  useEffect(() => {
    let cancelled = false;
    setListStatus('loading');
    refresh()
      .then((items) => {
        if (cancelled) return;
        setListStatus('ready');
        // 深链没指定文档时落到第一份，避免打开空工作区。
        if (!documentId && items[0]) {
          navigate(`/sources/rules/${encodeURIComponent(items[0].document_id)}`, {
            replace: true,
          });
        }
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        setListStatus('error');
        setListMessage(error instanceof Error ? error.message : String(error));
      });
    return () => {
      cancelled = true;
    };
  }, [documentId, navigate, refresh]);

  const selected = useMemo(
    () => documents.find((item) => item.document_id === documentId) ?? null,
    [documents, documentId],
  );

  // 每份文档一个 session；documentId 变化时换实例并重挂载子树。
  const sessionRef = useRef<{ id: string; session: RuleEditorSession } | null>(null);
  if (documentId && sessionRef.current?.id !== documentId) {
    sessionRef.current = { id: documentId, session: createRuleEditorSession() };
  }
  const session = sessionRef.current?.session;

  const createBlank = useCallback(async () => {
    setCreating(true);
    try {
      const summary = await createRuleEditorSession().getState().createBlank();
      await refresh();
      navigate(`/sources/rules/${encodeURIComponent(summary.document_id)}`);
    } catch (error) {
      toast.error(sessionErrorText(m, error));
    } finally {
      setCreating(false);
    }
  }, [m, navigate, refresh]);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <PageToolbar
        meta={
          documents.length > 0 ? m.rules_document_count({ count: documents.length }) : undefined
        }
        actions={
          <>
            {documentId && session ? (
              <RuleEditorSessionProvider session={session}>
                <EditorToolbar />
              </RuleEditorSessionProvider>
            ) : null}
            <Button
              size="sm"
              variant="outline"
              disabled={creating}
              onClick={() => void createBlank()}
            >
              {creating ? <Spinner /> : <Icon name="plus" />}
              <span>{m.rules_new_rule()}</span>
            </Button>
          </>
        }
      >
        <NativeSelect
          size="sm"
          className="ml-2 min-w-56"
          aria-label={m.rules_document_label()}
          value={documentId ?? ''}
          onChange={(event) => navigate(`/sources/rules/${encodeURIComponent(event.target.value)}`)}
          disabled={listStatus === 'loading' || documents.length === 0}
        >
          {documents.length === 0 ? (
            <NativeSelectOption value="">{m.rules_no_documents_option()}</NativeSelectOption>
          ) : null}
          {documents.map((document) => (
            <NativeSelectOption key={document.document_id} value={document.document_id}>
              {document.title}
            </NativeSelectOption>
          ))}
        </NativeSelect>
      </PageToolbar>

      {listStatus === 'error' ? (
        <p
          role="alert"
          className="border-b border-hairline px-(--page-gutter) py-1.5 text-ui-sm text-danger"
        >
          {listMessage}
        </p>
      ) : null}

      <div className="min-h-0 flex-1 bg-canvas">
        {documentId && selected && session ? (
          <RuleEditorSessionProvider key={documentId} session={session}>
            <ReactFlowProvider>
              <EditorSurface documentId={documentId} />
            </ReactFlowProvider>
          </RuleEditorSessionProvider>
        ) : (
          <Empty className="h-full">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <Icon name="tree-structure" />
              </EmptyMedia>
              <EmptyTitle>
                {listStatus === 'loading' ? m.rules_loading() : m.rules_canvas_empty()}
              </EmptyTitle>
              <EmptyDescription>{m.rules_canvas_empty_hint()}</EmptyDescription>
            </EmptyHeader>
          </Empty>
        )}
      </div>
    </div>
  );
}
