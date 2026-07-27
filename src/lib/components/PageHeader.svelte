<script lang="ts" module>
  type HeaderActionBase = {
    label: string;
    icon?: 'arrow-right' | 'plus' | 'x';
  };

  export type HeaderAction =
    | (HeaderActionBase & {
        href: string;
        onclick?: never;
        pressed?: never;
      })
    | (HeaderActionBase & {
        href?: never;
        onclick: () => void;
        pressed?: boolean;
      });
</script>

<script lang="ts">
  import { resolve } from '$app/paths';
  import Icon from '$lib/components/Icon.svelte';
  import { Button } from '$lib/components/ui/button/index.js';

  type Props = {
    title: string;
    description?: string;
    action?: HeaderAction;
  };

  let { title, description, action }: Props = $props();
</script>

<header
  data-slot="page-header"
  class="flex w-full flex-col gap-3 border-b border-hairline pb-3 in-data-[chrome-family=bottom]:gap-2 in-data-[chrome-family=bottom]:border-b-0 in-data-[chrome-family=bottom]:pb-0 sm:flex-row sm:items-end sm:justify-between"
>
  <div class="min-w-0">
    <h1
      class="text-xl font-semibold tracking-tight text-ink in-data-[chrome-family=bottom]:sr-only sm:text-2xl"
    >
      {title}
    </h1>
    {#if description}
      <p
        class="mt-1 max-w-[65ch] text-sm leading-5 text-ink-muted in-data-[chrome-family=bottom]:mt-0"
      >
        {description}
      </p>
    {/if}
  </div>

  {#if action}
    {#if 'href' in action}
      <Button href={resolve(action.href as '/')} variant="outline" class="self-start sm:self-auto">
        <span>{action.label}</span>
        <Icon name={action.icon ?? 'arrow-right'} class="size-4" />
      </Button>
    {:else}
      <Button
        type="button"
        variant="outline"
        class="self-start sm:self-auto"
        aria-pressed={action.pressed}
        onclick={() => action.onclick()}
      >
        <Icon name={action.icon ?? 'plus'} class="size-4" />
        <span>{action.label}</span>
      </Button>
    {/if}
  {/if}
</header>
