//! 定义预览：脱敏后的 Definition JSON 与后端溯源文本。
//!
//! 这里绝不显示凭证明文与执行 Plan。Definition 侧由本组件在序列化时把凭证键
//! 打码，溯源侧的 masked_text 由 Rust 负责脱敏。

import { useCallback, useEffect, useMemo, useState } from 'react';
import { Icon } from '@/components/Icon';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useMessages } from '@/shared/i18n/messages';
import { getNativeRuleProvenance, type NativeRuleProvenanceView } from '@/shared/tauri/rules';
import { useRuleEditorSession } from './use-session';

/** 凭证类键名一律打码；宁可多打码也不能漏。 */
const SECRET_KEY = /credential|secret|password|token/iu;
const MASK = '••••';

/** 复制成功提示的显示时长。 */
const COPIED_FEEDBACK_MS = 2000;

type PreviewTab = 'definition' | 'provenance';

export function DefinitionPreview() {
  const m = useMessages();
  const documentId = useRuleEditorSession((state) => state.summary?.document_id ?? null);
  const definition = useRuleEditorSession((state) => state.core.definition);

  const [tab, setTab] = useState<PreviewTab>('definition');
  const [provenance, setProvenance] = useState<NativeRuleProvenanceView | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!documentId) {
      setProvenance(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    getNativeRuleProvenance({ document_id: documentId })
      .then((view) => {
        if (cancelled) return;
        setProvenance(view);
        setLoading(false);
      })
      .catch((caught: unknown) => {
        if (cancelled) return;
        setError(caught instanceof Error ? caught.message : String(caught));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [documentId]);

  const definitionText = useMemo(
    () =>
      JSON.stringify(definition, (key, value: unknown) => (SECRET_KEY.test(key) ? MASK : value), 2),
    [definition],
  );

  const currentText = tab === 'definition' ? definitionText : (provenance?.masked_text ?? '');

  // 搜索是按行过滤，不是高亮；大定义里定位某个字段比翻页快。
  const filteredText = useMemo(() => {
    const term = search.trim().toLowerCase();
    if (!term) return currentText;
    return currentText
      .split('\n')
      .filter((line) => line.toLowerCase().includes(term))
      .join('\n');
  }, [currentText, search]);

  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), COPIED_FEEDBACK_MS);
    return () => clearTimeout(timer);
  }, [copied]);

  const copy = useCallback(() => {
    navigator.clipboard
      .writeText(filteredText)
      .then(() => setCopied(true))
      .catch(() => {
        // 受限 WebView 里没有 clipboard 权限；文本仍可手动选中复制。
        setCopied(false);
      });
  }, [filteredText]);

  return (
    <div className="flex min-h-0 flex-col gap-2">
      <Tabs value={tab} onValueChange={(value) => setTab(value as PreviewTab)}>
        <TabsList className="w-full">
          <TabsTrigger value="definition" className="flex-1">
            {m.rules_preview_definition_tab()}
          </TabsTrigger>
          <TabsTrigger value="provenance" className="flex-1" disabled={!provenance}>
            {m.rules_preview_provenance_tab()}
          </TabsTrigger>
        </TabsList>

        <div className="flex items-center gap-2 py-2">
          <Input
            type="search"
            value={search}
            placeholder={m.rules_preview_search_hint()}
            className="h-(--density-control-sm) min-w-0 flex-1 text-ui-sm"
            onChange={(event) => setSearch(event.target.value)}
          />
          <Button variant="ghost" size="xs" onClick={copy}>
            <Icon name={copied ? 'check' : 'copy'} />
            <span>{copied ? m.rules_preview_copied() : m.rules_preview_copy()}</span>
          </Button>
        </div>

        {(['definition', 'provenance'] as const).map((value) => (
          <TabsContent key={value} value={value} className="min-h-0">
            {loading ? (
              <p className="flex items-center gap-2 text-ui-sm text-ink-muted">
                <Icon name="arrow-clockwise" />
                {m.rules_loading()}
              </p>
            ) : error !== null && value === 'provenance' ? (
              <p role="alert" className="text-ui-sm text-danger">
                {error}
              </p>
            ) : filteredText.length === 0 ? (
              <p className="border border-dashed border-hairline px-3 py-4 text-center text-ui-sm text-ink-muted">
                {m.rules_preview_search_no_results()}
              </p>
            ) : (
              <ScrollArea className="max-h-80">
                <pre className="border border-hairline bg-surface-1 px-2.5 py-2 font-mono text-code break-all whitespace-pre-wrap text-ink">
                  {filteredText}
                </pre>
              </ScrollArea>
            )}
          </TabsContent>
        ))}
      </Tabs>

      <div className="flex flex-col gap-1 text-ui-sm text-ink-muted">
        <p className="flex items-center gap-1">
          <Icon name="lock" />
          {m.rules_preview_no_secret()}
        </p>
        <p className="flex items-center gap-1">
          <Icon name="eye" />
          {m.rules_preview_no_plan()}
        </p>
      </div>
    </div>
  );
}
