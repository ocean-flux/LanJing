import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import SelectTest from './select.test.svelte';

describe('Select', () => {
  it('portals the listbox and commits a keyboard selection', async () => {
    const { container } = render(SelectTest);
    const trigger = screen.getByRole('button', { name: 'Rule mode' });

    trigger.focus();
    await fireEvent.keyDown(trigger, { key: 'ArrowDown' });

    const listbox = await screen.findByRole('listbox');
    expect(container.contains(listbox)).toBe(false);
    expect(trigger.getAttribute('aria-expanded')).toBe('true');

    await fireEvent.keyDown(document.activeElement ?? listbox, { key: 'ArrowDown' });
    await fireEvent.keyDown(document.activeElement ?? listbox, { key: 'Enter' });

    expect(screen.getByRole('status', { name: 'Selected mode' }).textContent).toBe('form');
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
    expect(document.activeElement).toBe(trigger);
  });
});
