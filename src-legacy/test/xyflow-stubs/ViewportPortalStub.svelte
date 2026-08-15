<script lang="ts">
  let { target, children } = $props();
</script>

<div data-testid="viewport-portal" data-target={target}>
  {@render children?.()}
</div>
