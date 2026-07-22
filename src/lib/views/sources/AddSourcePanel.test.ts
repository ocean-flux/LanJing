import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import AddSourcePanel from './AddSourcePanel.svelte';

describe('AddSourcePanel', () => {
  it('offers five named radio entries without a dead import action', async () => {
    render(AddSourcePanel);

    const panel = screen.getByTestId('add-source-panel');
    expect(panel.className).toContain('double-bezel');

    const group = screen.getByRole('radiogroup', { name: '来源入口类型' });
    const radios = screen.getAllByRole('radio');
    expect(radios).toHaveLength(5);
    expect(screen.queryByRole('button', { name: '导入本地文件' })).toBeNull();
    expect(
      Array.from(radios).filter((radio) => radio.getAttribute('tabindex') === '0'),
    ).toHaveLength(1);

    await fireEvent.keyDown(radios[0]!, { key: 'ArrowRight' });
    expect(document.activeElement).toBe(radios[1]);
    expect(screen.getByTestId('add-source-precheck').textContent).toContain('订阅链接 预检查');
    expect(group.querySelector('[aria-checked="true"]')).toBe(radios[1]);
  });

  it('offers the local import seam only when the owner supplies it', async () => {
    const onimportlocal = vi.fn();
    render(AddSourcePanel, { props: { onimportlocal } });

    await fireEvent.click(screen.getByRole('radio', { name: /单个文件/ }));
    await fireEvent.click(screen.getByRole('button', { name: '导入本地文件' }));

    expect(onimportlocal).toHaveBeenCalledOnce();
  });
});
