import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setLocale } from '$lib/i18n';
import type { InstallCandidate } from '$lib/stores/rules.svelte';
import CandidatePreview from './CandidatePreview.svelte';
import { localizeImportDiagnostic } from './import-diagnostics';
import { classifyExpiresAt, truncateHash } from './candidate-preview';

const mountedViews: Array<{ unmount(): void }> = [];

const safetyCandidate: InstallCandidate = {
  id: 'candidate:safety',
  expected_installed_revision: 0,
  profile: {
    id: 'profile:safety',
    title: 'Safety source',
    icon_url: null,
    version: '4.2.0',
    group: 'Research',
    supported_intents: ['Search', 'ResolveItem'],
    risk_notes: ['Only contacts the configured host'],
  },
  required_grant: {
    network: true,
    system: { fs: false, env: false, process: false },
  },
  diagnostics: [
    {
      code: 'unknown_field',
      severity: 'warning',
      message: 'backend text must not render',
      span: { start: 2, end: 8, path: '/custom' },
    },
  ],
  definition_hash: 'definition-hash-0123456789',
  plan_hash: 'plan-hash-0123456789',
  expires_at_ms: Date.now() + 60_000,
};

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
    expect(localizeImportDiagnostic('unknown_field')).toBe('此未知字段在导入时会被忽略。');
    expect(localizeImportDiagnostic('future_backend_code')).toBe('收到无法识别的诊断。');
  });

  it('uses English stable-code copy and never renders Chinese backend messages', async () => {
    await setLocale('en', { reload: false });
    const backendKnownMessage = '后端中文：未知字段需要审阅';
    const backendUnknownMessage = '后端中文：未来诊断';
    const candidate: InstallCandidate = {
      id: 'candidate:diagnostics',
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

    expect(screen.getByText('This unrecognized field is ignored during import.')).toBeTruthy();
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
    expect(localizeImportDiagnostic('__proto__')).toBe('An unrecognized diagnostic was reported.');
  });
});

describe('CandidatePreview safety surface', () => {
  beforeEach(async () => {
    await setLocale('en', { reload: false });
  });

  it.each(['compact', 'full'] as const)(
    'keeps the complete safety summary in %s density without exposing internals',
    (density) => {
      const candidateWithInternals = {
        ...safetyCandidate,
        definition: 'INTERNAL_DEFINITION_VALUE',
        plan: 'INTERNAL_PLAN_VALUE',
        body: 'INTERNAL_BODY_VALUE',
        secret: 'INTERNAL_SECRET_VALUE',
      } as InstallCandidate;
      const view = render(CandidatePreview, {
        props: {
          candidate: candidateWithInternals,
          grant: 'none',
          density,
          onInstall: () => undefined,
        },
      });
      mountedViews.push(view);

      const preview = screen.getByTestId('install-candidate-preview');
      expect(preview.getAttribute('data-density')).toBe(density);
      expect(screen.getByRole('heading', { name: 'Install candidate' })).toBeTruthy();
      expect(screen.getByText('Safety source')).toBeTruthy();
      expect(screen.getByText('Research')).toBeTruthy();
      expect(screen.getByText('4.2.0')).toBeTruthy();
      expect(screen.getByText('Required')).toBeTruthy();
      expect(screen.getByText('Search')).toBeTruthy();
      expect(screen.getByText('Resolve item')).toBeTruthy();
      expect(screen.getByText('Only contacts the configured host')).toBeTruthy();
      expect(screen.getByText('unknown_field')).toBeTruthy();
      expect(screen.getByTestId('install-definition-hash').getAttribute('title')).toBe(
        safetyCandidate.definition_hash,
      );
      expect(screen.getByTestId('install-plan-hash').getAttribute('title')).toBe(
        safetyCandidate.plan_hash,
      );
      expect(preview.outerHTML).not.toContain('INTERNAL_DEFINITION_VALUE');
      expect(preview.outerHTML).not.toContain('INTERNAL_PLAN_VALUE');
      expect(preview.outerHTML).not.toContain('INTERNAL_BODY_VALUE');
      expect(preview.outerHTML).not.toContain('INTERNAL_SECRET_VALUE');
      expect(preview.outerHTML).not.toContain('backend text must not render');
    },
  );

  it('blocks unsupported system capabilities without offering a misleading grant', async () => {
    const onInstall = vi.fn();
    const blockedCandidate: InstallCandidate = {
      ...safetyCandidate,
      required_grant: {
        network: false,
        system: { fs: true, env: false, process: false },
      },
    };
    const view = render(CandidatePreview, {
      props: { candidate: blockedCandidate, grant: 'none', onInstall },
    });
    mountedViews.push(view);

    expect(
      screen.getByText(
        'This source requests system capabilities that this installer cannot grant.',
      ),
    ).toBeTruthy();
    expect(screen.queryByRole('combobox', { name: 'Network grant' })).toBeNull();
    const install = screen.getByRole('button', { name: 'Install source' });
    expect((install as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.click(install);
    expect(onInstall).not.toHaveBeenCalled();
  });
});
