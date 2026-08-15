<script lang="ts">
  import { m } from '$lib/i18n';
  import { onMount } from 'svelte';
  import Icon from '$lib/components/Icon.svelte';
  import { Input } from '$lib/components/ui/input/index.js';
  import { Tabs, TabsList, TabsTrigger, TabsContent } from '$lib/components/ui/tabs/index.js';
  import { ScrollArea } from '$lib/components/ui/scroll-area/index.js';
  import { Button } from '$lib/components/ui/button/index.js';
  import { getNativeRuleProvenance } from '$lib/rules/native-authoring/wire';
  import type { NativeRuleProvenanceView } from '$lib/rules/native-authoring/wire';
  import type { NativeRuleEditorSession } from '$lib/rules/native-authoring/session.svelte';

  type Props = {
    session: NativeRuleEditorSession;
  };

  let { session }: Props = $props();

  let provenance = $state<NativeRuleProvenanceView | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let searchTerm = $state('');
  let copied = $state(false);
  let activeTab = $state<'definition' | 'provenance'>('definition');

  // 文档 id 变化时重载溯源
  const documentId = $derived(session.documentId);

  onMount(() => {
    void loadProvenance();
  });

  async function loadProvenance() {
    if (!documentId) {
      provenance = null;
      return;
    }
    loading = true;
    error = null;
    try {
      provenance = await getNativeRuleProvenance({ document_id: documentId });
    } catch (caught) {
      error = String(caught);
    } finally {
      loading = false;
    }
  }

  /**
   * masked core state：定义 JSON 中凭证字段脱敏，不含 Plan/Graph secret。
   * 只读展示；复制/搜索均针对此文本。
   */
  function definitionText(): string {
    const definition = session.definition;
    if (!definition) return '';
    return JSON.stringify(
      definition,
      (key, value) => {
        // 凭证字段（json_pointer 含 credential/secret 路径）脱敏
        if (/credential|secret|password|token/i.test(key)) return '••••';
        return value;
      },
      2,
    );
  }

  function provenanceText(): string {
    if (!provenance) return '';
    // masked_text 已由后端脱敏，不含 secret/Plan
    return provenance.masked_text;
  }

  const currentText = $derived.by(() => {
    return activeTab === 'definition' ? definitionText() : provenanceText();
  });

  let filteredText = $derived.by(() => {
    const text = currentText;
    if (!searchTerm) return text;
    return text
      .split('\n')
      .filter((line) => line.toLowerCase().includes(searchTerm.toLowerCase()))
      .join('\n');
  });

  const hasSearchResults = $derived.by(() => {
    if (!searchTerm) return true;
    return currentText.toLowerCase().includes(searchTerm.toLowerCase());
  });

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(filteredText);
      copied = true;
      setTimeout(() => {
        copied = false;
      }, 2000);
    } catch {
      /* clipboard 不可用时静默 */
    }
  }
</script>

<div class="flex flex-col gap-2">
  <h3 class="flex items-center gap-2 text-sm font-medium">
    <Icon name="eye" class="size-4 text-lantern-strong" />
    <span>{m.rules_preview_title()}</span>
  </h3>

  {#if !documentId}
    <p class="text-xs text-ink-muted">{m.rules_preview_title()}</p>
  {:else}
    <Tabs bind:value={activeTab}>
      <TabsList class="w-full">
        <TabsTrigger value="definition" class="flex-1"
          >{m.rules_preview_definition_tab()}</TabsTrigger
        >
        <TabsTrigger value="provenance" class="flex-1" disabled={!provenance}>
          {m.rules_preview_provenance_tab()}
        </TabsTrigger>
      </TabsList>

      <div class="flex items-center gap-2">
        <div class="relative min-w-0 flex-1">
          <Input
            type="search"
            bind:value={searchTerm}
            placeholder={m.rules_preview_search_hint()}
            class="h-(--density-control-sm) text-xs"
          />
        </div>
        <Button type="button" variant="ghost" size="xs" onclick={handleCopy}>
          <Icon name={copied ? 'check' : 'copy'} class="size-3.5" />
          <span class="text-xs">{copied ? m.rules_preview_copied() : m.rules_preview_copy()}</span>
        </Button>
      </div>

      {#if loading}
        <div class="flex items-center gap-2 text-xs text-ink-muted">
          <Icon name="arrow-clockwise" class="size-3" />
          {m.rules_loading()}
        </div>
      {:else if error}
        <p class="text-xs text-destructive">{error}</p>
      {:else if !hasSearchResults}
        <div
          class="rounded-md border border-dashed border-hairline px-3 py-4 text-center text-xs text-ink-muted"
        >
          {m.rules_preview_search_no_results()}
        </div>
      {:else}
        <TabsContent value="definition">
          <ScrollArea class="max-h-80">
            <pre
              class="rounded-md border border-hairline bg-surface-1 px-2.5 py-2 font-mono text-xs break-all whitespace-pre-wrap text-ink">{filteredText}</pre>
          </ScrollArea>
        </TabsContent>
        <TabsContent value="provenance">
          <ScrollArea class="max-h-80">
            <pre
              class="rounded-md border border-hairline bg-surface-1 px-2.5 py-2 font-mono text-xs break-all whitespace-pre-wrap text-ink">{filteredText}</pre>
          </ScrollArea>
        </TabsContent>
      {/if}
    </Tabs>

    <div class="flex flex-col gap-1 text-xs text-ink-muted">
      <p class="flex items-center gap-1">
        <Icon name="lock" class="size-3" />
        {m.rules_preview_no_secret()}
      </p>
      <p class="flex items-center gap-1">
        <Icon name="eye" class="size-3" />
        {m.rules_preview_no_plan()}
      </p>
    </div>
  {/if}
</div>
