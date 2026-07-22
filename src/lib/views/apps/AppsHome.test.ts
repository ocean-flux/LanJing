import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import { mediaAppCards } from '$lib/app/demo-state';
import AppsHome from './AppsHome.svelte';

describe('AppsHome', () => {
  it('renders the full media app suite with next actions', () => {
    render(AppsHome);

    for (const app of mediaAppCards) {
      expect(screen.getByRole('heading', { name: app.label })).toBeTruthy();
      expect(screen.queryAllByText(app.statusLabel).length).toBeGreaterThan(0);
    }

    expect(screen.getByRole('link', { name: /进入小说/ }).getAttribute('href')).toBe('/apps/novel');
  });

  it('shows honest media-later copy for unconnected apps', async () => {
    render(AppsHome);

    await fireEvent.click(screen.getByRole('button', { name: /导入本地音乐/ }));

    const status = screen.getByRole('status');
    expect(status.textContent).toContain('媒体体验稍后');
    expect(status.textContent).toContain('音乐');
    expect(status.textContent).toContain('导入本地音乐');
  });

  it('uses the latest app props and closes stale placeholder panels on rerender', async () => {
    const view = render(AppsHome, { props: { apps: mediaAppCards } });

    await fireEvent.click(screen.getByRole('button', { name: /导入本地音乐/ }));

    const updatedApps = mediaAppCards.map((app) =>
      app.key === 'music' ? { ...app, label: '更新后的音乐', primaryAction: '继续导入音乐' } : app,
    );
    await view.rerender({ apps: updatedApps });

    const status = screen.getByRole('status');
    expect(status.textContent).toContain('更新后的音乐');
    expect(status.textContent).toContain('继续导入音乐');
    expect(status.textContent).not.toContain('导入本地音乐');

    await view.rerender({
      apps: updatedApps.map((app) => (app.key === 'music' ? { ...app, href: '/apps/music' } : app)),
    });
    expect(screen.queryByRole('status')).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: /导入本地视频/ }));
    await view.rerender({ apps: updatedApps.filter((app) => app.key !== 'video') });
    expect(screen.queryByRole('status')).toBeNull();
  });
});
