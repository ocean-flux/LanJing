import { render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import TemplateWizard from './TemplateWizard.svelte';

const sessionMock = vi.hoisted(() => ({
  createTemplate: vi.fn(async () => ({})),
  createBlank: vi.fn(async () => ({})),
}));

vi.mock('$lib/rules/native-authoring/session.svelte', () => ({
  NativeRuleEditorSession: class {},
}));

vi.mock('$app/state', () => ({ page: { url: new URL('https://lanjing.test/rules') } }));

describe('TemplateWizard', () => {
  const oncancel = vi.fn();
  const oncreated = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders wizard fields', () => {
    render(TemplateWizard, {
      props: { session: sessionMock as never, oncancel, oncreated },
    });
    const titleInput = screen.getByLabelText(/规则名称/);
    expect(titleInput).toBeTruthy();
    expect(titleInput.getAttribute('data-slot')).toBe('input');
  });

  it('disables create button when required fields empty', () => {
    render(TemplateWizard, {
      props: { session: sessionMock as never, oncancel, oncreated },
    });
    const createBtn = screen.getByRole('button', { name: /创建/ });
    expect(createBtn.hasAttribute('disabled')).toBe(true);
  });

  it('renders cancel button', () => {
    render(TemplateWizard, {
      props: { session: sessionMock as never, oncancel, oncreated },
    });
    const cancelBtn = screen.getByRole('button', { name: /取消/ });
    expect(cancelBtn).toBeTruthy();
  });
});
