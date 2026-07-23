import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { InstalledSource } from '$lib/stores/rules.svelte';
import SourcesHome from './SourcesHome.svelte';

const store = vi.hoisted(() => {
  const snapshot: {
    sources: InstalledSource[];
    loading: boolean;
    error: string | null;
  } = {
    sources: [],
    loading: false,
    error: null,
  };
  const subscribers = new Set<() => void>();

  const setSnapshot = (patch: Partial<typeof snapshot>) => {
    Object.assign(snapshot, patch);
    for (const update of subscribers) {
      update();
    }
  };

  return {
    snapshot,
    subscribers,
    setSnapshot,
    loadInstalledSources: vi.fn<() => Promise<void>>(),
    prepareInstall: vi.fn(),
    installCandidate: vi.fn(),
  };
});

vi.mock('$lib/stores/rules.svelte', async () => {
  // vi.mock factory 会被提升，Svelte reactivity 必须在 factory 内加载。
  const { createSubscriber } = await import('svelte/reactivity');
  const subscribe = createSubscriber((update) => {
    store.subscribers.add(update);
    return () => store.subscribers.delete(update);
  });

  return {
    loadInstalledSources: store.loadInstalledSources,
    prepareInstall: store.prepareInstall,
    installCandidate: store.installCandidate,
    getInstalledSources: () => {
      subscribe();
      return store.snapshot.sources;
    },
    getLoading: () => {
      subscribe();
      return store.snapshot.loading;
    },
    getError: () => {
      subscribe();
      return store.snapshot.error;
    },
  };
});

const installedSource: InstalledSource = {
  source_id: 'source:one',
  version: '2.3.1',
  revision: 7,
  profile: {
    id: 'profile:one',
    title: '真实来源',
    icon_url: null,
    version: '2.3.1',
    supported_intents: ['Search', 'ResolveItem'],
    risk_notes: ['仅访问目标站点'],
  },
};

beforeEach(() => {
  store.setSnapshot({ sources: [], loading: false, error: null });
  store.loadInstalledSources.mockReset().mockResolvedValue(undefined);
  store.prepareInstall.mockReset();
  store.installCandidate.mockReset();
});

describe('SourcesHome', () => {
  it('shows loading while the initial store refresh is pending', () => {
    store.loadInstalledSources.mockReturnValueOnce(Promise.withResolvers<void>().promise);
    render(SourcesHome);

    expect(screen.getByRole('status').textContent).toContain('正在加载已安装来源');
    expect(screen.queryByRole('textbox', { name: '规则 JSON' })).toBeNull();
  });

  it('shows one retry for load errors and recovers to the real empty install flow', async () => {
    store.loadInstalledSources
      .mockImplementationOnce(async () => {
        store.setSnapshot({ error: 'offline' });
      })
      .mockImplementationOnce(async () => {
        store.setSnapshot({ error: null });
      });
    render(SourcesHome);

    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('来源加载失败');
    expect(within(alert).getAllByRole('button')).toHaveLength(1);

    await fireEvent.click(within(alert).getByRole('button', { name: '重试' }));

    expect(await screen.findByRole('heading', { name: '还没有已安装来源' })).toBeTruthy();
    expect(screen.getByRole('textbox', { name: '规则 JSON' })).toBeTruthy();
    expect(store.loadInstalledSources).toHaveBeenCalledTimes(2);
  });

  it('renders installed source facts and exposes one installer entry', async () => {
    store.setSnapshot({ sources: [installedSource] });
    render(SourcesHome);

    const sourceHeading = await screen.findByRole('heading', { name: '真实来源' });
    const article = sourceHeading.closest('article');
    expect(article).toBeTruthy();
    expect(within(article!).getByText('2.3.1')).toBeTruthy();
    expect(within(article!).getByText('7')).toBeTruthy();
    expect(within(article!).getByText('搜索')).toBeTruthy();
    expect(within(article!).getByText('解析条目')).toBeTruthy();
    expect(within(article!).getByText('仅访问目标站点')).toBeTruthy();

    const add = screen.getByRole('button', { name: '添加来源' });
    expect(screen.getAllByRole('button', { name: '添加来源' })).toHaveLength(1);
    await fireEvent.click(add);
    expect(await screen.findByRole('textbox', { name: '规则 JSON' })).toBeTruthy();
    expect(screen.getByRole('button', { name: '收起安装' })).toBeTruthy();
  });

  it('shows the real install flow immediately for a successful empty load', async () => {
    render(SourcesHome);

    expect(await screen.findByRole('heading', { name: '还没有已安装来源' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: '安装来源' })).toBeTruthy();
    expect(screen.getByRole('textbox', { name: '规则 JSON' })).toBeTruthy();
    await waitFor(() => expect(store.loadInstalledSources).toHaveBeenCalledOnce());
  });
});
