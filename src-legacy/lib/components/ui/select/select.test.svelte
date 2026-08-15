<script lang="ts">
  import * as Select from './index.js';

  const options = [
    { value: 'original', label: 'Original' },
    { value: 'form', label: 'Form' },
    { value: 'tree', label: 'Tree' },
  ];
  let value = $state('original');
  let selectedLabel = $derived(options.find((option) => option.value === value)?.label ?? 'Choose');
</script>

<div data-testid="select-owner">
  <Select.Root type="single" bind:value items={options}>
    <Select.Trigger aria-label="Rule mode">
      <span>{selectedLabel}</span>
    </Select.Trigger>
    <Select.Content>
      <Select.Group>
        <Select.GroupHeading>Modes</Select.GroupHeading>
        {#each options as option (option.value)}
          <Select.Item value={option.value} label={option.label}>{option.label}</Select.Item>
        {/each}
      </Select.Group>
    </Select.Content>
  </Select.Root>
  <output aria-label="Selected mode">{value}</output>
</div>
