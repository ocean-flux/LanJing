<script lang="ts" module>
  export type PageFrameWidth = 'standard' | 'narrow' | 'workspace';

  const widthClasses: Record<PageFrameWidth, string> = {
    standard: 'max-w-(--measure-standard)',
    narrow: 'max-w-(--measure-narrow)',
    workspace: 'max-w-(--measure-workspace)',
  };
</script>

<script lang="ts">
  import type { Snippet } from 'svelte';
  import { cn } from '$lib/utils.js';

  type Props = {
    width?: PageFrameWidth;
    class?: string;
    children: Snippet;
  };

  let { width = 'standard', class: className, children }: Props = $props();
</script>

<div
  data-slot="page-frame"
  data-width={width}
  class={cn(
    'mx-auto flex w-full min-w-0 flex-col gap-(--section-gap) pb-(--section-gap)',
    widthClasses[width],
    className,
  )}
>
  {@render children()}
</div>
