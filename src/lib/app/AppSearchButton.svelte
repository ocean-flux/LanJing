<script lang="ts">
  import Search from '@lucide/svelte/icons/search';
  import { Button } from '$lib/components/ui/button';
  import { m } from '$lib/i18n';

  type Props = {
    label?: string;
    compact?: boolean;
    onclick?: () => void;
  };

  let { label = m.search(), compact = false, onclick }: Props = $props();
</script>

<Button
  type="button"
  variant="outline"
  size="sm"
  class={`${compact ? 'w-8 px-0' : 'px-3'} rounded-full bg-background/60 text-muted-foreground`}
  aria-label={m.search_open()}
  {onclick}
>
  <Search size={14} aria-hidden="true" />
  {#if !compact}
    <span class="hidden sm:inline">{label}</span>
  {/if}
</Button>
