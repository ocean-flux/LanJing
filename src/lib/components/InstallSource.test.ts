import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { InstallCandidate, InstalledSource } from '$lib/stores/rules.svelte';
import InstallSource from './InstallSource.svelte';

const storeMocks = vi.hoisted(() => ({
  prepareInstall: vi.fn(),
  installCandidate: vi.fn(),
}));

vi.mock('$lib/stores/rules.svelte', () => storeMocks);

const sourceInput = '{"bookSourceName":"保留输入"}';

const candidate: InstallCandidate = {
  id: 'candidate:one',
  document_ref: null,
  transient: true,
  expected_installed_revision: 0,
  profile: {
    id: 'profile:one',
    title: '示例来源',
    icon_url: null,
    version: '2.3.1',
    group: null,
    supported_intents: ['Search'],
    risk_notes: ['仅访问目标站点'],
  },
  required_grant: {
    network: true,
    system: { fs: false, env: false, process: false },
  },
  diagnostics: [],
  definition_hash: 'definition-hash',
  plan_hash: 'plan-hash',
  expires_at_ms: 1_800_000_000_000,
};

const installedSource: InstalledSource = {
  source_id: 'source:one',
  document_ref: null,
  version: '2.3.1',
  profile: candidate.profile,
  revision: 1,
};

async function prepareCandidate(): Promise<HTMLTextAreaElement> {
  const input = screen.getByTestId('json-highlight-input') as HTMLTextAreaElement;
  await fireEvent.input(input, { target: { value: sourceInput } });
  await fireEvent.click(screen.getByTestId('install-legado-prepare'));
  await screen.findByTestId('install-candidate-preview');
  return input;
}

beforeEach(() => {
  storeMocks.prepareInstall.mockReset();
  storeMocks.installCandidate.mockReset();
});

describe('InstallSource', () => {
  it('prepares a preview and requires an explicit network-only grant before install', async () => {
    storeMocks.prepareInstall.mockResolvedValueOnce(candidate);
    storeMocks.installCandidate.mockResolvedValueOnce(installedSource);
    render(InstallSource);

    const input = await prepareCandidate();

    expect(storeMocks.prepareInstall).toHaveBeenCalledWith(sourceInput);
    expect(screen.getByText('2.3.1')).toBeTruthy();
    expect(screen.getByText('此来源需要访问网络。安装前请明确选择“仅授权网络访问”。')).toBeTruthy();

    const grant = screen.getByRole('combobox', { name: '网络授权' });
    const install = screen.getByRole('button', { name: '安装来源' });
    expect((grant as HTMLSelectElement).value).toBe('none');
    expect((install as HTMLButtonElement).disabled).toBe(true);

    await fireEvent.change(grant, { target: { value: 'network_only' } });
    expect((install as HTMLButtonElement).disabled).toBe(false);
    await fireEvent.click(install);

    await waitFor(() => {
      expect(storeMocks.installCandidate).toHaveBeenCalledWith('candidate:one', 'network_only');
      expect(screen.getByText(/已安装：source:one/)).toBeTruthy();
    });
    expect(input.value).toBe('');
    expect(screen.queryByTestId('install-candidate-preview')).toBeNull();
    expect(screen.queryByRole('combobox', { name: '网络授权' })).toBeNull();
  });

  it('retains the source input when prepare fails', async () => {
    storeMocks.prepareInstall.mockRejectedValueOnce(new Error('prepare failed'));
    render(InstallSource);

    const input = screen.getByTestId('json-highlight-input');
    await fireEvent.input(input, { target: { value: sourceInput } });
    await fireEvent.click(screen.getByTestId('install-legado-prepare'));

    expect((await screen.findByRole('alert')).textContent).toContain('Error: prepare failed');
    expect((input as HTMLTextAreaElement).value).toBe(sourceInput);
  });

  it('retains the source input and candidate when install fails', async () => {
    storeMocks.prepareInstall.mockResolvedValueOnce(candidate);
    storeMocks.installCandidate.mockRejectedValueOnce(new Error('install failed'));
    render(InstallSource);

    const input = await prepareCandidate();
    await fireEvent.change(screen.getByRole('combobox', { name: '网络授权' }), {
      target: { value: 'network_only' },
    });
    await fireEvent.click(screen.getByRole('button', { name: '安装来源' }));

    expect((await screen.findByRole('alert')).textContent).toContain('Error: install failed');
    expect(input.value).toBe(sourceInput);
    expect(screen.getByTestId('install-candidate-preview')).toBeTruthy();
  });
});
