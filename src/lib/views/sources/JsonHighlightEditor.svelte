<script lang="ts">
  import { tokenizeJson, type JsonToken } from './json-highlight';
  import type { Attachment } from 'svelte/attachments';

  type Props = {
    id?: string;
    value?: string;
    placeholder?: string;
    disabled?: boolean;
    rows?: number;
    class?: string;
    ref?: HTMLTextAreaElement | null;
    ariaLabel?: string;
    onValueChange?: (value: string) => void;
  };

  let {
    id,
    value = $bindable(''),
    placeholder = '',
    disabled = false,
    rows = 7,
    class: className = '',
    ref = $bindable(null),
    ariaLabel,
    onValueChange,
  }: Props = $props();

  let mirrorEl: HTMLPreElement | null = null;

  const tokens = $derived.by((): JsonToken[] => {
    try {
      return tokenizeJson(value ?? '');
    } catch {
      return [{ kind: 'plain', text: value ?? '' }];
    }
  });

  function tokenClass(kind: JsonToken['kind']): string {
    switch (kind) {
      case 'key':
        return 'jh-key';
      case 'string':
        return 'jh-string';
      case 'number':
        return 'jh-number';
      case 'boolean':
        return 'jh-bool';
      case 'null':
        return 'jh-null';
      case 'punct':
        return 'jh-punct';
      case 'plain':
        return 'jh-plain';
      default:
        return '';
    }
  }

  const attachMirror: Attachment<HTMLPreElement> = (element) => {
    mirrorEl = element;
    return () => {
      if (mirrorEl === element) mirrorEl = null;
    };
  };

  const attachTextarea: Attachment<HTMLTextAreaElement> = (element) => {
    ref = element;
    return () => {
      if (ref === element) ref = null;
    };
  };

  function handleInput(event: Event): void {
    const nextValue = (event.currentTarget as HTMLTextAreaElement).value;
    if (onValueChange) {
      onValueChange(nextValue);
      return;
    }
    value = nextValue;
  }

  function syncScroll(): void {
    if (!ref || !mirrorEl) return;
    mirrorEl.scrollTop = ref.scrollTop;
    mirrorEl.scrollLeft = ref.scrollLeft;
  }
</script>

<div
  class="json-highlight-editor relative min-h-32 w-full overflow-hidden rounded-md border border-hairline bg-surface-2 {className}"
  data-testid="json-highlight-editor"
>
  <pre
    {@attach attachMirror}
    class="json-highlight-mirror pointer-events-none absolute inset-0 m-0 overflow-auto px-3 py-2 font-mono text-sm leading-5 break-words whitespace-pre-wrap text-ink"
    aria-hidden="true">{#if tokens.length === 0 || !(value ?? '')}<span class="jh-placeholder"
        >{placeholder}</span
      >{:else}{#each tokens as token, index (index)}{#if token.kind === 'ws'}{token.text}{:else}<span
            class={tokenClass(token.kind)}>{token.text}</span
          >{/if}{/each}{/if}</pre>
  <textarea
    {@attach attachTextarea}
    {id}
    value={value ?? ''}
    aria-label={ariaLabel}
    {disabled}
    {rows}
    spellcheck="false"
    autocomplete="off"
    autocapitalize="off"
    data-testid="json-highlight-input"
    class="relative z-10 min-h-32 w-full resize-y bg-transparent px-3 py-2 font-mono text-sm leading-5 break-words text-transparent caret-ink outline-none placeholder:text-transparent focus-visible:ring-2 focus-visible:ring-lantern/35 focus-visible:ring-inset disabled:cursor-not-allowed disabled:opacity-50"
    oninput={handleInput}
    onscroll={syncScroll}></textarea>
</div>

<style>
  .json-highlight-mirror :global(.jh-key) {
    color: var(--ink);
    font-weight: 500;
  }
  .json-highlight-mirror :global(.jh-string) {
    color: var(--lantern);
  }
  .json-highlight-mirror :global(.jh-number),
  .json-highlight-mirror :global(.jh-bool),
  .json-highlight-mirror :global(.jh-null) {
    color: var(--ink-muted);
  }
  .json-highlight-mirror :global(.jh-punct) {
    color: var(--ink-subtle);
  }
  .json-highlight-mirror :global(.jh-plain) {
    color: var(--ink);
  }
  .json-highlight-mirror :global(.jh-placeholder) {
    color: var(--ink-subtle);
  }
</style>
