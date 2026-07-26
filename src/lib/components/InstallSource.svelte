<script lang="ts">
  import SourceInstallEntry from '$lib/views/sources/SourceInstallEntry.svelte';

  type Props = {
    showHeading?: boolean;
    onInstalled?: () => void;
  };

  let { showHeading = true, onInstalled }: Props = $props();
</script>

<SourceInstallEntry {showHeading} {onInstalled} />
