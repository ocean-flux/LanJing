<script lang="ts">
  import { getPlatformContext } from './platform-context.svelte';

  const platformContext = getPlatformContext();
</script>

<output data-testid="platform-context-probe">{platformContext.platform}</output>
