import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import TextReaderShell from './TextReaderShell.svelte';

describe('TextReaderShell', () => {
  it('uses an honest non-immersive reader placeholder', () => {
    render(TextReaderShell);

    expect(screen.getByRole('heading', { name: '阅读器尚未接入' })).toBeTruthy();
    expect(screen.getByText('媒体体验稍后')).toBeTruthy();
    expect(screen.getByText(/阅读器沉浸层尚未接入/)).toBeTruthy();
    expect(screen.getByRole('link', { name: '返回小说' }).getAttribute('href')).toBe('/apps/novel');
    expect(screen.getByRole('link', { name: '应用' }).getAttribute('href')).toBe('/apps');
    expect(screen.queryByRole('button', { name: '双页预览' })).toBeNull();
    expect(screen.queryByText(/阅读主题/)).toBeNull();
  });
});
