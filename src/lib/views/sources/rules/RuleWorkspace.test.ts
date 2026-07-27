import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { setPlatformContext } from '$lib/app/platform-context.svelte';
import {
  RuleEditorSession,
  type RuleEditorConflictSnapshot,
  type RuleEditorSessionApi,
} from '$lib/stores/rule-editor-session.svelte';
import type { InstallCandidate } from '$lib/stores/rules.svelte';
import type { MaskedSourceDocument, SourceDocumentSummary } from '../rule-editor-api';
import RuleConflictPanel from './RuleConflictPanel.svelte';
import RuleCredentialPanel from './RuleCredentialPanel.svelte';
import RuleWorkspace from './RuleWorkspace.svelte';

const initialText =
  '{"bookSourceType":0,"bookSourceUrl":"https://example.com","bookSourceName":"示例来源"}';

const summary: SourceDocumentSummary = {
  document_id: 'document:one',
  format: 'legado',
  title: '示例文档',
  state: 'draft',
  source_identity: null,
  revision: 1,
  installed_revision: null,
  masked_hash: 'masked:one',
  credential_slot_count: 0,
  schema_version: 1,
  created_at_ms: 1,
  updated_at_ms: 1,
};

const initialDocument: MaskedSourceDocument = {
  summary,
  masked_text: initialText,
  credential_slots: [],
};

const candidate: InstallCandidate = {
  id: 'candidate:one',
  document_ref: { document_id: summary.document_id, document_revision: 2 },
  transient: false,
  expected_installed_revision: 0,
  profile: {
    id: 'profile:one',
    title: '示例来源',
    icon_url: null,
    version: null,
    group: null,
    supported_intents: ['Search'],
    risk_notes: [],
  },
  required_grant: {
    network: false,
    system: { fs: false, env: false, process: false },
  },
  diagnostics: [],
  definition_hash: 'definition',
  plan_hash: 'plan',
  expires_at_ms: 9_999_999_999_999,
};

const mountedViews: Array<{ unmount(): void }> = [];

afterEach(() => {
  for (const view of mountedViews.splice(0)) view.unmount();
  vi.restoreAllMocks();
});

function createSession() {
  let currentDocument = initialDocument;
  const saveDocument = vi.fn<RuleEditorSessionApi['saveDocument']>(async (request) => {
    currentDocument = {
      summary: {
        ...summary,
        revision: 2,
        masked_hash: 'masked:two',
        updated_at_ms: 2,
      },
      masked_text: request.masked_text,
      credential_slots: [],
    };
    return { status: 'saved', document: currentDocument };
  });
  const prepareInstall = vi.fn<RuleEditorSessionApi['prepareInstall']>(async () => candidate);
  const api: Partial<RuleEditorSessionApi> = {
    listDocuments: vi.fn(async () => [summary]),
    getDocument: vi.fn(async () => currentDocument),
    pinRevision: vi.fn(async (request) => ({
      pin_id: `pin:${request.document_revision}`,
      ...request,
      expires_at_ms: 9_999_999_999_999,
    })),
    releaseRevisionPin: vi.fn(async ({ pin_id }) => ({ status: 'released' as const, pin_id })),
    saveDocument,
    prepareInstall,
  };
  return { session: new RuleEditorSession({ api }), saveDocument, prepareInstall };
}

function dispatchShortcut(
  target: Element,
  key: string,
  options: { metaKey?: boolean; shiftKey?: boolean } = {},
): KeyboardEvent {
  const event = new KeyboardEvent('keydown', {
    key,
    ctrlKey: !(options.metaKey ?? false),
    metaKey: options.metaKey ?? false,
    shiftKey: options.shiftKey ?? false,
    bubbles: true,
    cancelable: true,
    composed: true,
  });
  target.dispatchEvent(event);
  return event;
}

const AndroidRuleWorkspace: typeof RuleWorkspace = (...args) => {
  setPlatformContext({ platform: 'android' });
  return RuleWorkspace(...args);
};

const WindowsRuleWorkspace: typeof RuleWorkspace = (...args) => {
  setPlatformContext({ platform: 'windows' });
  return RuleWorkspace(...args);
};

describe('RuleWorkspace', () => {
  it('keeps typed edits and undo shared across form/tree before preparing the saved revision', async () => {
    const { session, saveDocument, prepareInstall } = createSession();
    const view = render(RuleWorkspace, { props: { session } });
    mountedViews.push(view);

    expect((await screen.findAllByText('示例文档')).length).toBeGreaterThan(0);
    await fireEvent.click(await screen.findByRole('tab', { name: '字段表单' }));

    const nameField = await screen.findByRole('textbox', { name: 'bookSourceName 的值' });
    await fireEvent.change(nameField, { target: { value: '修改后的来源' } });
    await waitFor(() => expect(session.snapshot.dirty).toBe(true));
    expect(await screen.findByText(/有未保存修改/)).toBeTruthy();

    await fireEvent.click(screen.getByRole('tab', { name: '规则树' }));
    await fireEvent.click(screen.getByRole('button', { name: '撤销' }));
    expect(await screen.findByText(/· 已保存$/)).toBeTruthy();

    await fireEvent.click(screen.getByRole('tab', { name: '字段表单' }));
    await fireEvent.change(screen.getByRole('textbox', { name: 'bookSourceName 的值' }), {
      target: { value: '最终来源' },
    });
    await fireEvent.click(screen.getByRole('button', { name: '保存' }));

    await waitFor(() => expect(saveDocument).toHaveBeenCalledTimes(1));
    expect(screen.getAllByText('修订 2').length).toBeGreaterThan(0);
    await fireEvent.click(screen.getByRole('button', { name: '准备安装' }));

    await waitFor(() => {
      expect(prepareInstall).toHaveBeenCalledWith({
        document_id: summary.document_id,
        document_revision: 2,
      });
    });
    expect(await screen.findByRole('heading', { name: '安装候选' })).toBeTruthy();
  });

  it('keeps revealed plaintext out of props and clears the visible DOM before holder release', async () => {
    const reveal = vi.fn(async () => undefined);
    const clearRevealed = vi.fn(async () => undefined);
    const readRevealed = vi.fn(() => 'short-lived-secret');
    const props = {
      slots: [
        {
          slot_id: 'slot:one',
          path: '/header/Authorization',
          name: 'Authorization',
          has_value: true,
        },
      ],
      revealedSlotIds: [] as readonly string[],
      onReveal: reveal,
      onClearRevealed: clearRevealed,
      readRevealed,
      onReplace: vi.fn(async () => undefined),
      onClear: vi.fn(async () => undefined),
    };
    const view = render(RuleCredentialPanel, { props });
    mountedViews.push(view);

    await fireEvent.click(screen.getByRole('button', { name: '显示 Authorization' }));
    expect(reveal).toHaveBeenCalledWith('slot:one');
    await view.rerender({ ...props, revealedSlotIds: ['slot:one'] });

    const revealed = screen.getByRole('textbox', { name: 'Authorization' }) as HTMLInputElement;
    expect(revealed.value).toBe('short-lived-secret');
    await fireEvent.click(screen.getByRole('button', { name: '隐藏 Authorization' }));

    expect(revealed.value).toBe('');
    expect(clearRevealed).toHaveBeenCalledTimes(1);
  });

  it('wipes and closes credential dialogs when busy or slot ownership changes', async () => {
    const props = {
      slots: [
        {
          slot_id: 'slot:one',
          path: '/header/Authorization',
          name: 'Authorization',
          has_value: true,
        },
      ],
      revealedSlotIds: [] as readonly string[],
      onReveal: vi.fn(async () => undefined),
      onClearRevealed: vi.fn(async () => undefined),
      readRevealed: vi.fn(() => null),
      onReplace: vi.fn(async () => undefined),
      onClear: vi.fn(async () => undefined),
    };
    const view = render(RuleCredentialPanel, { props });
    mountedViews.push(view);

    await fireEvent.click(screen.getByRole('button', { name: '替换 Authorization' }));
    const replaceDialog = await screen.findByRole('dialog', { name: '替换凭证' });
    const replacement = replaceDialog.querySelector<HTMLInputElement>(
      '#rule-credential-replacement',
    )!;
    expect(replacement).toBeTruthy();
    expect(replacement.labels?.[0]?.textContent).toContain('新凭证值');
    await fireEvent.input(replacement, { target: { value: 'dom-only-secret' } });
    expect(replacement.value).toBe('dom-only-secret');

    await view.rerender({ ...props, busy: true });
    await waitFor(() => expect(screen.queryByRole('dialog', { name: '替换凭证' })).toBeNull());
    expect(replacement.value).toBe('');
    await view.rerender({ ...props, busy: false });
    expect(screen.queryByRole('dialog', { name: '替换凭证' })).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: '替换 Authorization' }));
    const ownerReplaceDialog = await screen.findByRole('dialog', { name: '替换凭证' });
    const ownerReplacement = ownerReplaceDialog.querySelector<HTMLInputElement>(
      '#rule-credential-replacement',
    )!;
    expect(ownerReplacement).toBeTruthy();
    expect(ownerReplacement.labels?.[0]?.textContent).toContain('新凭证值');
    await fireEvent.input(ownerReplacement, { target: { value: 'old-owner-secret' } });
    const changedOwnerProps = {
      ...props,
      slots: [
        {
          slot_id: 'slot:two',
          path: '/loginUrl',
          name: 'loginUrl',
          has_value: true,
        },
      ],
    };
    await view.rerender(changedOwnerProps);
    await waitFor(() => expect(screen.queryByRole('dialog', { name: '替换凭证' })).toBeNull());
    expect(ownerReplacement.value).toBe('');

    await fireEvent.click(screen.getByRole('button', { name: '清除 loginUrl' }));
    expect(await screen.findByRole('dialog', { name: '清除凭证？' })).toBeTruthy();
    await view.rerender({ ...changedOwnerProps, busy: true });
    await waitFor(() => expect(screen.queryByRole('dialog', { name: '清除凭证？' })).toBeNull());
  });

  it('wipes the replacement DOM value during component cleanup', async () => {
    const view = render(RuleCredentialPanel, {
      props: {
        slots: [
          {
            slot_id: 'slot:one',
            path: '/header/Authorization',
            name: 'Authorization',
            has_value: true,
          },
        ],
        revealedSlotIds: [],
        onReveal: vi.fn(async () => undefined),
        onClearRevealed: vi.fn(async () => undefined),
        readRevealed: vi.fn(() => null),
        onReplace: vi.fn(async () => undefined),
        onClear: vi.fn(async () => undefined),
      },
    });
    mountedViews.push(view);
    await fireEvent.click(screen.getByRole('button', { name: '替换 Authorization' }));
    const replaceDialog = await screen.findByRole('dialog', { name: '替换凭证' });
    const replacement = replaceDialog.querySelector<HTMLInputElement>(
      '#rule-credential-replacement',
    )!;
    expect(replacement).toBeTruthy();
    expect(replacement.labels?.[0]?.textContent).toContain('新凭证值');
    await fireEvent.input(replacement, { target: { value: 'cleanup-secret' } });

    view.unmount();
    mountedViews.splice(mountedViews.indexOf(view), 1);
    expect(replacement.value).toBe('');
  });

  it('exposes masked conflict diff with reload, fork, manual merge, and continue paths', async () => {
    const conflict: RuleEditorConflictSnapshot = {
      kind: 'revision',
      reload_required: false,
      base_revision: 1,
      current_revision: 2,
      current: {
        summary: { ...summary, revision: 2, masked_hash: 'masked:current' },
        masked_text: initialText.replace('示例来源', '当前来源'),
        credential_slots: [],
      },
      dismissed: false,
      credentials: [],
    };
    const reload = vi.fn(async () => undefined);
    const fork = vi.fn(async () => true);
    const merge = vi.fn(async () => true);
    const continueEditing = vi.fn();
    const editText = vi.fn();
    const view = render(RuleConflictPanel, {
      props: {
        conflict,
        localMaskedText: initialText.replace('示例来源', '本地来源'),
        onTextEdit: editText,
        onSetResolution: vi.fn(),
        onReload: reload,
        onFork: fork,
        onMerge: merge,
        onContinue: continueEditing,
      },
    });
    mountedViews.push(view);

    expect(screen.getByText(/本地来源/)).toBeTruthy();
    expect(screen.getByText(/当前来源/)).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: '重新加载当前修订' }));
    expect(reload).toHaveBeenCalledOnce();
    await fireEvent.click(screen.getByRole('button', { name: '继续编辑' }));
    expect(continueEditing).toHaveBeenCalledOnce();

    await fireEvent.click(screen.getByRole('button', { name: '另存为草稿' }));
    await fireEvent.input(screen.getByRole('textbox', { name: '新草稿标题' }), {
      target: { value: '冲突草稿' },
    });
    await fireEvent.click(screen.getByRole('button', { name: '创建草稿' }));
    expect(fork).toHaveBeenCalledWith('冲突草稿');

    await fireEvent.click(screen.getByRole('button', { name: '手动合并' }));
    await fireEvent.input(screen.getByRole('textbox', { name: '合并后的遮罩文本' }), {
      target: { value: initialText.replace('示例来源', '合并来源') },
    });
    expect(editText).toHaveBeenCalled();
    await fireEvent.click(screen.getByRole('button', { name: '合并并保存' }));
    expect(merge).toHaveBeenCalledOnce();
  });

  it('restricts sentinel rotation conflicts to the safe reload action', async () => {
    const reload = vi.fn(async () => undefined);
    const conflict: RuleEditorConflictSnapshot = {
      kind: 'sentinel_rotation',
      reload_required: true,
      base_revision: 1,
      current_revision: 2,
      current: {
        summary: { ...summary, revision: 2, masked_hash: 'masked:rotated' },
        masked_text: initialText,
        credential_slots: [],
      },
      dismissed: false,
      credentials: [],
    };
    const view = render(RuleConflictPanel, {
      props: {
        conflict,
        localMaskedText: initialText,
        onTextEdit: vi.fn(),
        onSetResolution: vi.fn(),
        onReload: reload,
        onFork: vi.fn(async () => false),
        onMerge: vi.fn(async () => false),
        onContinue: vi.fn(),
      },
    });
    mountedViews.push(view);

    expect(screen.getByRole('heading', { name: '凭证令牌已更新' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: '另存为草稿' })).toBeNull();
    expect(screen.queryByRole('button', { name: '手动合并' })).toBeNull();
    expect(screen.queryByRole('button', { name: '继续编辑' })).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: '重新加载已保存修订' }));
    expect(reload).toHaveBeenCalledOnce();
  });

  it('keeps unsuccessful fork and rejected merge dialogs open with their entered values', async () => {
    const conflict: RuleEditorConflictSnapshot = {
      kind: 'revision',
      reload_required: false,
      base_revision: 1,
      current_revision: 2,
      current: {
        summary: { ...summary, revision: 2, masked_hash: 'masked:current' },
        masked_text: initialText.replace('示例来源', '当前来源'),
        credential_slots: [],
      },
      dismissed: false,
      credentials: [],
    };
    const fork = vi.fn(async () => false);
    const merge = vi.fn(async (): Promise<boolean> => {
      throw new Error('document_pin_failed');
    });
    const editText = vi.fn();
    const props = {
      conflict,
      localMaskedText: initialText,
      onTextEdit: editText,
      onSetResolution: vi.fn(),
      onReload: vi.fn(async () => undefined),
      onFork: fork,
      onMerge: merge,
      onContinue: vi.fn(),
    };
    const view = render(RuleConflictPanel, { props });
    mountedViews.push(view);

    await fireEvent.click(screen.getByRole('button', { name: '另存为草稿' }));
    const forkTitle = screen.getByRole('textbox', { name: '新草稿标题' }) as HTMLInputElement;
    await fireEvent.input(forkTitle, { target: { value: '保留的冲突草稿' } });
    await fireEvent.click(screen.getByRole('button', { name: '创建草稿' }));

    expect(fork).toHaveBeenCalledWith('保留的冲突草稿');
    expect(screen.getByRole('dialog', { name: '将冲突另存为新草稿' })).toBeTruthy();
    expect(forkTitle.value).toBe('保留的冲突草稿');
    expect(screen.getByRole('alert').textContent).toContain('请检查当前修订和凭证处理方式后重试');

    await fireEvent.click(screen.getByRole('button', { name: '取消' }));
    await fireEvent.click(screen.getByRole('button', { name: '手动合并' }));
    const mergedText = initialText.replace('示例来源', '保留的合并来源');
    const mergeInput = screen.getByRole('textbox', { name: '合并后的遮罩文本' });
    await fireEvent.input(mergeInput, { target: { value: mergedText } });
    await view.rerender({ ...props, localMaskedText: mergedText });
    await fireEvent.click(screen.getByRole('button', { name: '合并并保存' }));

    expect(merge).toHaveBeenCalledOnce();
    expect(screen.getByRole('dialog', { name: '手动合并遮罩文本' })).toBeTruthy();
    expect(
      (screen.getByRole('textbox', { name: '合并后的遮罩文本' }) as HTMLTextAreaElement).value,
    ).toBe(mergedText);
    expect(screen.getByRole('alert').textContent).toContain('请检查当前修订和凭证处理方式后重试');
  });

  it('disables credential resolutions that have no source value', async () => {
    const setResolution = vi.fn();
    const conflict: RuleEditorConflictSnapshot = {
      kind: 'revision',
      reload_required: false,
      base_revision: 1,
      current_revision: 2,
      current: {
        summary: { ...summary, revision: 2, masked_hash: 'masked:credentials' },
        masked_text: initialText,
        credential_slots: [],
      },
      dismissed: false,
      credentials: [
        { path: '/local-only', base_present: true, current_present: false, resolution: null },
        { path: '/current-only', base_present: false, current_present: true, resolution: null },
      ],
    };
    const view = render(RuleConflictPanel, {
      props: {
        conflict,
        localMaskedText: initialText,
        onTextEdit: vi.fn(),
        onSetResolution: setResolution,
        onReload: vi.fn(async () => undefined),
        onFork: vi.fn(async () => false),
        onMerge: vi.fn(async () => false),
        onContinue: vi.fn(),
      },
    });
    mountedViews.push(view);

    const localTrigger = screen.getByRole('button', { name: '/local-only 的解决方式' });
    localTrigger.focus();
    await fireEvent.keyDown(localTrigger, { key: 'ArrowDown' });
    const unavailableCurrent = await screen.findByRole('option', { name: '保留当前值' });
    expect(unavailableCurrent.matches('[data-disabled], [aria-disabled="true"]')).toBe(true);
    expect(setResolution).not.toHaveBeenCalled();
    await fireEvent.keyDown(document.activeElement ?? localTrigger, { key: 'Enter' });
    expect(setResolution).toHaveBeenLastCalledWith('/local-only', { kind: 'keep_local' });

    const currentTrigger = screen.getByRole('button', { name: '/current-only 的解决方式' });
    currentTrigger.focus();
    await fireEvent.keyDown(currentTrigger, { key: 'ArrowDown' });
    const unavailableLocal = await screen.findByRole('option', { name: '保留你的值' });
    expect(unavailableLocal.matches('[data-disabled], [aria-disabled="true"]')).toBe(true);
    expect(setResolution).toHaveBeenCalledTimes(1);
    await fireEvent.keyDown(document.activeElement ?? currentTrigger, { key: 'Enter' });
    expect(setResolution).toHaveBeenLastCalledWith('/current-only', { kind: 'keep_current' });
  });

  it('uses generic reload-only copy when the safe base snapshot is unavailable', async () => {
    const reload = vi.fn(async () => undefined);
    const conflict: RuleEditorConflictSnapshot = {
      kind: 'snapshot_unavailable',
      reload_required: true,
      base_revision: 1,
      current_revision: 2,
      current: {
        summary: { ...summary, revision: 2, masked_hash: 'masked:current' },
        masked_text: initialText,
        credential_slots: [],
      },
      dismissed: false,
      credentials: [],
    };
    const view = render(RuleConflictPanel, {
      props: {
        conflict,
        localMaskedText: initialText,
        onTextEdit: vi.fn(),
        onSetResolution: vi.fn(),
        onReload: reload,
        onFork: vi.fn(async () => false),
        onMerge: vi.fn(async () => false),
        onContinue: vi.fn(),
      },
    });
    mountedViews.push(view);

    expect(screen.getByRole('heading', { name: '历史快照不可用' })).toBeTruthy();
    expect(screen.queryByText('凭证令牌已更新')).toBeNull();
    expect(screen.queryByRole('button', { name: '另存为草稿' })).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: '重新加载当前已保存修订' }));
    expect(reload).toHaveBeenCalledOnce();
  });

  it('releases editable and modal shortcuts while scoping undo and redo to the editor surface', async () => {
    const { session, saveDocument } = createSession();
    const undo = vi.spyOn(session, 'undo').mockReturnValue(true);
    const redo = vi.spyOn(session, 'redo').mockReturnValue(true);
    const view = render(RuleWorkspace, { props: { session } });
    mountedViews.push(view);

    expect((await screen.findAllByText('示例文档')).length).toBeGreaterThan(0);
    await waitFor(() =>
      expect(session.snapshot.document?.summary.document_id).toBe(summary.document_id),
    );
    await fireEvent.click((await screen.findAllByRole('button', { name: '新建文档' }))[0]!);
    const modalCancel = await screen.findByRole('button', { name: '取消' });
    expect(dispatchShortcut(modalCancel, 'z').defaultPrevented).toBe(false);
    expect(undo).not.toHaveBeenCalled();
    await fireEvent.click(modalCancel);

    session.editText(initialText.replace('示例来源', '快捷键来源'));
    await waitFor(() => expect(session.snapshot.dirty).toBe(true));
    const surface = await waitFor(() => {
      const element = document.querySelector<HTMLElement>('[data-rule-editor-surface]');
      expect(element).not.toBeNull();
      return element!;
    });

    const input = document.createElement('input');
    const textarea = document.createElement('textarea');
    const select = document.createElement('select');
    const editable = document.createElement('div');
    editable.setAttribute('contenteditable', 'true');
    for (const target of [input, textarea, select, editable]) {
      surface.append(target);
      expect(dispatchShortcut(target, 'z').defaultPrevented).toBe(false);
      expect(dispatchShortcut(target, 'y').defaultPrevented).toBe(false);
      expect(dispatchShortcut(target, 's').defaultPrevented).toBe(false);
    }
    expect(undo).not.toHaveBeenCalled();
    expect(redo).not.toHaveBeenCalled();
    expect(saveDocument).not.toHaveBeenCalled();

    const saveButton = screen.getByRole('button', { name: '保存' });
    expect(dispatchShortcut(saveButton, 'z').defaultPrevented).toBe(false);
    expect(undo).not.toHaveBeenCalled();

    surface.tabIndex = -1;
    surface.focus();
    expect(dispatchShortcut(surface, 'z').defaultPrevented).toBe(true);
    expect(dispatchShortcut(surface, 'y').defaultPrevented).toBe(true);
    expect(undo).toHaveBeenCalledOnce();
    expect(redo).toHaveBeenCalledOnce();
    expect(dispatchShortcut(surface, 's', { metaKey: true }).defaultPrevented).toBe(true);
    await waitFor(() => expect(saveDocument).toHaveBeenCalledOnce());
  });

  it('shows the force-kill save warning only for native mobile platform context', async () => {
    const androidSession = createSession().session;
    const androidView = render(AndroidRuleWorkspace, { props: { session: androidSession } });
    expect((await screen.findAllByText('示例文档')).length).toBeGreaterThan(0);
    await waitFor(() =>
      expect(androidSession.snapshot.document?.summary.document_id).toBe(summary.document_id),
    );
    androidSession.editText(initialText.replace('示例来源', 'Android 来源'));
    expect(await screen.findByText(/操作系统强制结束应用无法被拦截/)).toBeTruthy();
    expect(document.querySelector('[data-native-mobile-save-risk="android"]')).toBeTruthy();
    androidView.unmount();

    const windowsSession = createSession().session;
    const windowsView = render(WindowsRuleWorkspace, { props: { session: windowsSession } });
    mountedViews.push(windowsView);
    expect((await screen.findAllByText('示例文档')).length).toBeGreaterThan(0);
    await waitFor(() =>
      expect(windowsSession.snapshot.document?.summary.document_id).toBe(summary.document_id),
    );
    windowsSession.editText(initialText.replace('示例来源', 'Windows 来源'));
    await waitFor(() => expect(windowsSession.snapshot.dirty).toBe(true));
    expect(screen.queryByText(/操作系统强制结束应用无法被拦截/)).toBeNull();
    expect(document.querySelector('[data-native-mobile-save-risk]')).toBeNull();
  });
});
