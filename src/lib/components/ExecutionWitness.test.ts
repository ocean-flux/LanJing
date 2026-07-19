import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/stores/rules.svelte', () => ({
  getInstalledSources: () => [],
  loadInstalledSources: vi.fn(),
}));

vi.mock('$lib/stores/execution.svelte', () => ({
  startSearch: vi.fn(),
  startDiscover: vi.fn(),
  selectMediaItem: vi.fn(),
  selectMediaUnit: vi.fn(),
  goBack: vi.fn(),
  cleanup: vi.fn(),
  getMediaItems: () => [],
  getMediaUnits: () => [],
  getResolvedText: () => 'Normalized reading text',
  getLoading: () => false,
  getError: () => null,
  getCurrentStage: () => 'asset',
  getSelectedItem: () => null,
  getSelectedUnit: () => ({ title: 'Unit one' }),
}));

import ExecutionWitness from './ExecutionWitness.svelte';

describe('ExecutionWitness', () => {
  it('renders normalized text assets in the asset stage', () => {
    render(ExecutionWitness);

    expect(screen.getByText('Unit one')).toBeTruthy();
    expect(screen.getByText('Normalized reading text')).toBeTruthy();
  });
});
