<script lang="ts">
  import { importRule, confirmImport, type ImportPreview } from '$lib/stores/rules.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Textarea } from '$lib/components/ui/textarea';
  import { m } from '$lib/i18n';

  let jsonText = $state('');
  let preview = $state<ImportPreview | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let success = $state<string | null>(null);

  /** 检测 host 是否为私有/内网地址（RFC 1918）。 */
  function isPrivateHost(host: string): boolean {
    return /^(localhost|127\.0\.0\.1|::1|192\.168\.|172\.(1[6-9]|2\d|3[01])\.|169\.254\.|0\.0\.0\.0)$/.test(
      host,
    );
  }

  /** 检测 URL 是否指向内网地址。 */
  function isInternalUrl(url: string): boolean {
    const lower = url.toLowerCase();
    try {
      const u = new URL(lower);
      return isPrivateHost(u.hostname);
    } catch {
      return isPrivateHost(lower);
    }
  }

  /** 检测 JS 源码中是否含危险调用。 */
  function hasDangerousJs(code: string): boolean {
    return /\bfetch\s*\(|\beval\s*\(/.test(code);
  }

  async function handlePreview() {
    loading = true;
    error = null;
    success = null;
    preview = null;
    try {
      preview = await importRule(jsonText);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function handleConfirm() {
    if (!preview) return;
    loading = true;
    error = null;
    success = null;
    try {
      const id = await confirmImport(preview.graph_json);
      success = m.debug_import_success({ id });
      preview = null;
      jsonText = '';
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
</script>

<div class="flex flex-col gap-4 h-full overflow-auto">
  <h2 class="text-lg font-semibold">{m.debug_import_title()}</h2>

  <!-- JSON 输入 -->
  <div class="flex flex-col gap-2">
    <label for="rule-json" class="text-sm font-medium">{m.debug_rule_json_label()}</label>
    <Textarea
      id="rule-json"
      bind:value={jsonText}
      placeholder={m.debug_rule_json_placeholder()}
      rows={8}
      disabled={loading}
    />
  </div>

  <Button onclick={handlePreview} disabled={loading || !jsonText.trim()}>
    {loading ? m.debug_parse_loading() : m.debug_parse_preview()}
  </Button>

  <!-- 错误提示 -->
  {#if error}
    <div
      class="rounded-md border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
    >
      {error}
    </div>
  {/if}

  <!-- 成功提示 -->
  {#if success}
    <div
      class="rounded-md border border-emerald-500/30 bg-emerald-50 dark:bg-emerald-950/20 p-3 text-sm text-emerald-700 dark:text-emerald-400"
    >
      {success}
    </div>
  {/if}

  <!-- 预览面板 -->
  {#if preview}
    <div class="rounded-md border bg-card p-4 space-y-3">
      <h3 class="font-semibold text-base">{m.debug_preview_title()}</h3>

      <div class="grid grid-cols-2 gap-2 text-sm">
        <span class="text-muted-foreground">{m.debug_source_url()}</span>
        <span class="font-mono text-xs break-all"
          >{preview.source_url || m.debug_no_source_url()}</span
        >

        <span class="text-muted-foreground">{m.debug_node_count()}</span>
        <span>{preview.node_count}</span>

        <span class="text-muted-foreground">{m.debug_edge_count()}</span>
        <span>{preview.edge_count}</span>

        <span class="text-muted-foreground">{m.debug_js_block_count()}</span>
        <span>{preview.js_block_count}</span>

        <span class="text-muted-foreground">{m.debug_sandbox_network()}</span>
        <span>{preview.sandbox.network ? m.debug_allowed() : m.debug_denied()}</span>

        <span class="text-muted-foreground">{m.debug_sandbox_fs()}</span>
        <span>{preview.sandbox.system.fs ? m.debug_allowed() : m.debug_denied()}</span>

        <span class="text-muted-foreground">{m.debug_sandbox_env()}</span>
        <span>{preview.sandbox.system.env ? m.debug_allowed() : m.debug_denied()}</span>

        <span class="text-muted-foreground">{m.debug_sandbox_process()}</span>
        <span>{preview.sandbox.system.process ? m.debug_allowed() : m.debug_denied()}</span>
      </div>

      <!-- HTTP 目标 URL 列表 -->
      {#if preview.http_target_urls.length > 0}
        <div class="space-y-1">
          <span class="text-sm font-medium">{m.debug_http_targets()}</span>
          <ul class="space-y-0.5">
            {#each preview.http_target_urls as url (url)}
              <li class="text-xs font-mono flex items-start gap-1">
                {#if isInternalUrl(url)}
                  <span class="text-destructive font-bold shrink-0">⚠</span>
                  <span class="text-destructive break-all">{url}</span>
                {:else}
                  <span class="text-muted-foreground break-all">{url}</span>
                {/if}
              </li>
            {/each}
          </ul>
        </div>
      {/if}

      <!-- JS 源码预览 -->
      {#if preview.js_sources.length > 0}
        <div class="space-y-2">
          <span class="text-sm font-medium"
            >{m.debug_js_sources({ count: preview.js_sources.length })}</span
          >
          {#each preview.js_sources as src, i (i)}
            <div class="rounded border bg-muted/30 p-2">
              <div class="flex items-center gap-2 mb-1">
                <span class="text-xs font-mono text-muted-foreground"
                  >{m.debug_block_index({ index: i + 1 })}</span
                >
                {#if hasDangerousJs(src)}
                  <span class="text-xs text-destructive font-semibold"
                    >{m.debug_dangerous_js()}</span
                  >
                {/if}
              </div>
              <pre
                class="text-xs font-mono whitespace-pre-wrap max-h-32 overflow-y-auto">{src}</pre>
            </div>
          {/each}
        </div>
      {/if}

      <!-- 确认导入 -->
      <Button onclick={handleConfirm} disabled={loading} variant="default" class="w-full">
        {loading ? m.debug_importing() : m.debug_confirm_import()}
      </Button>
    </div>
  {/if}
</div>
