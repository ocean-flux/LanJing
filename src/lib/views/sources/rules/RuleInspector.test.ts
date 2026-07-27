import { render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';
import { setLocale } from '$lib/i18n';
import {
  RuleEditorSession,
  type RuleEditorSessionSnapshot,
} from '$lib/stores/rule-editor-session.svelte';
import RuleInspector from './RuleInspector.svelte';

const mountedViews: Array<{ unmount(): void }> = [];
const sessions: RuleEditorSession[] = [];

afterEach(async () => {
  for (const view of mountedViews.splice(0)) view.unmount();
  for (const session of sessions.splice(0)) await session.dispose();
  await setLocale('zh-CN', { reload: false });
});

describe('RuleInspector diagnostics', () => {
  it('uses the same English code mapping and fallback without rendering backend messages', async () => {
    await setLocale('en', { reload: false });
    const session = new RuleEditorSession();
    sessions.push(session);
    const backendMessage = '后端中文：此文案不得进入界面';
    const snapshot: RuleEditorSessionSnapshot = {
      ...session.snapshot,
      diagnostics: [
        {
          severity: 'warning',
          code: 'unknown_field',
          path: '/custom',
          byte_offset: 4,
          byte_length: 8,
          support: 'unknown',
        },
        {
          severity: 'error',
          code: 'invalid_json',
          path: '',
          byte_offset: 0,
          byte_length: 1,
          support: 'blocked',
        },
      ],
      prepare_diagnostics: [
        {
          code: 'future_backend_code',
          severity: 'warning',
          message: backendMessage,
          span: { start: 20, end: 28, path: '/prepared' },
        },
      ],
    };

    const view = render(RuleInspector, { props: { session, snapshot } });
    mountedViews.push(view);

    expect(
      screen.getByText('This unrecognized field is preserved but is not executable.'),
    ).toBeTruthy();
    expect(screen.getByText('An unrecognized diagnostic was reported.')).toBeTruthy();
    expect(screen.getByText('unknown_field')).toBeTruthy();
    expect(screen.getByText('future_backend_code')).toBeTruthy();
    expect(screen.getByText('The source text is not valid JSON.')).toBeTruthy();
    const rootDiagnostic = screen.getByText('invalid_json').closest('li');
    expect(rootDiagnostic?.getAttribute('data-diagnostic-path')).toBe('');
    expect(screen.getByText('Path /')).toBeTruthy();
    expect(screen.getByText('Span 0–1')).toBeTruthy();
    const authoringDiagnostic = screen.getByText('unknown_field').closest('li');
    expect(authoringDiagnostic?.getAttribute('data-diagnostic-code')).toBe('unknown_field');
    expect(authoringDiagnostic?.getAttribute('data-diagnostic-path')).toBe('/custom');
    expect(authoringDiagnostic?.getAttribute('data-diagnostic-span-start')).toBe('4');
    expect(authoringDiagnostic?.getAttribute('data-diagnostic-span-end')).toBe('12');
    expect(screen.getByText('Path /custom')).toBeTruthy();
    expect(screen.getByText('Span 4–12')).toBeTruthy();
    expect(screen.getByText('Path /prepared')).toBeTruthy();
    expect(screen.getByText('Span 20–28')).toBeTruthy();
    expect(screen.queryByText(backendMessage)).toBeNull();
  });
});
