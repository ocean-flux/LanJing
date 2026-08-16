//! 编辑器工具栏：撤销 / 重做 / 预览 / 校验 / 保存。
//!
//! 挂在页面 PageToolbar 的 actions 槽里，位于画布之外，所以只依赖 session
//! store，不碰 ReactFlow 上下文。

import { useCallback, useState } from 'react';
import { toast } from 'sonner';
import { Icon } from '@/components/Icon';
import { Button } from '@/components/ui/button';
import { ButtonGroup } from '@/components/ui/button-group';
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetTrigger } from '@/components/ui/sheet';
import { Spinner } from '@/components/ui/spinner';
import { useMessages } from '@/shared/i18n/messages';
import { DefinitionPreview } from './DefinitionPreview';
import {
  selectCanRedo,
  selectCanUndo,
  selectHasUnsavedChanges,
  selectIsSaving,
} from './model/session';
import { sessionErrorText } from './session-error';
import { useRuleEditorSession, useRuleEditorSessionStore } from './use-session';

export function EditorToolbar() {
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
      <Sheet>
        <SheetTrigger
          render={
            <Button
              variant="outline"
              size="icon-sm"
              aria-label={m.rules_preview_title()}
              title={m.rules_preview_title()}
            >
              <Icon name="eye" />
            </Button>
          }
        />
        <SheetContent side="right" className="w-[32rem] max-w-full">
          <SheetHeader>
            <SheetTitle>{m.rules_preview_title()}</SheetTitle>
          </SheetHeader>
          <div className="min-h-0 flex-1 overflow-y-auto px-4 pb-4">
            <DefinitionPreview />
          </div>
        </SheetContent>
      </Sheet>
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
