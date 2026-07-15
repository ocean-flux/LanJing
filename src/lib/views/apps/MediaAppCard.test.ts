import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import { mediaAppCards } from '$lib/app/demo-state';
import MediaAppCard from './MediaAppCard.svelte';

describe('MediaAppCard', () => {
  it('renders novel as a real route link', () => {
    const novel = mediaAppCards.find((app) => app.key === 'novel');
    if (!novel) throw new Error('missing novel app');

    render(MediaAppCard, { props: { app: novel } });

    const link = screen.getByRole('link', { name: /进入小说/ });
    expect(link.getAttribute('href')).toBe('/apps/novel');
    expect(screen.getByRole('heading', { name: '小说' })).toBeTruthy();
  });

  it('uses placeholder action for unconnected apps', async () => {
    const music = mediaAppCards.find((app) => app.key === 'music');
    if (!music) throw new Error('missing music app');
    const onselect = vi.fn();

    render(MediaAppCard, { props: { app: music, onselect } });
    await fireEvent.click(screen.getByRole('button', { name: /导入本地音乐/ }));

    expect(onselect).toHaveBeenCalledWith(music);
  });
});
