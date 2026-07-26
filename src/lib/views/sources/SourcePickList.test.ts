import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import type { CatalogItem } from '$lib/deeplink/catalog';
import SourcePickList from './SourcePickList.svelte';

const items: CatalogItem[] = [
  {
    id: 'catalog:0',
    name: '甲源',
    group: '玄幻',
    rawJson: '{"bookSourceName":"甲源"}',
  },
  {
    id: 'catalog:1',
    name: '乙源',
    group: '玄幻',
    rawJson: '{"bookSourceName":"乙源"}',
  },
  {
    id: 'catalog:2',
    name: '丙源',
    group: null,
    rawJson: '{"bookSourceName":"丙源"}',
  },
];

describe('SourcePickList', () => {
  it('groups rows and supports group select-all', async () => {
    const onSelectedIdsChange = vi.fn();
    render(SourcePickList, {
      props: {
        items,
        selectedIds: [],
        onSelectedIdsChange,
      },
    });

    expect(screen.getByText('甲源')).toBeTruthy();
    expect(screen.getByText('乙源')).toBeTruthy();
    expect(screen.getByText('丙源')).toBeTruthy();
    expect(screen.getByText('玄幻')).toBeTruthy();

    const groupHeaders = screen.getAllByTestId('source-pick-group-header');
    const fantasyHeader = groupHeaders.find(
      (node) => node.getAttribute('data-group-id') === 'group:玄幻',
    );
    expect(fantasyHeader).toBeTruthy();
    const groupSelect = within(fantasyHeader as HTMLElement).getByTestId(
      'source-pick-group-select',
    );
    const checkbox = within(groupSelect).getByRole('checkbox');
    await fireEvent.click(checkbox);

    expect(onSelectedIdsChange).toHaveBeenCalled();
    const last = onSelectedIdsChange.mock.calls.at(-1)?.[0] as string[];
    expect(last.sort()).toEqual(['catalog:0', 'catalog:1']);
  });

  it('shows install cap notice when selection exceeds the limit', async () => {
    render(SourcePickList, {
      props: {
        items,
        selectedIds: ['catalog:0', 'catalog:1', 'catalog:2'],
        installCap: 2,
        truncated: true,
        totalCount: 99,
      },
    });

    expect(screen.getByTestId('source-pick-install-cap').textContent).toMatch(/3/);
    expect(screen.getByTestId('source-pick-display-cap').textContent).toMatch(/99/);
  });
});
