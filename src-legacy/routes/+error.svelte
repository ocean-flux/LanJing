<script lang="ts">
  import { page } from '$app/state';
  import Icon from '$lib/components/Icon.svelte';
  import PageFrame from '$lib/components/PageFrame.svelte';
  import { Button } from '$lib/components/ui/button';
  import { m } from '$lib/i18n';
</script>

<svelte:head>
  <title>{m.shell_error_title()} · {m.app_name()}</title>
</svelte:head>

<PageFrame width="narrow">
  <section
    class="glass-panel flex flex-col items-start gap-3 rounded-lg border border-hairline p-4 sm:p-5"
    aria-labelledby="root-error-heading"
    aria-describedby="root-error-description"
    data-root-error
  >
    <p class="font-mono text-xs font-medium text-ink-muted">
      {m.shell_error_status({ status: page.status })}
    </p>
    <div class="space-y-1.5">
      <h1 id="root-error-heading" class="text-xl font-semibold tracking-tight text-ink">
        {m.shell_error_heading()}
      </h1>
      <p id="root-error-description" class="max-w-[65ch] text-sm leading-6 text-ink-muted">
        {m.shell_error_description()}
      </p>
    </div>
    <Button href="/" size="sm" class="mt-1">
      <Icon name="arrow-left" class="size-4" />
      <span>{m.shell_error_return()}</span>
    </Button>
  </section>
</PageFrame>
