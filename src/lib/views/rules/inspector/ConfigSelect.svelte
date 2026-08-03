<script lang="ts">
  import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
  } from '$lib/components/ui/select/index.js';
  import { cn } from '$lib/utils.js';

  export type ConfigSelectOption = {
    value: string;
    label: string;
  };

  type Props = {
    value: string;
    options: readonly ConfigSelectOption[];
    onChange: (value: string) => void;
    label?: string;
    id?: string;
    class?: string;
    disabled?: boolean;
  };

  let { value, options, onChange, label, id, class: className, disabled = false }: Props = $props();

  const selectedLabel = $derived(options.find((option) => option.value === value)?.label ?? value);
</script>

<Select type="single" {value} {disabled} onValueChange={onChange}>
  <SelectTrigger {id} class={cn('w-full', className)} aria-label={label}>
    <span class="truncate">{selectedLabel}</span>
  </SelectTrigger>
  <SelectContent>
    {#each options as option (option.value)}
      <SelectItem value={option.value}>{option.label}</SelectItem>
    {/each}
  </SelectContent>
</Select>
