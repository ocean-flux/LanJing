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
  diagnostics: [],
  definition_hash: 'def',
  plan_hash: 'plan',
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

describe('SourceInstallEntry format segment', () => {
  beforeEach(() => {
    store.prepareInstall.mockReset();
    store.prepareMaccmsInstall.mockReset();
    store.installCandidate.mockReset();
  });

  it('clears legado candidate when switching to maccms', async () => {
    store.prepareInstall.mockResolvedValue(legadoCandidate);

    render(SourceInstallEntry);

    const textarea = screen.getByPlaceholderText(/Paste Legado|粘贴 Legado/i);
    await fireEvent.input(textarea, {
      target: { value: '{"bookSourceUrl":"https://example.test"}' },
    });

    const prepareButtons = screen.getAllByRole('button', {
      name: /Prepare installation|准备安装/i,
    });
    await fireEvent.click(prepareButtons[0]!);

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
});
