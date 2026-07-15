import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import DebugTools from './DebugTools.svelte';

describe('DebugTools', () => {
  it('keeps import preview and execution witness reachable', async () => {
    render(DebugTools);

    expect(screen.getByRole('heading', { name: '调试工具' })).toBeTruthy();
    expect(screen.getByRole('tab', { name: '导入规则' })).toBeTruthy();

    await fireEvent.click(screen.getByRole('tab', { name: '执行 Witness' }));

    expect(screen.getByRole('heading', { name: '规则执行 Witness' })).toBeTruthy();
  });
});
