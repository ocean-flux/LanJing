import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import {
  demoSources,
  noSourceRealmState,
  sourceNoResourceRealmState,
  sourceWarningRealmState,
} from '$lib/app/demo-state';
import type { RealmState } from '$lib/app/shell-types';
import RealmHome from './RealmHome.svelte';

describe('RealmHome', () => {
  it('shows complete no-source state without fake shelves', () => {
    render(RealmHome, { props: { state: noSourceRealmState, sources: [] } });

    expect(screen.getByRole('heading', { name: '构建你的媒体境场' })).toBeTruthy();
    expect(screen.getByTestId('realm-hero')).toBeTruthy();
    expect(screen.getAllByRole('link', { name: /添加来源/ })[0]?.getAttribute('href')).toBe(
      '/sources',
    );
    expect(screen.getAllByRole('link', { name: /导入本地文件/ })[0]?.getAttribute('href')).toBe(
      '/sources',
    );
    // 无源态只保留单 hero，不重复堆「媒体入口等待接入」副卡
    expect(screen.getAllByText('不显示假推荐；接入后再生成内容。').length).toBeGreaterThan(0);
    expect(screen.queryByText('推荐')).toBeNull();
    expect(screen.queryByText('历史')).toBeNull();
    expect(screen.queryByText('导入规则')).toBeNull();
    expect(screen.queryByText('执行 Witness')).toBeNull();
  });

  it('shows source-no-resource as normal initial state', () => {
    render(RealmHome, {
      props: { state: sourceNoResourceRealmState, sources: demoSources.slice(0, 1) },
    });

    expect(screen.getByRole('heading', { name: '已添加来源，暂无媒体资源' })).toBeTruthy();
    expect(screen.getAllByRole('link', { name: /搜索内容/ })[0]?.getAttribute('href')).toBe(
      '/apps',
    );
    expect(screen.getAllByRole('link', { name: /打开发现/ })[0]?.getAttribute('href')).toBe(
      '/apps',
    );
    expect(screen.getAllByRole('link', { name: /检查来源/ })[0]?.getAttribute('href')).toBe(
      '/sources',
    );
  });

  it('localizes source warnings without turning the realm into an error page', () => {
    render(RealmHome, { props: { state: sourceWarningRealmState, sources: demoSources } });

    expect(screen.getByRole('heading', { name: '来源需要处理，境场仍可使用' })).toBeTruthy();
    expect(screen.getByText('来源状态提示')).toBeTruthy();
    expect(screen.getByText('需要检查的音乐源')).toBeTruthy();
  });

  it('uses typed action hrefs without deriving navigation from localized labels', () => {
    const state: RealmState = {
      kind: 'has-content',
      title: '已有真实内容',
      description: '内容已从真实资源投影。',
      primaryAction: { kind: 'search-content', label: '自定义主动作', href: '/sources' },
      secondaryAction: { kind: 'import-local', label: '自定义次动作', href: '/apps' },
    };

    render(RealmHome, { props: { state, sources: demoSources.slice(0, 1) } });

    expect(screen.getByRole('link', { name: '自定义主动作' }).getAttribute('href')).toBe(
      '/sources',
    );
    expect(screen.getByRole('link', { name: '自定义次动作' }).getAttribute('href')).toBe('/apps');
    expect(screen.queryByText('媒体入口等待接入')).toBeNull();
  });
});
