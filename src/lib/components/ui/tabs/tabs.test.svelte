<script lang="ts">
  import * as Tabs from './index.js';

  let active = $state('original');
</script>

<Tabs.Root bind:value={active}>
  <Tabs.List aria-label="Rule modes">
    <Tabs.Trigger value="original">Original</Tabs.Trigger>
    <Tabs.Trigger value="form">Form</Tabs.Trigger>
    <Tabs.Trigger value="tree">Tree</Tabs.Trigger>
  </Tabs.List>
  <Tabs.Content value="original">Original panel</Tabs.Content>
  <Tabs.Content value="form">Form panel</Tabs.Content>
  <Tabs.Content value="tree">Tree panel</Tabs.Content>
</Tabs.Root>
