import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import AddSourcePanel from './AddSourcePanel.svelte';

describe('AddSourcePanel', () => {
  it('offers five source entry types and updates pre-check placeholder', async () => {
    render(AddSourcePanel);

    for (const label of ['单个 URL', '订阅链接', '本地规则包', '本地目录', '单个文件']) {
      expect(screen.getByRole('button', { name: new RegExp(label) })).toBeTruthy();
    }

    await fireEvent.click(screen.getByRole('button', { name: /本地目录/ }));

    expect(screen.getByText('本地目录 预检查')).toBeTruthy();
    expect(screen.getByText('将扫描目录并按媒体类型建立本地资源入口。')).toBeTruthy();
  });
});
