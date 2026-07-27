import { render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';
import { setLocale } from '$lib/i18n';
import type { InstallCandidate } from '$lib/stores/rules.svelte';
import CandidatePreview from './CandidatePreview.svelte';
import { localizeSourceDiagnostic } from './source-diagnostics';
import { classifyExpiresAt, truncateHash } from './candidate-preview';

const mountedViews: Array<{ unmount(): void }> = [];

afterEach(async () => {
  for (const view of mountedViews.splice(0)) view.unmount();
  await setLocale('zh-CN', { reload: false });
});

describe('truncateHash', () => {
  it('returns short hashes unchanged', () => {
    expect(truncateHash('abcdef')).toBe('abcdef');
  });

  it('truncates long hashes as head…tail', () => {
    expect(truncateHash('0123456789abcdef0123')).toBe('01234567…0123');
  });
});

describe('classifyExpiresAt', () => {
  const now = 1_700_000_000_000;

  it('marks past timestamps as expired', () => {
    expect(classifyExpiresAt(now - 1, now)).toEqual({ kind: 'expired' });
  });

  it('uses minutes under one hour', () => {
    expect(classifyExpiresAt(now + 25 * 60_000, now)).toEqual({ kind: 'minutes', minutes: 25 });
  });

  it('uses hours under 48h', () => {
    expect(classifyExpiresAt(now + 5 * 3_600_000, now)).toEqual({ kind: 'hours', hours: 5 });
  });

  it('uses absolute text for longer windows', () => {
    const hint = classifyExpiresAt(now + 72 * 3_600_000, now);
    expect(hint.kind).toBe('absolute');
  });
});

describe('candidate diagnostics', () => {
  it('provides Chinese code copy and a localized fallback', async () => {
    await setLocale('zh-CN', { reload: false });
    expect(localizeSourceDiagnostic('unknown_field')).toBe('此未知字段会保留，但不可执行。');
    expect(localizeSourceDiagnostic('future_backend_code')).toBe('收到无法识别的诊断。');
  });

  it('uses English stable-code copy and never renders Chinese backend messages', async () => {
    await setLocale('en', { reload: false });
    const backendKnownMessage = '后端中文：未知字段需要审阅';
    const backendUnknownMessage = '后端中文：未来诊断';
    const candidate: InstallCandidate = {
      id: 'candidate:diagnostics',
      document_ref: null,
      transient: true,
      expected_installed_revision: 0,
      profile: {
        id: 'profile:diagnostics',
        title: 'Diagnostic source',
        icon_url: null,
        version: null,
        group: null,
        supported_intents: [],
        risk_notes: [],
      },
      required_grant: {
        network: false,
        system: { fs: false, env: false, process: false },
      },
      diagnostics: [
        {
          code: 'unknown_field',
          severity: 'warning',
          message: backendKnownMessage,
          span: { start: 4, end: 12, path: '/custom' },
        },
        {
          code: 'future_backend_code',
          severity: 'warning',
          message: backendUnknownMessage,
        },
      ],
      definition_hash: 'definition-hash',
      plan_hash: 'plan-hash',
      expires_at_ms: Date.now() + 60_000,
    };

    const view = render(CandidatePreview, {
      props: { candidate, grant: 'none', onInstall: () => undefined },
    });
    mountedViews.push(view);

    expect(
      screen.getByText('This unrecognized field is preserved but is not executable.'),
    ).toBeTruthy();
    expect(screen.getByText('An unrecognized diagnostic was reported.')).toBeTruthy();
    expect(screen.getByText('unknown_field')).toBeTruthy();
    expect(screen.getByText('future_backend_code')).toBeTruthy();
    const knownDiagnostic = screen.getByText('unknown_field').closest('li');
    expect(knownDiagnostic?.getAttribute('data-diagnostic-code')).toBe('unknown_field');
    expect(knownDiagnostic?.getAttribute('data-diagnostic-path')).toBe('/custom');
    expect(knownDiagnostic?.getAttribute('data-diagnostic-span-start')).toBe('4');
    expect(knownDiagnostic?.getAttribute('data-diagnostic-span-end')).toBe('12');
    expect(screen.getByText('Path /custom')).toBeTruthy();
    expect(screen.getByText('Span 4–12')).toBeTruthy();
    expect(screen.queryByText(backendKnownMessage)).toBeNull();
    expect(screen.queryByText(backendUnknownMessage)).toBeNull();
    expect(localizeSourceDiagnostic('__proto__')).toBe('An unrecognized diagnostic was reported.');
  });
});
