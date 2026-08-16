//! 节点配置里的 JS 编辑器。
//!
//! CodeMirror 6 只在这里用到，且规则工作区本身已经是懒加载路由，
//! 但编辑器仍然按需再切一刀：只有真正选中 js / condition-js / loop-js
//! 节点时才付这份下载成本。

import { useEffect, useId, useRef, useState } from 'react';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/shared/utils';
import { useMessages } from '@/shared/i18n/messages';

// 这些只是类型导入，编译期被完全擦除，不会把 CodeMirror 拉进主 bundle。
import type * as CmAutocomplete from '@codemirror/autocomplete';
import type * as CmCommands from '@codemirror/commands';
import type * as CmJavascript from '@codemirror/lang-javascript';
import type * as CmLanguage from '@codemirror/language';
import type * as CmSearch from '@codemirror/search';
import type * as CmState from '@codemirror/state';
import type * as CmView from '@codemirror/view';

/** CodeMirror 的模块集合；首次加载后缓存，后续挂载直接复用。 */
type CodeMirrorModules = {
  state: typeof CmState;
  view: typeof CmView;
  commands: typeof CmCommands;
  javascript: typeof CmJavascript;
  language: typeof CmLanguage;
  autocomplete: typeof CmAutocomplete;
  search: typeof CmSearch;
};

let modulesPromise: Promise<CodeMirrorModules> | null = null;

function loadCodeMirror(): Promise<CodeMirrorModules> {
  modulesPromise ??= Promise.all([
    import('@codemirror/state'),
    import('@codemirror/view'),
    import('@codemirror/commands'),
    import('@codemirror/lang-javascript'),
    import('@codemirror/language'),
    import('@codemirror/autocomplete'),
    import('@codemirror/search'),
  ]).then(([state, view, commands, javascript, language, autocomplete, search]) => ({
    state,
    view,
    commands,
    javascript,
    language,
    autocomplete,
    search,
  }));
  return modulesPromise;
}

export type JsCodeEditorProps = {
  value: string;
  onChange: (code: string) => void;
  /** 编辑器最小高度（行数）。 */
  minLines?: number;
  'aria-label'?: string;
};

export function JsCodeEditor({
  value,
  onChange,
  minLines = 12,
  'aria-label': ariaLabel,
}: JsCodeEditorProps) {
  const m = useMessages();
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<CmView.EditorView | null>(null);
  // 把 onChange 存进 ref：CodeMirror 的 updateListener 在初始化时闭包捕获，
  // 让它进 effect 依赖会导致父组件每次重渲染都重建整个编辑器。
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  // 同理：模块下载期间 value 可能已经变了（撤销、外部改配置），
  // 建 EditorState 时要用最新值，不能用 effect 起跑那一刻的闭包快照。
  const valueRef = useRef(value);
  valueRef.current = value;
  const [ready, setReady] = useState(false);
  const [failed, setFailed] = useState(false);
  const labelId = useId();

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    let disposed = false;
    loadCodeMirror()
      .then((cm) => {
        if (disposed) return;
        const view = new cm.view.EditorView({
          parent: host,
          state: cm.state.EditorState.create({
            doc: valueRef.current,
            extensions: [
              cm.view.lineNumbers(),
              cm.view.highlightActiveLineGutter(),
              cm.view.highlightSpecialChars(),
              cm.view.drawSelection(),
              cm.view.rectangularSelection(),
              cm.view.crosshairCursor(),
              cm.view.keymap.of([
                ...cm.commands.defaultKeymap,
                ...cm.commands.historyKeymap,
                ...cm.autocomplete.completionKeymap,
                ...cm.search.searchKeymap,
                cm.commands.indentWithTab,
              ]),
              cm.commands.history(),
              cm.language.bracketMatching(),
              cm.language.indentOnInput(),
              cm.language.syntaxHighlighting(cm.language.defaultHighlightStyle, {
                fallback: true,
              }),
              cm.autocomplete.closeBrackets(),
              cm.autocomplete.autocompletion(),
              cm.javascript.javascript(),
              cm.view.EditorView.lineWrapping,
              cm.view.EditorView.updateListener.of((update) => {
                if (update.docChanged) onChangeRef.current(update.state.doc.toString());
              }),
              cm.view.EditorView.contentAttributes.of({
                'aria-labelledby': labelId,
              }),
            ],
          }),
        });
        viewRef.current = view;
        setReady(true);
      })
      .catch(() => {
        if (!disposed) setFailed(true);
      });

    return () => {
      disposed = true;
      viewRef.current?.destroy();
      viewRef.current = null;
    };
    // 依赖只留 labelId：初始文档走 valueRef，onChange 走 onChangeRef，
    // 把它们列进来会让父组件的每次重渲染都重建编辑器。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [labelId]);

  // 外部改了配置（撤销、切换节点）时把文档拉回来；自己的输入不会命中，
  // 因为那时 value 已经等于 doc。ready 进依赖是为了让编辑器建好之后补一次同步。
  useEffect(() => {
    const view = viewRef.current;
    if (!view || view.state.doc.toString() === value) return;
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: value } });
  }, [value, ready]);

  if (failed) {
    // 编辑器加载失败时退回 textarea，脚本仍然可编辑。
    return (
      <textarea
        aria-label={ariaLabel}
        className="w-full resize-y border border-hairline bg-surface-1 p-2 font-mono text-code text-ink"
        style={{ minHeight: `${minLines * 1.4}em` }}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    );
  }

  return (
    <div className="relative">
      <span id={labelId} className="sr-only">
        {ariaLabel ?? m.rules_js_host_title()}
      </span>
      <div
        ref={hostRef}
        className={cn('js-code-editor border border-hairline bg-surface-1', !ready && 'opacity-0')}
        style={{ ['--js-editor-min-height' as string]: `${minLines * 1.4}em` }}
      />
      {ready ? null : (
        <div
          className="absolute inset-0 flex items-center justify-center gap-2 text-ui-sm text-ink-subtle"
          style={{ minHeight: `${minLines * 1.4}em` }}
        >
          <Spinner />
          <span>{m.rules_js_host_loading()}</span>
        </div>
      )}
    </div>
  );
}
