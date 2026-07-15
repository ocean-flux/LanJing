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

  it('keeps placeholder apps meaningful without navigating to missing pages', async () => {
    render(AppsHome);

    await fireEvent.click(screen.getByRole('button', { name: /导入本地音乐/ }));

    const status = screen.getByRole('status');
    expect(status.textContent).toContain('音乐 尚未接入');
    expect(status.textContent).toContain('导入本地音乐');
  });
});
