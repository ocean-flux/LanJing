<script lang="ts">
  import { resolve } from '$app/paths';
  import Icon from '$lib/components/Icon.svelte';

  type HeaderAction = {
    label: string;
    href?: string;
    onclick?: () => void;
    icon?: 'arrow-right' | 'plus' | 'x';
    pressed?: boolean;
  };

  type Props = {
    title: string;
    description?: string;
    action?: HeaderAction;
  };

  let { title, description, action }: Props = $props();
</script>

<header
  class="flex w-full flex-col gap-4 border-b border-hairline pb-5 sm:flex-row sm:items-end sm:justify-between"
>
  <div class="min-w-0">
    <h1 class="text-xl font-semibold tracking-tight text-ink sm:text-2xl">{title}</h1>
    {#if description}
      <p class="mt-1 max-w-2xl text-sm leading-6 text-ink-muted">{description}</p>
    {/if}
  </div>

  {#if action}
    {#if action.href}
      <a
        href={resolve(action.href as '/')}
        class="inline-flex min-h-11 shrink-0 items-center justify-center gap-2 rounded-lg border border-hairline-strong bg-surface-1 px-4 text-sm font-semibold text-ink outline-none hover:bg-surface-2 focus-visible:shadow-[var(--focus-ring)]"
      >
        <span>{action.label}</span>
        <Icon name={action.icon ?? 'arrow-right'} class="size-4" />
      </a>
    {:else}
      <button
        type="button"
        class="inline-flex min-h-11 shrink-0 items-center justify-center gap-2 rounded-lg border border-hairline-strong bg-surface-1 px-4 text-sm font-semibold text-ink outline-none hover:bg-surface-2 focus-visible:shadow-[var(--focus-ring)]"
        aria-pressed={action.pressed}
        onclick={action.onclick}
      >
        <Icon name={action.icon ?? 'plus'} class="size-4" />
        <span>{action.label}</span>
      </button>
    {/if}
  {/if}
</header>
