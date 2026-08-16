//! 新建规则对话框：模板 / 空白图 / 导入三条路径。
//!
//! legacy 把它们摊在三个位置（向导面板、空态按钮、工具栏导入按钮），这里收成
//! 一个入口。三条路径都用一个临时 session 建文档，建完由调用方跳转过去。

import { useRef, useState } from 'react';
import { Icon } from '@/components/Icon';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select';
import { Spinner } from '@/components/ui/spinner';
import { useMessages } from '@/shared/i18n/messages';
import type {
  ExpectedDataType,
  NativeRuleDocumentSummary,
  StandardIntent,
} from '@/shared/tauri/rules';
import {
  MAX_NATIVE_RULE_IMPORT_BYTES,
  parseNativeRuleDefinition,
  type NativeRuleImportIssue,
} from './model/native-import';
import { createRuleEditorSession } from './model/session';

type Messages = ReturnType<typeof useMessages>;

const INTENT_OPTIONS: readonly { value: StandardIntent; label: (m: Messages) => string }[] = [
  { value: 'Search', label: (m) => m.rules_template_wizard_intent_search() },
  { value: 'Discover', label: (m) => m.rules_template_wizard_intent_discover() },
  { value: 'ResolveItem', label: (m) => m.rules_template_wizard_intent_resolve_item() },
  { value: 'ListUnits', label: (m) => m.rules_template_wizard_intent_list_units() },
  { value: 'ResolveAsset', label: (m) => m.rules_template_wizard_intent_resolve_asset() },
  { value: 'ContinueAction', label: (m) => m.rules_template_wizard_intent_continue_action() },
];

const DATA_TYPE_OPTIONS: readonly { value: ExpectedDataType; label: (m: Messages) => string }[] = [
  { value: 'html', label: (m) => m.rules_template_wizard_data_type_html() },
  { value: 'xml', label: (m) => m.rules_template_wizard_data_type_xml() },
  { value: 'json', label: (m) => m.rules_template_wizard_data_type_json() },
];

/** 导入失败原因的稳定 code → 本地化文案。 */
function importIssueText(m: Messages, issue: NativeRuleImportIssue): string {
  switch (issue) {
    case 'invalid_json': {
      return m.rules_import_invalid_json();
    }
    case 'root_not_object': {
      return m.rules_import_root_not_object();
    }
    case 'contract_mismatch': {
      return m.rules_import_contract_mismatch();
    }
    case 'schema_unsupported': {
      return m.rules_import_schema_unsupported();
    }
    case 'unknown_root_field': {
      return m.rules_import_unknown_field();
    }
    case 'invalid_source_identity': {
      return m.rules_import_invalid_source_identity();
    }
    case 'invalid_intent_exports': {
      return m.rules_import_invalid_intent_exports();
    }
    case 'invalid_flow': {
      return m.rules_import_invalid_flow();
    }
    case 'invalid_capability_manifest': {
      return m.rules_import_invalid_capability_manifest();
    }
    case 'invalid_source_id_rules': {
      return m.rules_import_invalid_source_id_rules();
    }
    case 'file_too_large': {
      return m.rules_import_file_too_large();
    }
  }
}

function errorText(m: Messages, caught: unknown): string {
  if (caught instanceof Error && caught.message.length > 0) return caught.message;
  return m.rules_import_create_failed();
}

export function NewRuleDialog({
  open,
  onOpenChange,
  onCreated,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreated: (summary: NativeRuleDocumentSummary) => void;
}) {
  const m = useMessages();
  const fileRef = useRef<HTMLInputElement>(null);

  const [title, setTitle] = useState('');
  const [intent, setIntent] = useState<StandardIntent>('Search');
  const [dataType, setDataType] = useState<ExpectedDataType>('html');
  const [baseUrl, setBaseUrl] = useState('');
  const [titleTouched, setTitleTouched] = useState(false);
  const [baseUrlTouched, setBaseUrlTouched] = useState(false);
  const [pending, setPending] = useState<'template' | 'blank' | 'import' | null>(null);
  const [error, setError] = useState<string | null>(null);

  const titleValid = title.trim().length > 0;
  const baseUrlValid = baseUrl.trim().length > 0;

  /** 三条创建路径共用：跑一次一次性 session，成功就把摘要交回去。 */
  const create = async (
    kind: 'template' | 'blank' | 'import',
    run: (
      session: ReturnType<typeof createRuleEditorSession>,
    ) => Promise<NativeRuleDocumentSummary>,
  ) => {
    setPending(kind);
    setError(null);
    try {
      onCreated(await run(createRuleEditorSession()));
    } catch (caught) {
      setError(errorText(m, caught));
    } finally {
      setPending(null);
    }
  };

  const handleFile = async (file: File) => {
    if (file.size > MAX_NATIVE_RULE_IMPORT_BYTES) {
      setError(importIssueText(m, 'file_too_large'));
      return;
    }
    const parsed = parseNativeRuleDefinition(await file.text());
    if (!parsed.ok) {
      setError(importIssueText(m, parsed.issue));
      return;
    }
    // 文件名去掉扩展名当标题；空名回退到默认标题。
    const fileTitle = file.name.replace(/\.json$/iu, '').trim() || m.rules_import_default_title();
    await create('import', (session) =>
      session.getState().createImported({ title: fileTitle, definition: parsed.definition }),
    );
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{m.rules_new_rule()}</DialogTitle>
          <DialogDescription>{m.rules_template_wizard_intro()}</DialogDescription>
        </DialogHeader>

        {error === null ? null : (
          <p
            role="alert"
            className="border border-danger/30 bg-danger/10 px-3 py-2 text-ui-sm text-danger"
          >
            {error}
          </p>
        )}

        <div className="flex flex-col gap-3">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="new-rule-title">{m.rules_template_wizard_title_label()}</Label>
            <Input
              id="new-rule-title"
              value={title}
              placeholder={m.rules_template_wizard_title_placeholder()}
              aria-invalid={titleTouched && !titleValid}
              onBlur={() => setTitleTouched(true)}
              onChange={(event) => setTitle(event.target.value)}
            />
            {titleTouched && !titleValid ? (
              <p className="text-ui-sm text-danger">{m.rules_template_wizard_required()}</p>
            ) : null}
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="new-rule-intent">{m.rules_template_wizard_intent_label()}</Label>
              <NativeSelect
                id="new-rule-intent"
                size="sm"
                value={intent}
                onChange={(event) => setIntent(event.target.value as StandardIntent)}
              >
                {INTENT_OPTIONS.map((option) => (
                  <NativeSelectOption key={option.value} value={option.value}>
                    {option.label(m)}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
            </div>

            <div className="flex flex-col gap-1.5">
              <Label htmlFor="new-rule-data-type">
                {m.rules_template_wizard_data_type_label()}
              </Label>
              <NativeSelect
                id="new-rule-data-type"
                size="sm"
                value={dataType}
                onChange={(event) => setDataType(event.target.value as ExpectedDataType)}
              >
                {DATA_TYPE_OPTIONS.map((option) => (
                  <NativeSelectOption key={option.value} value={option.value}>
                    {option.label(m)}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
            </div>
          </div>

          <div className="flex flex-col gap-1.5">
            <Label htmlFor="new-rule-base-url">{m.rules_template_wizard_base_url_label()}</Label>
            <Input
              id="new-rule-base-url"
              type="url"
              value={baseUrl}
              placeholder={m.rules_template_wizard_base_url_placeholder()}
              aria-invalid={baseUrlTouched && !baseUrlValid}
              onBlur={() => setBaseUrlTouched(true)}
              onChange={(event) => setBaseUrl(event.target.value)}
            />
            {baseUrlTouched && !baseUrlValid ? (
              <p className="text-ui-sm text-danger">{m.rules_template_wizard_required()}</p>
            ) : null}
          </div>
        </div>

        <DialogFooter className="sm:justify-between">
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              disabled={pending !== null}
              onClick={() => void create('blank', (session) => session.getState().createBlank())}
            >
              {pending === 'blank' ? <Spinner /> : <Icon name="file-plus" />}
              <span>{m.rules_blank_graph()}</span>
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={pending !== null}
              onClick={() => fileRef.current?.click()}
            >
              {pending === 'import' ? <Spinner /> : <Icon name="upload-simple" />}
              <span>{pending === 'import' ? m.rules_importing() : m.rules_import_native()}</span>
            </Button>
            <input
              ref={fileRef}
              type="file"
              accept="application/json,.json"
              className="sr-only"
              aria-label={m.rules_import_native()}
              onChange={(event) => {
                const file = event.target.files?.[0];
                // 先清空再处理：同一个文件连选两次也要能触发 change。
                event.target.value = '';
                if (file) void handleFile(file);
              }}
            />
          </div>

          <div className="flex gap-2">
            <DialogClose
              render={
                <Button variant="ghost" size="sm">
                  {m.rules_template_wizard_cancel()}
                </Button>
              }
            />
            <Button
              size="sm"
              disabled={!titleValid || !baseUrlValid || pending !== null}
              onClick={() =>
                void create('template', (session) =>
                  session.getState().createTemplate({
                    title: title.trim(),
                    intent,
                    dataType,
                    baseUrl: baseUrl.trim(),
                  }),
                )
              }
            >
              {pending === 'template' ? <Spinner /> : null}
              <span>
                {pending === 'template'
                  ? m.rules_template_wizard_creating()
                  : m.rules_template_wizard_create()}
              </span>
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
