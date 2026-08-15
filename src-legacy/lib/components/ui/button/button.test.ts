import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import Button from './button.svelte';

describe('Button', () => {
  it('renders an empty href through the anchor branch', () => {
    const { container } = render(Button, { href: '', 'aria-label': 'Empty destination' });

    const link = container.querySelector('[data-slot="button"]');
    expect(link).toBeInstanceOf(HTMLAnchorElement);
    expect(link?.getAttribute('href')).toBe('');
  });

  it('owns disabled anchor semantics and suppresses click handling', () => {
    const onclick = vi.fn();
    const bubbled = vi.fn();
    render(Button, {
      href: '/library',
      disabled: true,
      onclick,
      'aria-label': 'Disabled destination',
      'aria-disabled': 'false',
      role: 'button',
      tabindex: 3,
    });

    const link = screen.getByRole('link', { name: 'Disabled destination' });
    document.body.addEventListener('click', bubbled);
    const event = new MouseEvent('click', { bubbles: true, cancelable: true });
    const dispatched = link.dispatchEvent(event);
    document.body.removeEventListener('click', bubbled);

    expect(link.getAttribute('href')).toBeNull();
    expect(link.getAttribute('aria-disabled')).toBe('true');
    expect(link.getAttribute('role')).toBe('link');
    expect(link.getAttribute('tabindex')).toBe('-1');
    expect(dispatched).toBe(false);
    expect(event.defaultPrevented).toBe(true);
    expect(onclick).not.toHaveBeenCalled();
    expect(bubbled).not.toHaveBeenCalled();
  });

  it('calls the managed click handler for enabled anchors', async () => {
    const onclick = vi.fn((event: MouseEvent) => event.preventDefault());
    render(Button, { href: '/library', onclick, 'aria-label': 'Open library' });

    await fireEvent.click(screen.getByRole('link', { name: 'Open library' }));

    expect(onclick).toHaveBeenCalledOnce();
  });
});
