import { render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { setLocale } from '$lib/i18n';
import RootError from './+error.svelte';

vi.mock('$app/state', () => ({
  page: {
    status: 503,
    error: new Error('internal detail must stay hidden'),
  },
}));

afterEach(async () => {
  await setLocale('zh-CN', { reload: false });
});

describe('root error page', () => {
  it('renders a localized safe recovery surface without exposing raw errors', async () => {
    await setLocale('en', { reload: false });
    render(RootError);

    expect(screen.getByRole('heading', { level: 1, name: 'Page unavailable' })).toBeTruthy();
    expect(screen.getByText('Error 503')).toBeTruthy();
    expect(screen.getByRole('link', { name: 'Return to Realm' }).getAttribute('href')).toBe('/');
    expect(screen.queryByText('internal detail must stay hidden')).toBeNull();
  });
});
