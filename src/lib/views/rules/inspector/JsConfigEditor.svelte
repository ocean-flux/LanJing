<script lang="ts">
  import { m } from '$lib/i18n';
  import { onMount, onDestroy } from 'svelte';
  import Icon from '$lib/components/Icon.svelte';
  import Notice from '$lib/components/Notice.svelte';

  type Props = {
    value: string;
    onChange: (value: string) => void;
    minLines?: number;
    readonly?: boolean;
  };

  let { value, onChange, minLines = 12, readonly = false }: Props = $props();

  let containerEl = $state<HTMLDivElement | undefined>();
  let editorReady = $state(false);
  let editorError = $state<string | null>(null);
  let lintCount = $state(0);
  let cmView: { destroy: () => void } | null = null;

  function captureRef(element: HTMLDivElement): void {
    containerEl = element;
  }

  /** 附件清理：元素卸载时释放引用（CM view 销毁由 onDestroy 负责）。 */
  function releaseRef(): void {
    containerEl = undefined;
  }

  onMount(async () => {
    const el = containerEl;
    if (!el) {
      editorError = '容器元素未就绪';
      return;
    }

    try {
      const mods = await Promise.all([
        import('@codemirror/state'),
        import('@codemirror/lang-javascript'),
        import('@codemirror/view'),
        import('@codemirror/language'),
        import('@codemirror/commands'),
        import('@codemirror/autocomplete'),
        import('@codemirror/lint'),
      ]);

      const [{ EditorState }] = mods;
      const [{ javascript }] = [mods[1]];
      const [{ EditorView, keymap }] = [mods[2]];
      const [{ syntaxHighlighting, defaultHighlightStyle }] = [mods[3]];
      const [{ defaultKeymap, history, historyKeymap }] = [mods[4]];
      const [{ autocompletion, completionKeymap }] = [mods[5]];
      const [{ lintGutter, linter }] = [mods[6]];

      const updateListener = EditorView.updateListener.of(
        (update: { docChanged: boolean; state: { doc: { toString: () => string } } }) => {
          if (update.docChanged) {
            onChange(update.state.doc.toString());
          }
        },
      );

      const jsLinter = linter(() => {
        const docText = el.textContent ?? '';
        try {
          Function(docText);
          lintCount = 0;
          return [];
        } catch (e) {
          lintCount = 1;
          return [
            {
              from: 0,
              to: docText.length,
              severity: 'error' as const,
              message: (e as Error).message,
            },
          ];
        }
      });

      const state = EditorState.create({
        doc: value,
        extensions: [
          javascript(),
          syntaxHighlighting(defaultHighlightStyle),
          history(),
          keymap.of([...defaultKeymap, ...historyKeymap, ...completionKeymap]),
          autocompletion(),
          lintGutter(),
          jsLinter,
          EditorView.editable.of(!readonly),
          EditorView.lineWrapping,
          updateListener,
        ],
      });

      cmView = new EditorView({ state, parent: el });
      editorReady = true;
    } catch (caught) {
      editorError = String(caught);
    }
  });

  onDestroy(() => {
    if (cmView) {
      cmView.destroy();
      cmView = null;
    }
  });
</script>

<div class="flex flex-col gap-2">
  <div class="flex items-center justify-between">
    <p class="text-xs font-medium text-ink-muted">{m.rules_js_host_title()}</p>
    {#if lintCount > 0}
      <span class="flex items-center gap-1 text-xs text-destructive">
        <Icon name="warning-circle" class="size-3" />
        {m.rules_js_host_lint_error()}
      </span>
    {/if}
  </div>

  {#if editorError}
    <Notice tone="danger" role="alert">{editorError}</Notice>
  {:else if !editorReady}
    <div class="flex items-center gap-2 text-xs text-ink-muted">
      <Icon name="arrow-clockwise" class="size-3" />
      {m.rules_js_host_loading()}
    </div>
  {/if}

  <div
    {@attach captureRef}
    {@attach releaseRef}
    class="overflow-hidden rounded-md border border-hairline bg-surface-1"
    class:min-h-[calc(var(--density-control-md)*3)]={!editorReady}
    role="textbox"
    aria-multiline="true"
    aria-label={m.rules_js_host_title()}
    style="min-height: {minLines * 20}px;"
  ></div>
</div>
