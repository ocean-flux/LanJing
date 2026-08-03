<script lang="ts">
  import { m } from '$lib/i18n';
  import Icon from '$lib/components/Icon.svelte';
  import { Badge } from '$lib/components/ui/badge/index.js';
  import { ScrollArea } from '$lib/components/ui/scroll-area/index.js';
  import type { InstallDiagnostic } from '$lib/rules/native-authoring/wire';
  import type { NativeRuleEditorSession } from '$lib/rules/native-authoring/session.svelte';

  type Props = {
    session: NativeRuleEditorSession;
  };

  let { session }: Props = $props();

  const diagnostics = $derived(session.diagnostics);

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
</script>

<div class="flex flex-col gap-2">
  <h3 class="flex items-center gap-2 text-sm font-medium">
    <Icon name="warning-circle" class="size-4 text-lantern-strong" />
    <span>{m.rules_diagnostics()}</span>
    {#if diagnostics.length > 0}
      <Badge variant="outline" class="ml-auto">{diagnostics.length}</Badge>
    {/if}
  </h3>

  {#if diagnostics.length === 0}
    <p class="text-xs text-ink-muted">{m.rules_diagnostics_empty()}</p>
  {:else}
    <ScrollArea class="max-h-80">
      <div class="flex flex-col gap-1">
        {#each [...diagnostics].sort((a, b) => severityOrder(a.severity) - severityOrder(b.severity)) as diag (diag.code + diag.severity + (diag.span?.start ?? 0))}
          <div
            class="flex items-start gap-2 rounded-md px-2 py-1.5 text-xs transition-colors hover:bg-surface-2"
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
          </div>
        {/each}
      </div>
    </ScrollArea>
  {/if}
</div>
