import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import TabsTest from './tabs.test.svelte';

describe('Tabs', () => {
  it('uses automatic keyboard activation and links each tab to its panel', async () => {
    render(TabsTest);

    const original = screen.getByRole('tab', { name: 'Original' });
    const form = screen.getByRole('tab', { name: 'Form' });

    expect(original.getAttribute('aria-selected')).toBe('true');
    expect(screen.getByRole('tabpanel').textContent).toBe('Original panel');

    original.focus();
    await fireEvent.keyDown(original, { key: 'ArrowRight' });

    expect(document.activeElement).toBe(form);
    expect(form.getAttribute('aria-selected')).toBe('true');
    expect(screen.getByRole('tabpanel').textContent).toBe('Form panel');
  });
});
