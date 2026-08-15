<script lang="ts">
  //! @xyflow/svelte Handle 的测试替身：渲染为可断言的 div，
  //! 暴露 handle id / type / position，供节点组件测试校验端口。

  let { id, type, position, class: className, children, ...rest } = $props();
</script>

<div
  data-testid="flow-handle"
  data-handle-id={id ?? ''}
  data-handle-type={type}
  data-position={position}
  class={className}
  {...rest}
>
  {@render children?.()}
</div>
