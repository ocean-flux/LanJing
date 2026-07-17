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

<!-- 全宽轻量：与 denselist 节奏对齐，不居中窄杂志栏 -->
<div class="min-h-full w-full max-w-none">
  <RealmStatePanel {state} {sources} />
</div>
