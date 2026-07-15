<script lang="ts">
  import { noSourceRealmState } from '$lib/app/demo-state';
  import type { RealmState, SourceCardState } from '$lib/app/shell-types';
  import RealmStatePanel from './RealmStatePanel.svelte';

  type Props = {
    state?: RealmState;
    sources?: SourceCardState[];
  };

  let { state = noSourceRealmState, sources = [] }: Props = $props();
</script>

<div class="mx-auto min-h-full max-w-[var(--content-max-width)]">
  <RealmStatePanel {state} {sources} />
</div>
