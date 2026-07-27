import { render, screen, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SourceAuthoringDocument } from '$lib/rules/authoring';
import RuleTreeEditor from './RuleTreeEditor.svelte';

const mountedViews: Array<{ unmount(): void }> = [];

afterEach(() => {
  for (const view of mountedViews.splice(0)) view.unmount();
});

describe('RuleTreeEditor', () => {
  it('exposes a native list while keeping every enabled editor control keyboard-focusable', () => {
    const authoringDocument = SourceAuthoringDocument.open(
      '{"bookSourceType":0,"bookSourceUrl":"https://example.com","bookSourceName":"Example"}',
    );
    const view = render(RuleTreeEditor, {
      props: { document: authoringDocument, onPatch: vi.fn() },
    });
    mountedViews.push(view);

    expect(screen.queryByRole('tree')).toBeNull();
    expect(screen.queryByRole('treeitem')).toBeNull();

    const list = screen.getByRole('list');
    expect(within(list).getAllByRole('listitem').length).toBeGreaterThan(1);

    const controls = [
      ...within(list).getAllByRole('button'),
      ...within(list).getAllByRole('textbox'),
      ...within(list).getAllByRole('spinbutton'),
    ].filter((control) => !(control as HTMLButtonElement | HTMLInputElement).disabled);
    expect(controls.length).toBeGreaterThan(0);

    for (const control of controls) {
      expect(control.tabIndex).toBe(0);
      control.focus();
      expect(globalThis.document.activeElement).toBe(control);
    }
  });
});
