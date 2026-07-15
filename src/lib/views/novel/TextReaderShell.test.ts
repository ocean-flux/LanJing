import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import TextReaderShell from './TextReaderShell.svelte';

describe('TextReaderShell', () => {
  it('uses text reader defaults independently from shell surfaces', () => {
    render(TextReaderShell);

    expect(screen.getByRole('heading', { name: '第一章 · 上林署' })).toBeTruthy();
    expect(screen.getByText(/阅读主题：paper · serif · 18px/)).toBeTruthy();
    expect(screen.getByText('第 1 / 2 页')).toBeTruthy();
    expect(screen.getByRole('link', { name: '返回小说' }).getAttribute('href')).toBe('/apps/novel');
    expect(screen.getByRole('button', { name: '双页预览' })).toBeTruthy();
  });
});
