import { fireEvent, render, screen } from '@testing-library/svelte';
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
  it('groups rows and changes only the selected group', async () => {
    const onSelectedIdsChange = vi.fn();
    render(SourcePickList, {
      props: {
        items,
        selectedIds: ['catalog:2'],
        onSelectedIdsChange,
      },
    });

    expect(screen.getByText('甲源')).toBeTruthy();
    expect(screen.getByText('乙源')).toBeTruthy();
    expect(screen.getByText('丙源')).toBeTruthy();
    expect(screen.getByRole('button', { name: '折叠分组：玄幻' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '折叠分组：未分组' })).toBeTruthy();

    const groupSelect = screen.getByRole('checkbox', { name: '全选分组 玄幻' });
    expect(groupSelect.getAttribute('aria-checked')).toBe('false');
    await fireEvent.click(groupSelect);

    const selectedAll = onSelectedIdsChange.mock.calls.at(-1)?.[0] as string[];
    expect([...selectedAll].sort()).toEqual(['catalog:0', 'catalog:1', 'catalog:2']);
    expect(groupSelect.getAttribute('aria-checked')).toBe('true');

    await fireEvent.click(groupSelect);
    const selectedOutsideGroup = onSelectedIdsChange.mock.calls.at(-1)?.[0] as string[];
    expect(selectedOutsideGroup).toEqual(['catalog:2']);
    expect(groupSelect.getAttribute('aria-checked')).toBe('false');
  });

  it('updates group names and row reachability when collapsed and expanded', async () => {
    render(SourcePickList, {
      props: { items, selectedIds: [] },
    });

    const collapse = screen.getByRole('button', { name: '折叠分组：玄幻' });
    expect(collapse.getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByRole('checkbox', { name: '甲源' })).toBeTruthy();

    await fireEvent.click(collapse);

    const expand = screen.getByRole('button', { name: '展开分组：玄幻' });
    expect(expand.getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByRole('checkbox', { name: '甲源' })).toBeNull();

    await fireEvent.click(expand);
    expect(screen.getByRole('checkbox', { name: '甲源' })).toBeTruthy();
  });

  it('reports display truncation and an over-cap selection without hiding choices', () => {
    render(SourcePickList, {
      props: {
        items,
        selectedIds: ['catalog:0', 'catalog:1', 'catalog:2'],
        installCap: 2,
        truncated: true,
        totalCount: 99,
      },
    });

    const installCap = screen.getByRole('status');
    expect(installCap.textContent).toMatch(/已选 3 条，单次最多安装 2 条/);
    expect(screen.getByTestId('source-pick-display-cap').textContent).toMatch(/3 \/ 99/);
    expect(screen.getByRole('checkbox', { name: '甲源' }).getAttribute('aria-checked')).toBe(
      'true',
    );
    expect(screen.getByRole('checkbox', { name: '丙源' }).getAttribute('aria-checked')).toBe(
      'true',
    );
  });
});
