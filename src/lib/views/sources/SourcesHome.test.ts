import { render, screen, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import { demoSources } from '$lib/app/demo-state';
import SourcesHome from './SourcesHome.svelte';

describe('SourcesHome', () => {
  it('shows add-source choices and local import action when no sources exist', () => {
    render(SourcesHome, { props: { sources: [] } });

    expect(screen.getByRole('heading', { name: '添加来源' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '导入本地文件' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: '还没有来源' })).toBeTruthy();
    expect(screen.getByRole('button', { name: /订阅链接/ })).toBeTruthy();
  });

  it('sorts source cards by attention and exposes trust facts plus actions', () => {
    render(SourcesHome, { props: { sources: demoSources } });

    const cards = screen.getAllByRole('article');
    expect(within(cards[0]).getByText('需要检查的音乐源')).toBeTruthy();
    expect(within(cards[0]).getByText('需要处理')).toBeTruthy();
    expect(within(cards[0]).getByRole('button', { name: '重试' })).toBeTruthy();
    expect(within(cards[0]).getByRole('button', { name: '禁用' })).toBeTruthy();

    for (const fact of ['来源', '网络访问', '远程解析', '失败隔离']) {
      expect(within(cards[0]).getByText(fact)).toBeTruthy();
    }
  });

  it('groups failed, partial, ready, unchecked and disabled sources separately', () => {
    render(SourcesHome, { props: { sources: demoSources } });

    const failedSection = screen.getByLabelText('需要处理');
    const partialSection = screen.getByLabelText('部分可用');
    const readySection = screen.getByLabelText('可用');
    const uncheckedSection = screen.getByLabelText('待检查');
    const disabledSection = screen.getByLabelText('已禁用');

    expect(within(failedSection).getByText('需要检查的音乐源')).toBeTruthy();
    expect(within(partialSection).getByText('部分能力漫画源')).toBeTruthy();
    expect(within(readySection).getByText('示例小说源')).toBeTruthy();
    expect(within(uncheckedSection).getByText('待检查 RSS 源')).toBeTruthy();
    expect(within(disabledSection).getByText('已禁用视频源')).toBeTruthy();
  });

  it('applies reduced opacity to disabled sources section', () => {
    render(SourcesHome, { props: { sources: demoSources } });

    const disabledSection = screen.getByLabelText('已禁用');
    expect(disabledSection.className).toContain('opacity-70');
  });

  it('renders all source action buttons as keyboard-reachable native buttons', () => {
    render(SourcesHome, { props: { sources: demoSources } });

    const allButtons = screen.getAllByRole('button');
    const actionButtons = allButtons.filter(
      (btn) =>
        btn.textContent?.includes('重试') ||
        btn.textContent?.includes('禁用') ||
        btn.textContent?.includes('启用') ||
        btn.textContent?.includes('开始检查') ||
        btn.textContent?.includes('移除'),
    );

    expect(actionButtons.length).toBeGreaterThan(0);
    for (const btn of actionButtons) {
      expect(btn.tagName).toBe('BUTTON');
      expect(btn.getAttribute('type')).toBe('button');
    }
  });
});
