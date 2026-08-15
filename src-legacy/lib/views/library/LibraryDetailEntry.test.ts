import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import LibraryDetailEntry from './LibraryDetailEntry.svelte';

describe('LibraryDetailEntry', () => {
  it('shows the stable resource_id for a minimal detail entry', () => {
    render(LibraryDetailEntry, { props: { resourceId: 'item:one' } });

    expect(screen.getByRole('heading', { level: 1, name: '资料详情' })).toBeTruthy();
    expect(screen.getByTestId('library-detail-resource-id').textContent).toBe('item:one');
    expect(screen.getByRole('link', { name: '返回资料库' }).getAttribute('href')).toBe('/library');
  });

  it('reports missing resource_id without inventing a demo item', () => {
    render(LibraryDetailEntry, { props: { resourceId: null } });

    expect(screen.getByTestId('library-detail-missing-id')).toBeTruthy();
    expect(screen.getByRole('alert').textContent).toContain('缺少 resource_id');
    expect(screen.getByRole('link', { name: '返回资料库' }).getAttribute('href')).toBe('/library');
  });
});
