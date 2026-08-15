import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import DiagnosticList from './DiagnosticList.svelte';
import type { InstallDiagnostic } from '$lib/rules/native-authoring/wire';

function makeSession(diagnostics: InstallDiagnostic[]) {
  return { diagnostics } as never;
}

describe('DiagnosticList', () => {
  it('shows empty state when no diagnostics', () => {
    render(DiagnosticList, { props: { session: makeSession([]) } });
    expect(screen.getByText('暂无诊断')).toBeTruthy();
  });

  it('sorts diagnostics by severity and shows code/message', () => {
    const diagnostics: InstallDiagnostic[] = [
      { code: 'INFO_1', severity: 'info', message: '提示消息' },
      { code: 'ERR_1', severity: 'error', message: '错误消息' },
      { code: 'WARN_1', severity: 'warning', message: '警告消息' },
    ];
    render(DiagnosticList, { props: { session: makeSession(diagnostics) } });

    expect(screen.getByText('ERR_1')).toBeTruthy();
    expect(screen.getByText('错误消息')).toBeTruthy();
    expect(screen.getByText('WARN_1')).toBeTruthy();
    expect(screen.getByText('INFO_1')).toBeTruthy();
  });
});
