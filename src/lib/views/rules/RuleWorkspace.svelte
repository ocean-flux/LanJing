<script lang="ts">
  import { getLocale, m } from '$lib/i18n';
  import PageFrame from '$lib/components/PageFrame.svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Notice from '$lib/components/Notice.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import Icon from '$lib/components/Icon.svelte';
  import { Button } from '$lib/components/ui/button/index.js';
  import {
    Sheet,
    SheetContent,
    SheetTitle,
    SheetHeader,
    SheetDescription,
  } from '$lib/components/ui/sheet/index.js';
  import { ScrollArea } from '$lib/components/ui/scroll-area/index.js';
  import TemplateWizard from './TemplateWizard.svelte';
  import NativeRuleImport from './NativeRuleImport.svelte';
  import RuleFlowCanvas from './RuleFlowCanvas.svelte';
  import NodeInspector from './inspector/NodeInspector.svelte';
  import EdgeInspector from './inspector/EdgeInspector.svelte';
  import PortInspector from './inspector/PortInspector.svelte';
  import LoopRegionInspector from './inspector/LoopRegionInspector.svelte';
  import DiagnosticList from './diagnostics/DiagnosticList.svelte';
  import DefinitionPreview from './preview/DefinitionPreview.svelte';
  import type {
    NativeRuleDocumentSummary,
    FlowNode,
    FlowNodeKind,
  } from '$lib/rules/native-authoring/wire';
  import { listNativeRuleDocuments } from '$lib/rules/native-authoring/wire';
  import { NativeRuleEditorSession } from '$lib/rules/native-authoring/session.svelte';
  import { parseEditorSelection } from '$lib/rules/native-authoring/flow-adapter';

  import { onMount } from 'svelte';

  function localizedMessage(key: string, fallback: { en: string; 'zh-CN': string }): string {
    const candidate = (m as unknown as Record<string, () => string>)[key];
    if (typeof candidate === 'function') return candidate();
    if (getLocale() === 'en') return fallback.en;
    return fallback['zh-CN'];
  }

  const validationCopy = {
    validate: () => localizedMessage('rules_validate', { en: 'Validate', 'zh-CN': '校验' }),
    validating: () =>
      localizedMessage('rules_validating', { en: 'Validating…', 'zh-CN': '正在校验…' }),
    prepare: () =>
      localizedMessage('rules_prepare', { en: 'Prepare install', 'zh-CN': '准备安装' }),
    required: () =>
      localizedMessage('rules_validation_required', {
        en: 'Save and validate this revision first',
        'zh-CN': '请先保存并通过校验',
      }),
    prepared: () =>
      localizedMessage('rules_prepared', {
        en: 'Install candidate prepared',
        'zh-CN': '已准备安装候选',
      }),
  };

  /** 唯一页面 owner：持有 session，组件只调 session 方法、读 session 投影。 */
  const session = new NativeRuleEditorSession();

  let documents = $state<NativeRuleDocumentSummary[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let selectedDocId = $state<string | null>(null);
  let showWizard = $state(false);
  let showPreview = $state(false);
  let showDiagnostics = $state(false);
  let inspectorSheetOpen = $state(false);
  let diagnosticsSheetOpen = $state(false);
  let previewSheetOpen = $state(false);
  let documentSheetOpen = $state(false);
  let operationError = $state<string | null>(null);
  let operationNotice = $state<string | null>(null);

  // 平板及移动端把次要 rails 收进 Sheet，主画布保持可用宽度。
  let isNarrow = $state(false);
  let mql: MediaQueryList | undefined;

  onMount(() => {
    mql = window.matchMedia('(max-width: 1023px)');
    isNarrow = mql.matches;
    const handler = () => {
      isNarrow = mql!.matches;
    };
    mql.addEventListener('change', handler);
    loadDocuments();
    return () => mql?.removeEventListener('change', handler);
  });

  // ---- 会话投影（响应式） ----
  const sessionTitle = $derived(session.title);
  const sessionSaving = $derived(session.isSaving);
  const sessionConflict = $derived(session.conflict);
  const hasUnsaved = $derived(session.hasUnsavedChanges);
  const canUndo = $derived(session.canUndo);
  const canRedo = $derived(session.canRedo);
  const validation = $derived(session.validation);
  const validationBusy = $derived(validation.status === 'pending');
  const canPrepare = $derived(
    validation.status === 'valid' && !session.dirtySemantic && !session.conflict.semantic,
  );

  // ---- 选中节点投影（NodeInspector 输入） ----
  const editorSelection = $derived(parseEditorSelection(session.selection));
  const selectedNode = $derived.by(() => {
    if (editorSelection.kind !== 'node') return null;
    const node = session.definition.flow.nodes.find(
      (candidate) => candidate.id === editorSelection.nodeId,
    );
    if (!node) return null;
    return node;
  });
  const selectedNodeType = $derived<FlowNodeKind | null>(selectedNode?.config.kind ?? null);
  const selectedNodeConfig = $derived<Record<string, unknown> | null>(
    selectedNode?.config.value ?? null,
  );
  const selectedEdgeId = $derived(editorSelection.kind === 'edge' ? editorSelection.edgeId : null);
  const selectedEdge = $derived.by(() => {
    if (!selectedEdgeId) return null;
    return session.flowProjection.edges.find((edge) => edge.id === selectedEdgeId) ?? null;
  });
  const selectedEdgeSourceNode = $derived.by((): FlowNode | null => {
    const nodeId = selectedEdge?.data.edge.from.node_id;
    return nodeId
      ? (session.definition.flow.nodes.find((node) => node.id === nodeId) ?? null)
      : null;
  });
  const selectedEdgeTargetNode = $derived.by((): FlowNode | null => {
    const nodeId = selectedEdge?.data.edge.to.node_id;
    return nodeId
      ? (session.definition.flow.nodes.find((node) => node.id === nodeId) ?? null)
      : null;
  });
  const selectedPortNode = $derived.by((): FlowNode | null => {
    if (editorSelection.kind !== 'port') return null;
    return session.definition.flow.nodes.find((node) => node.id === editorSelection.nodeId) ?? null;
  });
  const selectedLoopRegion = $derived.by(() => {
    if (editorSelection.kind !== 'loopRegion') return null;
    return (
      session.flowProjection.loopRegions.find(
        (region) => region.loopNodeId === editorSelection.loopNodeId,
      ) ?? null
    );
  });
  const inspectorHasSelection = $derived(editorSelection.kind !== 'none');

  // 离开守卫：未保存更改时提示
  onMount(() => {
    const handler = (event: BeforeUnloadEvent) => {
      if (session.hasUnsavedChanges) {
        event.preventDefault();
      }
    };
    window.addEventListener('beforeunload', handler);
    return () => window.removeEventListener('beforeunload', handler);
  });

  async function loadDocuments() {
    loading = true;
    loadError = null;
    try {
      const summaries = await listNativeRuleDocuments();
      documents = summaries;
      if (!selectedDocId && summaries[0]) {
        await selectDocument(summaries[0].document_id);
      }
    } catch (caught) {
      loadError = errorMessage(caught);
    } finally {
      loading = false;
    }
  }

  async function selectDocument(id: string) {
    selectedDocId = id;
    loadError = null;
    operationError = null;
    operationNotice = null;
    try {
      await session.loadDocument(id);
    } catch (caught) {
      selectedDocId = null;
      loadError = errorMessage(caught);
    }
  }

  function openWizard() {
    showWizard = true;
  }
  /** 提取可读错误文本（Tauri 错误是 { message, code } 结构）。 */
  function errorMessage(caught: unknown): string {
    if (caught instanceof Error) return caught.message;
    if (typeof caught === 'object' && caught !== null) {
      const msg = (caught as { message?: unknown }).message;
      if (typeof msg === 'string' && msg) return msg;
    }
    return String(caught);
  }
  function closeWizard() {
    showWizard = false;
  }

  async function handleWizardCreated() {
    closeWizard();
    // 新文档已由 session 生命周期方法打开；列表与选中项同步到同一文档。
    if (session.documentId) selectedDocId = session.documentId;
    documents = await listNativeRuleDocuments();
  }

  async function handleSave() {
    operationError = null;
    operationNotice = null;
    try {
      await session.save();
    } catch (caught) {
      operationError = errorMessage(caught);
    }
  }

  async function handleValidate() {
    operationError = null;
    operationNotice = null;
    try {
      await session.validate();
      showDiagnostics = true;
      if (isNarrow) diagnosticsSheetOpen = true;
    } catch (caught) {
      operationError = errorMessage(caught);
    }
  }

  async function handlePrepare() {
    operationError = null;
    operationNotice = null;
    try {
      await session.prepare();
      operationNotice = validationCopy.prepared();
    } catch (caught) {
      operationError =
        caught instanceof Error && caught.message === 'document_validation_required'
          ? validationCopy.required()
          : errorMessage(caught);
    }
  }

  function handleUndo() {
    session.undo();
  }
  function handleRedo() {
    session.redo();
  }

  function handleNodeChange(patch: Record<string, unknown>) {
    operationError = null;
    operationNotice = null;
    const id = selectedNode?.id;
    if (!id) return;
    session.setNodeConfig(id, patch);
  }

  function handleEdgeDelete() {
    const edge = selectedEdge;
    if (!edge) return;
    session.disconnect(edge.data.edge);
    session.selectNode(null);
  }

  function handlePortEdit() {
    if (selectedPortNode) session.selectNode(selectedPortNode.id);
  }

  function handleLoopRegionToggle(collapsed: boolean) {
    if (selectedLoopRegion) {
      session.collapseLoopRegion(selectedLoopRegion.loopNodeId, collapsed);
    }
  }
</script>

<PageFrame
  width="workspace"
  class="rules-page-frame h-full min-h-0 flex-1 gap-0! pb-0! in-data-[chrome-family=bottom]:pb-0"
>
  <div class="rules-workspace flex h-full min-h-0 flex-1 flex-col overflow-hidden">
    <div class="rules-page-heading shrink-0">
      <PageHeader
        title={m.rules_title()}
        description={m.rules_workspace_title()}
        action={{ label: m.rules_new_rule(), icon: 'plus', onclick: openWizard }}
      />
    </div>

    {#if loading}
      <Notice tone="info" role="status" icon="arrow-clockwise" class="w-full">
        {m.rules_loading()}
      </Notice>
    {:else if loadError && !selectedDocId}
      <div class="flex flex-col gap-2">
        <Notice tone="danger" role="alert" title={m.rules_load_error()} icon="warning-circle">
          <span class="break-words">{loadError}</span>
        </Notice>
        <Button type="button" variant="outline" onclick={loadDocuments}>
          <Icon name="arrow-clockwise" class="size-4" />
          <span>{m.action_retry()}</span>
        </Button>
      </div>
    {:else if selectedDocId && sessionTitle}
      <div class="rules-workbench flex min-h-0 flex-1 gap-2 overflow-hidden">
        <aside
          class="rules-document-rail flex w-56 shrink-0 flex-col overflow-hidden rounded-lg border border-hairline bg-surface-panel"
        >
          <div class="flex items-center gap-2 border-b border-hairline px-3 py-2">
            <div class="min-w-0 flex-1">
              <h2 class="truncate text-xs font-semibold text-ink">{m.rules_document_list()}</h2>
              <p class="mt-0.5 text-[11px] text-ink-subtle">
                {m.rules_document_count({ count: documents.length })}
              </p>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              aria-label={m.rules_new_rule()}
              title={m.rules_new_rule()}
              onclick={openWizard}
            >
              <Icon name="plus" class="size-3.5" />
            </Button>
          </div>

          <ScrollArea class="min-h-0 flex-1">
            <div class="flex flex-col gap-0.5 p-1.5">
              {#each documents as doc (doc.document_id)}
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  class="group min-h-11 w-full justify-start gap-2 rounded-md border border-transparent px-2 py-1.5 text-left hover:bg-surface-2 aria-pressed:border-lantern-strong/30 aria-pressed:bg-lantern-soft"
                  aria-pressed={selectedDocId === doc.document_id}
                  onclick={() => selectDocument(doc.document_id)}
                >
                  <span
                    class="flex size-7 shrink-0 items-center justify-center rounded-md bg-surface-2 text-ink-subtle group-aria-pressed:bg-lantern-tint group-aria-pressed:text-lantern-strong"
                  >
                    <Icon name="file-text" class="size-3.5" />
                  </span>
                  <span class="min-w-0 flex-1">
                    <span class="block truncate text-xs font-medium text-ink">{doc.title}</span>
                    <span class="mt-0.5 block truncate font-mono text-[10px] text-ink-subtle">
                      {doc.state === 'draft' ? m.rules_state_draft() : m.rules_state_linked()}
                    </span>
                  </span>
                </Button>
              {/each}
            </div>
          </ScrollArea>

          <div class="border-t border-hairline p-2">
            <div class="flex flex-col gap-2">
              <Button type="button" variant="outline" size="sm" class="w-full" onclick={openWizard}>
                <Icon name="file-plus" class="size-3.5" />
                <span>{m.rules_new_rule()}</span>
              </Button>
              <NativeRuleImport {session} oncreated={handleWizardCreated} class="w-full" />
            </div>
          </div>
        </aside>

        <main
          class="rules-canvas-column flex min-w-0 flex-1 flex-col overflow-hidden rounded-lg border border-hairline bg-canvas"
        >
          <div
            class="rules-editor-toolbar flex min-h-11 shrink-0 items-center gap-2 border-b border-hairline bg-surface-panel px-2"
          >
            {#if isNarrow}
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                aria-label={m.rules_document_list()}
                title={m.rules_document_list()}
                onclick={() => (documentSheetOpen = true)}
              >
                <Icon name="list-bullets" class="size-4" />
              </Button>
            {/if}
            <div class="min-w-0 flex-1">
              <div class="flex min-w-0 items-center gap-2">
                <span class="truncate text-sm font-semibold text-ink">{sessionTitle}</span>
                <span
                  class="hidden shrink-0 items-center gap-1 text-[11px] sm:inline-flex"
                  class:text-warning={hasUnsaved}
                  class:text-ink-subtle={!hasUnsaved}
                >
                  <Icon name={hasUnsaved ? 'warning-circle' : 'check-circle'} class="size-3" />
                  {hasUnsaved ? m.rules_unsaved_changes() : m.rules_state_draft()}
                </span>
              </div>
              <code class="hidden truncate font-mono text-[10px] text-ink-subtle lg:block">
                {session.documentId}
              </code>
            </div>

            <div
              class="flex items-center gap-0.5"
              role="toolbar"
              aria-label={m.rules_workspace_title()}
            >
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                aria-label={m.rules_undo()}
                title={m.rules_undo()}
                disabled={!canUndo}
                onclick={handleUndo}
              >
                <Icon name="arrow-counter-clockwise" class="size-4" />
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                aria-label={m.rules_redo()}
                title={m.rules_redo()}
                disabled={!canRedo}
                onclick={handleRedo}
              >
                <Icon name="arrow-clockwise" class="size-4" />
              </Button>
              <span class="mx-1 hidden h-5 w-px bg-hairline sm:block"></span>
              <Button
                type="button"
                variant={showDiagnostics ? 'secondary' : 'ghost'}
                size="sm"
                disabled={validationBusy ||
                  session.dirtySemantic ||
                  Boolean(session.conflict.semantic)}
                aria-busy={validationBusy}
                aria-label={validationBusy
                  ? validationCopy.validating()
                  : validationCopy.validate()}
                title={validationBusy ? validationCopy.validating() : validationCopy.validate()}
                onclick={handleValidate}
              >
                <Icon name="shield-check" class="size-3.5" />
                <span class="hidden lg:inline"
                  >{validationBusy ? validationCopy.validating() : validationCopy.validate()}</span
                >
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                disabled={!canPrepare || sessionSaving}
                aria-label={validationCopy.prepare()}
                title={validationCopy.prepare()}
                onclick={handlePrepare}
              >
                <Icon name="upload-simple" class="size-3.5" />
                <span class="hidden lg:inline">{validationCopy.prepare()}</span>
              </Button>
              <Button
                type="button"
                variant={showDiagnostics ? 'secondary' : 'ghost'}
                size="sm"
                aria-pressed={showDiagnostics}
                aria-label={m.rules_diagnostics()}
                title={m.rules_diagnostics()}
                onclick={() => {
                  showDiagnostics = !showDiagnostics;
                  if (isNarrow) diagnosticsSheetOpen = showDiagnostics;
                }}
              >
                <Icon name="warning-circle" class="size-3.5" />
                <span class="hidden md:inline">{m.rules_diagnostics()}</span>
              </Button>
              <NativeRuleImport {session} oncreated={handleWizardCreated} class="shrink-0" />
              <Button
                type="button"
                variant={showPreview ? 'secondary' : 'ghost'}
                size="sm"
                aria-pressed={showPreview}
                onclick={() => {
                  showPreview = !showPreview;
                  if (isNarrow) previewSheetOpen = showPreview;
                }}
              >
                <Icon name="eye" class="size-3.5" />
                <span class="hidden md:inline">{m.rules_preview_title()}</span>
              </Button>
              <Button
                type="button"
                variant="secondary"
                size="sm"
                disabled={!hasUnsaved || sessionSaving}
                aria-busy={sessionSaving}
                onclick={handleSave}
              >
                <Icon name="floppy-disk" class="size-3.5" />
                <span class="hidden sm:inline"
                  >{sessionSaving ? m.rules_saving() : m.rules_save()}</span
                >
              </Button>
            </div>
          </div>

          {#if sessionConflict.semantic || sessionConflict.layout}
            <Notice
              tone="danger"
              role="alert"
              title={m.rules_conflict_title()}
              icon="warning-circle"
              class="m-2 shrink-0"
            >
              {m.rules_conflict_hint()}
            </Notice>
          {/if}
          {#if operationError}
            <Notice tone="danger" role="alert" icon="warning-circle" class="m-2 shrink-0">
              <span class="break-words">{operationError}</span>
            </Notice>
          {:else if operationNotice}
            <Notice tone="info" role="status" icon="check-circle" class="m-2 shrink-0">
              {operationNotice}
            </Notice>
          {/if}

          <div class="relative min-h-0 flex-1">
            <RuleFlowCanvas {session} class="absolute inset-0" />

            {#if showDiagnostics && !isNarrow}
              <aside
                class="rules-floating-panel absolute top-3 right-3 bottom-3 z-10 w-80 overflow-auto rounded-lg border border-hairline-strong bg-surface-overlay p-3 shadow-(--surface-overlay-shadow)"
              >
                <DiagnosticList {session} />
              </aside>
            {/if}
            {#if showPreview && !isNarrow}
              <aside
                class="rules-floating-panel absolute top-3 right-3 bottom-3 z-10 w-80 overflow-auto rounded-lg border border-hairline-strong bg-surface-overlay p-3 shadow-(--surface-overlay-shadow)"
              >
                <DefinitionPreview {session} />
              </aside>
            {/if}

            {#if isNarrow && inspectorHasSelection}
              <Button
                type="button"
                variant="secondary"
                size="sm"
                class="absolute right-3 bottom-3 z-10 shadow-(--surface-overlay-shadow)"
                onclick={() => (inspectorSheetOpen = true)}
              >
                <Icon name="pencil-simple" class="size-3.5" />
                <span
                  >{selectedEdge
                    ? m.rules_edge_inspector_title()
                    : editorSelection.kind === 'port'
                      ? '端口属性'
                      : editorSelection.kind === 'loopRegion'
                        ? 'Loop 区域'
                        : m.rules_node_inspector()}</span
                >
              </Button>
            {/if}
          </div>
        </main>

        {#if !isNarrow}
          <aside
            class="rules-inspector-rail w-80 shrink-0 overflow-y-auto rounded-lg border border-hairline bg-surface-panel p-3"
          >
            <div class="mb-3 flex items-center gap-2 border-b border-hairline pb-2">
              <Icon
                name={selectedEdge
                  ? 'arrow-right'
                  : editorSelection.kind === 'port'
                    ? 'git-merge'
                    : 'gear-six'}
                class="size-4 text-lantern-strong"
              />
              <h2 class="text-xs font-semibold text-ink">
                {selectedEdge
                  ? m.rules_edge_inspector_title()
                  : editorSelection.kind === 'port'
                    ? '端口属性'
                    : editorSelection.kind === 'loopRegion'
                      ? 'Loop 区域'
                      : m.rules_node_inspector()}
              </h2>
            </div>
            {#if selectedEdge}
              <EdgeInspector
                edge={selectedEdge}
                sourceNode={selectedEdgeSourceNode}
                targetNode={selectedEdgeTargetNode}
                onDelete={handleEdgeDelete}
              />
            {:else if editorSelection.kind === 'port'}
              <PortInspector
                nodeId={selectedPortNode?.id ?? null}
                node={selectedPortNode}
                direction={editorSelection.direction}
                handle={editorSelection.handle}
                edges={session.flowProjection.edges}
                onEditNode={handlePortEdit}
              />
            {:else if selectedLoopRegion}
              <LoopRegionInspector
                region={selectedLoopRegion}
                onToggleCollapsed={handleLoopRegionToggle}
              />
            {:else}
              <NodeInspector
                nodeId={selectedNode?.id ?? null}
                nodeType={selectedNodeType}
                config={selectedNodeConfig}
                onChange={handleNodeChange}
              />
            {/if}
          </aside>
        {/if}
      </div>
    {:else if selectedDocId}
      <Notice tone="danger" role="alert" icon="warning-circle">
        <span class="break-words">{loadError}</span>
      </Notice>
    {:else}
      <div class="flex min-h-0 flex-1 items-center justify-center">
        <EmptyState
          title={m.rules_workspace_title()}
          description={m.rules_no_documents()}
          icon="file-text"
          action={renderCreateAction}
        />
      </div>
    {/if}
  </div>

  <!-- 移动端 Inspector Sheet -->
  <Sheet bind:open={inspectorSheetOpen}>
    <SheetContent side="bottom" showCloseButton={true}>
      <SheetHeader>
        <SheetTitle
          >{selectedEdge
            ? m.rules_edge_inspector_title()
            : editorSelection.kind === 'port'
              ? '端口属性'
              : editorSelection.kind === 'loopRegion'
                ? 'Loop 区域'
                : m.rules_node_inspector()}</SheetTitle
        >
      </SheetHeader>
      {#if selectedEdge}
        <EdgeInspector
          edge={selectedEdge}
          sourceNode={selectedEdgeSourceNode}
          targetNode={selectedEdgeTargetNode}
          onDelete={handleEdgeDelete}
        />
      {:else if editorSelection.kind === 'port'}
        <PortInspector
          nodeId={selectedPortNode?.id ?? null}
          node={selectedPortNode}
          direction={editorSelection.direction}
          handle={editorSelection.handle}
          edges={session.flowProjection.edges}
          onEditNode={handlePortEdit}
        />
      {:else if selectedLoopRegion}
        <LoopRegionInspector
          region={selectedLoopRegion}
          onToggleCollapsed={handleLoopRegionToggle}
        />
      {:else}
        <NodeInspector
          nodeId={selectedNode?.id ?? null}
          nodeType={selectedNodeType}
          config={selectedNodeConfig}
          onChange={handleNodeChange}
        />
      {/if}
    </SheetContent>
  </Sheet>

  <!-- 移动端文档 Sheet -->
  <Sheet bind:open={documentSheetOpen}>
    <SheetContent side="left" showCloseButton={true} class="w-[min(86vw,22rem)] p-0">
      <SheetHeader class="border-b border-hairline px-4 py-3">
        <SheetTitle>{m.rules_document_list()}</SheetTitle>
        <SheetDescription>{m.rules_workspace_title()}</SheetDescription>
      </SheetHeader>
      <ScrollArea class="min-h-0 flex-1 px-2 py-2">
        <div class="flex flex-col gap-0.5">
          {#each documents as doc (doc.document_id)}
            <Button
              type="button"
              variant="ghost"
              size="sm"
              class="min-h-11 w-full justify-start gap-2 rounded-md px-2 text-left hover:bg-surface-2 aria-pressed:bg-lantern-soft"
              aria-pressed={selectedDocId === doc.document_id}
              onclick={() => {
                selectDocument(doc.document_id);
                documentSheetOpen = false;
              }}
            >
              <Icon name="file-text" class="size-4 text-ink-subtle" />
              <span class="min-w-0 flex-1 truncate text-sm text-ink">{doc.title}</span>
            </Button>
          {/each}
        </div>
      </ScrollArea>
      <div class="border-t border-hairline p-3">
        <Button type="button" class="w-full" onclick={openWizard}>
          <Icon name="file-plus" class="size-4" />
          <span>{m.rules_new_rule()}</span>
        </Button>
      </div>
    </SheetContent>
  </Sheet>

  <!-- 诊断 Sheet（移动端） -->
  <Sheet bind:open={diagnosticsSheetOpen}>
    <SheetContent side="bottom" showCloseButton={true}>
      <SheetHeader>
        <SheetTitle>{m.rules_diagnostics()}</SheetTitle>
      </SheetHeader>
      <DiagnosticList {session} />
    </SheetContent>
  </Sheet>

  <!-- 预览 Sheet（移动端） -->
  <Sheet bind:open={previewSheetOpen}>
    <SheetContent side="bottom" showCloseButton={true}>
      <SheetHeader>
        <SheetTitle>{m.rules_preview_title()}</SheetTitle>
      </SheetHeader>
      <DefinitionPreview {session} />
    </SheetContent>
  </Sheet>

  <!-- 模板向导 Sheet -->
  <Sheet bind:open={showWizard}>
    <SheetContent side="right" showCloseButton={true} class="sm:max-w-md">
      <SheetHeader>
        <SheetTitle>{m.rules_template_wizard_title()}</SheetTitle>
        <SheetDescription>{m.rules_template_wizard_intro()}</SheetDescription>
      </SheetHeader>
      <TemplateWizard {session} oncancel={closeWizard} oncreated={handleWizardCreated} />
    </SheetContent>
  </Sheet>
</PageFrame>

{#snippet renderCreateAction()}
  <div class="flex flex-wrap gap-2">
    <Button type="button" onclick={openWizard}>
      <Icon name="file-plus" class="size-4" />
      <span>{m.rules_from_template()}</span>
    </Button>
    <Button
      type="button"
      variant="outline"
      onclick={async () => {
        await session.createBlank();
        await handleWizardCreated();
      }}
    >
      <span>{m.rules_blank_graph()}</span>
    </Button>
    <NativeRuleImport {session} oncreated={handleWizardCreated} />
  </div>
{/snippet}

<style>
  :global(.rules-page-heading [data-slot='page-header']) {
    gap: 0.5rem;
    padding-bottom: 0.625rem;
  }

  :global(.rules-page-heading [data-slot='page-header'] h1) {
    font-size: 1.25rem;
  }

  :global(.rules-page-heading [data-slot='page-header'] p) {
    margin-top: 0;
    font-size: 0.75rem;
    line-height: 1rem;
  }

  .rules-workbench {
    min-height: 24rem;
    height: 100%;
  }

  :global(.rules-page-frame) {
    max-width: none;
    margin: calc(-1 * var(--section-gap)) calc(-1 * var(--page-gutter)) 0;
    padding: var(--section-gap) var(--page-gutter) 0;
  }

  @media (max-width: 1023px) {
    .rules-document-rail,
    .rules-inspector-rail {
      display: none;
    }
  }

  @media (max-width: 767px) {
    .rules-editor-toolbar {
      min-height: 3rem;
    }

    .rules-canvas-column {
      border-radius: 0.5rem;
    }
  }
</style>
