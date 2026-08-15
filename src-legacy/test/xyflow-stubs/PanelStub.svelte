<script lang="ts">
  //! @xyflow/svelte Panel 的测试替身：渲染为带 position 的容器。

  let { children, position, ...rest } = $props();
</script>

<div data-testid="flow-panel" data-position={position} {...rest}>
  {@render children?.()}
</div>
