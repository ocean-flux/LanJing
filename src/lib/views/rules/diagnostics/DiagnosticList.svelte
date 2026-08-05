<script lang="ts">
  import { getLocale, m } from '$lib/i18n';
  import Icon from '$lib/components/Icon.svelte';
  import { Badge } from '$lib/components/ui/badge/index.js';
  import { ScrollArea } from '$lib/components/ui/scroll-area/index.js';
  import type { ValidationState } from '$lib/rules/native-authoring/core';
  import type { InstallDiagnostic } from '$lib/rules/native-authoring/wire';
  import type { NativeRuleEditorSession } from '$lib/rules/native-authoring/session.svelte';

  type Props = {
    session: NativeRuleEditorSession;
  };

  let { session }: Props = $props();

  function localizedMessage(key: string, fallback: { en: string; 'zh-CN': string }): string {
    const candidate = (m as unknown as Record<string, () => string>)[key];
    if (typeof candidate === 'function') return candidate();
    if (getLocale() === 'en') return fallback.en;
    return fallback['zh-CN'];
  }

  const diagnostics = $derived(session.diagnostics);
  const validation = $derived<ValidationState>(
    session.validation ?? {
      status: 'unknown',
      revision: null,
      definitionHash: null,
      planHash: null,
      diagnostics: [],
    },
  );

  /** severity 排序权重 */
  function severityOrder(sev: InstallDiagnostic['severity']): number {
    return sev === 'error' ? 0 : sev === 'warning' ? 1 : 2;
  }

  function severityLabel(sev: InstallDiagnostic['severity']): string {
    return sev === 'error'
      ? m.rules_diagnostics_error({ count: 0 })
      : sev === 'warning'
        ? m.rules_diagnostics_warning({ count: 0 })
        : m.rules_diagnostics_info({ count: 0 });
  }

  function severityIcon(sev: InstallDiagnostic['severity']): 'warning-circle' | 'file-text' {
    return sev === 'error' || sev === 'warning' ? 'warning-circle' : 'file-text';
  }

  function severityBadgeVariant(
    sev: InstallDiagnostic['severity'],
  ): 'destructive' | 'default' | 'outline' {
    return sev === 'error' ? 'destructive' : sev === 'warning' ? 'default' : 'outline';
  }

  /** severity 图标颜色（Icon 组件不接受 class: 指令，用计算字符串）。 */
  function severityIconClass(sev: InstallDiagnostic['severity']): string {
    return sev === 'error'
      ? 'text-destructive'
      : sev === 'warning'
        ? 'text-warning'
        : 'text-ink-muted';
  }

  function validationLabel(status: typeof validation.status): string {
    const labels = {
      unknown: ['rules_validation_unknown', 'Not validated', '未校验'],
      pending: ['rules_validation_pending', 'Validating', '校验中'],
      valid: ['rules_validation_valid', 'Validated', '已通过'],
      invalid: ['rules_validation_invalid', 'Validation failed', '未通过'],
      stale: ['rules_validation_stale', 'Validation is stale', '结果已过期'],
      error: ['rules_validation_error', 'Validation error', '校验失败'],
    } as const;
    const [key, en, zhCN] = labels[status];
    return localizedMessage(key, { en, 'zh-CN': zhCN });
  }

  function focusDiagnostic(diagnostic: InstallDiagnostic): void {
    const path = diagnostic.span?.path;
    const nodeId = path?.match(/^\/flow\/nodes\/([^/]+)/)?.[1];
    if (nodeId && session.definition.flow.nodes.some((node) => node.id === nodeId)) {
      session.selectNode(nodeId);
      return;
    }
    const edge = session.flowProjection.edges.find((candidate) => {
      const semantic = candidate.data.edge;
      return Boolean(path?.includes(semantic.from.node_id) && path?.includes(semantic.to.node_id));
    });
    if (edge) session.selectEdge(edge.id);
  }
</script>

<div class="flex flex-col gap-2">
  <div class="flex items-center gap-2">
    <h3 class="flex items-center gap-2 text-sm font-medium">
      <Icon name="warning-circle" class="size-4 text-lantern-strong" />
      <span>{m.rules_diagnostics()}</span>
      {#if diagnostics.length > 0}
        <Badge variant="outline">{diagnostics.length}</Badge>
      {/if}
    </h3>
    <Badge variant={validation.status === 'valid' ? 'default' : 'outline'} class="ml-auto">
      {validationLabel(validation.status)}
    </Badge>
  </div>

  {#if diagnostics.length === 0}
    <p class="text-xs text-ink-muted">{m.rules_diagnostics_empty()}</p>
  {:else}
    <ScrollArea class="max-h-80">
      <div class="flex flex-col gap-1">
        {#each [...diagnostics].sort((a, b) => severityOrder(a.severity) - severityOrder(b.severity)) as diag (diag.code + diag.severity + (diag.span?.start ?? 0))}
          <button
            type="button"
            class="flex w-full items-start gap-2 rounded-md px-2 py-1.5 text-left text-xs transition-colors hover:bg-surface-2"
            aria-label={diag.code}
            onclick={() => focusDiagnostic(diag)}
          >
            <Icon
              name={severityIcon(diag.severity)}
              class="mt-0.5 size-3 shrink-0 {severityIconClass(diag.severity)}"
            />
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-1">
                <Badge variant={severityBadgeVariant(diag.severity)} class="text-[10px]">
                  {diag.code}
                </Badge>
                <span class="text-ink-muted">{severityLabel(diag.severity)}</span>
              </div>
              <p class="mt-0.5 leading-snug text-ink">{diag.message}</p>
              {#if diag.span}
                <p class="text-ink-muted">
                  {diag.span.path ?? ''}:{diag.span.start}–{diag.span.end}
                </p>
              {/if}
            </div>
          </button>
        {/each}
      </div>
    </ScrollArea>
  {/if}
</div>
