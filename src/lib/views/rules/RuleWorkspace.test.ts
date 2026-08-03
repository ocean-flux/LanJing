import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import RuleWorkspace from './RuleWorkspace.svelte';
import type { NativeRuleDocumentSummary } from '$lib/rules/native-authoring/wire';

const mocks = vi.hoisted(() => ({
  listNativeRuleDocuments: vi.fn(),
  createNativeRuleDocument: vi.fn(),
  getNativeRuleDocument: vi.fn(),
  getNativeRuleProvenance: vi.fn(),
  saveNativeRuleDocument: vi.fn(),
  validateNativeRuleDocument: vi.fn(),
  prepareNativeRuleDocument: vi.fn(),
  renameNativeRuleDocument: vi.fn(),
  deleteNativeRuleDocument: vi.fn(),
}));

vi.mock('$lib/rules/native-authoring/wire', () => mocks);

/** 模拟 session：真实 session 依赖 invoke/Tauri，组件测试用轻量假实现。 */
vi.mock('$lib/rules/native-authoring/session.svelte', () => {
  class FakeSession {
    title: string | null = null;
    documentId: string | null = null;
    selection: string | null = null;
    definition = { flow: { nodes: [] }, intent_exports: {} };
    dirty = { semantic: false, layout: false };
    isSaving = false;
    conflict = { semantic: null, layout: null };
    hasUnsavedChanges = false;
    canUndo = false;
    canRedo = false;
    diagnostics: unknown[] = [];
    flowProjection = { nodes: [], edges: [] };
    flowProjectionSimple = { nodes: [], edges: [] };
    intentFocus: string | null = null;
    loadDocument = vi.fn(async (id: string) => {
      this.documentId = id;
      this.title = 'Test Rule';
    });
    save = vi.fn(async () => ({}));
    undo = vi.fn();
    redo = vi.fn();
    setNodeConfig = vi.fn();
    selectNode = vi.fn();
    createTemplate = vi.fn(async () => ({}));
    createBlank = vi.fn(async () => ({}));
    cleanup = vi.fn();
  }
  return { NativeRuleEditorSession: FakeSession };
});

vi.mock('$app/state', () => ({ page: { url: new URL('https://lanjing.test/rules') } }));

const docSummary: NativeRuleDocumentSummary = {
  document_id: 'doc:1',
  format: 'native_rule',
  title: 'Test Rule',
  source_identity: 'source:test',
  state: 'draft',
  semantic_revision: 1,
  layout_revision: 1,
  link_revision: 0,
  created_at_ms: 1_700_000_000_000,
  updated_at_ms: 1_700_000_000_000,
};

describe('RuleWorkspace', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('shows loading notice before the list resolves', () => {
    mocks.listNativeRuleDocuments.mockReturnValue(new Promise(() => {}));
    render(RuleWorkspace);
    expect(screen.getByRole('status').textContent).toContain('正在加载');
  });

  it('shows empty state with create actions when no documents', async () => {
    mocks.listNativeRuleDocuments.mockResolvedValue([]);
    render(RuleWorkspace);

    expect(await screen.findByText(/暂无规则文档/)).toBeTruthy();
    // 空状态在 denselist 与主区域各渲染一次创建入口
    expect(screen.getAllByRole('button', { name: /从模板创建/ }).length).toBeGreaterThan(0);
    expect(screen.getAllByRole('button', { name: /空白图/ }).length).toBeGreaterThan(0);
  });

  it('renders the document denselist when documents exist', async () => {
    mocks.listNativeRuleDocuments.mockResolvedValue([docSummary]);
    render(RuleWorkspace);

    expect((await screen.findAllByText('Test Rule')).length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText('草稿').length).toBeGreaterThanOrEqual(2);
  });

  it('selecting a document opens it in the session and shows the canvas bar', async () => {
    mocks.listNativeRuleDocuments.mockResolvedValue([docSummary]);
    render(RuleWorkspace);

    const item = (await screen.findAllByText('Test Rule'))[0];
    expect(item).toBeTruthy();
    await fireEvent.click(item);

    // 文档打开后出现保存/撤销/重做工具条
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /保存/ })).toBeTruthy();
    });
  });
});
