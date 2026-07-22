import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import { demoSources } from '$lib/app/demo-state';
import SourcesHome from './SourcesHome.svelte';

describe('SourcesHome', () => {
  it('defaults to honest empty state without demo sources', () => {
    render(SourcesHome);

    expect(screen.getByRole('heading', { name: '添加来源' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: '还没有来源' })).toBeTruthy();
    expect(screen.getByTestId('sources-empty').textContent).toContain('不会显示示例繁荣列表');
    expect(screen.getByTestId('add-source-panel').className).toContain('double-bezel');
    expect(screen.queryByText('示例小说源')).toBeNull();
    expect(screen.queryByText('需要检查的音乐源')).toBeNull();
  });

  it('shows honest source entry choices when no sources exist', () => {
    render(SourcesHome, { props: { sources: [] } });

    expect(screen.getByRole('heading', { name: '添加来源' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: '还没有来源' })).toBeTruthy();
    expect(screen.getByRole('radio', { name: /订阅链接/ })).toBeTruthy();
  });

  it('sorts source cards by attention without rendering fake action buttons', () => {
    render(SourcesHome, { props: { sources: demoSources } });

    const cards = screen.getAllByRole('article');
    expect(within(cards[0]).getByText('需要检查的音乐源')).toBeTruthy();
    expect(within(cards[0]).getByText('需要处理')).toBeTruthy();
    expect(within(cards[0]).queryByRole('button', { name: '重试' })).toBeNull();
    expect(within(cards[0]).getByRole('group', { name: '能力' })).toBeTruthy();

    for (const fact of ['来源', '网络访问', '远程解析', '失败隔离']) {
      expect(within(cards[0]).getByText(fact)).toBeTruthy();
    }
  });

  it('passes native source actions through an observable typed callback', async () => {
    const onaction = vi.fn();
    render(SourcesHome, { props: { sources: demoSources, onaction } });

    const retry = screen.getByRole('button', { name: '重试' });
    expect(retry.tagName).toBe('BUTTON');
    expect(retry.getAttribute('type')).toBe('button');
    await fireEvent.click(retry);

    expect(onaction).toHaveBeenCalledWith({ sourceId: 'demo-warning', action: '重试' });
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

  it('uses semantic disabled styling instead of fading the whole group', () => {
    render(SourcesHome, { props: { sources: demoSources } });

    const disabledSection = screen.getByLabelText('已禁用');
    const disabledCard = within(disabledSection).getByRole('article');
    expect(disabledSection.className).not.toContain('opacity-70');
    expect(disabledCard.getAttribute('data-status')).toBe('disabled');
  });
});
