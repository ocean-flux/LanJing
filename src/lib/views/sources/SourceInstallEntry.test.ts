import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { InstallCandidate } from '$lib/stores/rules.svelte';
import SourceInstallEntry from './SourceInstallEntry.svelte';

const store = vi.hoisted(() => ({
  prepareInstall: vi.fn(),
  prepareMaccmsInstall: vi.fn(),
  installCandidate: vi.fn(),
}));

vi.mock('$lib/stores/rules.svelte', () => ({
  prepareInstall: store.prepareInstall,
  prepareMaccmsInstall: store.prepareMaccmsInstall,
  installCandidate: store.installCandidate,
}));

const legadoCandidate: InstallCandidate = {
  id: 'candidate:legado',
  profile: {
    id: 'profile:legado',
    title: 'Legado Source',
    icon_url: null,
    version: '1.0.0',
    supported_intents: ['Search'],
    risk_notes: [],
  },
  required_grant: {
    network: false,
    system: { fs: false, env: false, process: false },
  },
  diagnostics: [{ code: 'INFO', message: 'ready' }],
  definition_hash: 'definition-hash-abcdef012345',
  plan_hash: 'plan-hash-abcdef012345',
  expires_at_ms: Date.now() + 60_000,
};

const maccmsCandidate: InstallCandidate = {
  ...legadoCandidate,
  id: 'candidate:maccms',
  profile: {
    ...legadoCandidate.profile,
    id: 'profile:maccms',
    title: 'Maccms Source',
  },
};

async function typeLegadoJson(value: string): Promise<HTMLElement> {
  const textarea = screen.getByTestId('json-highlight-input');
  await fireEvent.input(textarea, { target: { value } });
  return textarea;
}

describe('SourceInstallEntry format segment', () => {
  beforeEach(() => {
    store.prepareInstall.mockReset();
    store.prepareMaccmsInstall.mockReset();
    store.installCandidate.mockReset();
  });

  it('clears legado candidate when switching to maccms', async () => {
    store.prepareInstall.mockResolvedValue(legadoCandidate);

    render(SourceInstallEntry);

    await typeLegadoJson('{"bookSourceUrl":"https://example.test"}');

    await fireEvent.click(screen.getByTestId('install-legado-prepare'));

    await waitFor(() => {
      expect(screen.getByText('Legado Source')).toBeTruthy();
    });

    await fireEvent.click(screen.getByTestId('install-format-maccms'));

    await waitFor(() => {
      expect(screen.queryByText('Legado Source')).toBeNull();
      expect(screen.getByTestId('install-maccms-url')).toBeTruthy();
    });
  });

  it('prepares maccms install via prepareMaccmsInstall', async () => {
    store.prepareMaccmsInstall.mockResolvedValue(maccmsCandidate);

    render(SourceInstallEntry);

    await fireEvent.click(screen.getByTestId('install-format-maccms'));

    const urlInput = screen.getByTestId('install-maccms-url');
    await fireEvent.input(urlInput, {
      target: { value: 'https://api.example.test/provide/vod' },
    });

    await fireEvent.click(screen.getByTestId('install-maccms-prepare'));

    await waitFor(() => {
      expect(store.prepareMaccmsInstall).toHaveBeenCalledWith(
        'https://api.example.test/provide/vod',
      );
      expect(screen.getByText('Maccms Source')).toBeTruthy();
    });
    expect(store.prepareInstall).not.toHaveBeenCalled();
  });

  it('rejects array JSON without calling prepareInstall', async () => {
    render(SourceInstallEntry);

    await typeLegadoJson('[{"bookSourceName":"A"},{"bookSourceName":"B"}]');
    await fireEvent.click(screen.getByTestId('install-legado-prepare'));

    await waitFor(() => {
      expect(screen.getByTestId('install-error').textContent).toMatch(
        /deep-link|深链|Multi-source|多书源/i,
      );
    });
    expect(store.prepareInstall).not.toHaveBeenCalled();
  });

  it('prepares object JSON and shows thickened candidate preview', async () => {
    store.prepareInstall.mockResolvedValue(legadoCandidate);

    render(SourceInstallEntry);

    await typeLegadoJson('{"bookSourceUrl":"https://example.test"}');
    await fireEvent.click(screen.getByTestId('install-legado-prepare'));

    await waitFor(() => {
      expect(store.prepareInstall).toHaveBeenCalledWith('{"bookSourceUrl":"https://example.test"}');
      expect(screen.getByTestId('install-candidate-preview')).toBeTruthy();
      expect(screen.getByTestId('install-validated-hint')).toBeTruthy();
      expect(screen.getByTestId('install-definition-hash')).toBeTruthy();
      expect(screen.getByTestId('install-supported-intents')).toBeTruthy();
      expect(screen.getByTestId('install-diagnostics')).toBeTruthy();
    });
  });
});
