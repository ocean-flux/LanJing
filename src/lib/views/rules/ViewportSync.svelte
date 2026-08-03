<script lang="ts">
  import { useSvelteFlow } from '@xyflow/svelte';
  import { onMount } from 'svelte';

  const { fitView } = useSvelteFlow();
  onMount(() => {
    queueMicrotask(() => void fitView({ padding: 0.3, duration: 180 }));
  });
</script>
