//! 规则工作区：文档切换 + 编辑器 session 的宿主。
//!
//! 每份文档一个 session 实例（用 documentId 作 key 重挂载），切换文档不会
//! 继承上一份的撤销历史。工具栏与编辑面只调 session 的 action，不直接接触 core。

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Icon } from '@/components/Icon';
import { PageToolbar } from '@/components/PageToolbar';
import { Button } from '@/components/ui/button';
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select';
import { useMessages } from '@/shared/i18n/messages';
import { listNativeRuleDocuments, type NativeRuleDocumentSummary } from '@/shared/tauri/rules';
import { EditorSurface } from './EditorSurface';
import { EditorToolbar } from './EditorToolbar';
import { createRuleEditorSession, type RuleEditorSession } from './model/session';
import { NewRuleDialog } from './NewRuleDialog';
import { RuleEditorSessionProvider } from './use-session';

export function RuleWorkspace({ documentId }: { documentId?: string }) {
  const m = useMessages();
  const navigate = useNavigate();
  const [documents, setDocuments] = useState<NativeRuleDocumentSummary[]>([]);
  const [listStatus, setListStatus] = useState<'loading' | 'ready' | 'error'>('loading');
  const [listMessage, setListMessage] = useState('');
  const [createOpen, setCreateOpen] = useState(false);

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

  /** 三条创建路径都汇到这里：刷新列表并跳到新文档。 */
  const handleCreated = useCallback(
    async (summary: NativeRuleDocumentSummary) => {
      setCreateOpen(false);
      await refresh();
      navigate(`/sources/rules/${encodeURIComponent(summary.document_id)}`);
    },
    [navigate, refresh],
  );

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
            <Button size="sm" variant="outline" onClick={() => setCreateOpen(true)}>
              <Icon name="plus" />
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
            <EditorSurface documentId={documentId} />
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

      <NewRuleDialog
        open={createOpen}
        onOpenChange={setCreateOpen}
        onCreated={(summary) => void handleCreated(summary)}
      />
    </div>
  );
}
