<script lang="ts">
  //! @xyflow/svelte SvelteFlow 的测试替身：
  //! 挂载时把接收到的 props（isValidConnection/onconnect/onnodeschange 等）写入
  //! 共享 captured 对象，供测试直接调用回调；children 原样渲染。

  import { onMount } from 'svelte';
  import { setCaptured } from './capture';

  let { children, ...props } = $props();

  onMount(() => {
    setCaptured(props);
  });
</script>

<div
  data-testid="svelte-flow"
  data-nodes-count={props.nodes?.length ?? 0}
  data-edges-count={props.edges?.length ?? 0}
>
  {@render children?.()}
</div>
