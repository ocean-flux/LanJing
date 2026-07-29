import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { InstalledSource } from '$lib/stores/rules.svelte';
import SourcesHome from './SourcesHome.svelte';

const appState = vi.hoisted(() => ({
  page: { url: new URL('https://lanjing.test/sources') },
}));

vi.mock('$app/state', () => appState);

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
    prepareMaccmsInstall: vi.fn(),
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
    prepareMaccmsInstall: store.prepareMaccmsInstall,
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
    group: '玄幻',
    supported_intents: ['Search', 'ResolveItem'],
    risk_notes: ['仅访问目标站点'],
  },
};

const ungroupedSource: InstalledSource = {
  source_id: 'source:two',
  version: '1.0.0',
  revision: 1,
  profile: {
    id: 'profile:two',
    title: '未分组来源',
    icon_url: null,
    version: '1.0.0',
    group: null,
    supported_intents: ['Discover'],
    risk_notes: [],
  },
};

beforeEach(() => {
  appState.page.url = new URL('https://lanjing.test/sources');
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

  it('shows one retry for load errors and recovers to the empty state', async () => {
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
    expect(within(alert).queryByRole('button')).toBeNull();
    const retry = screen.getByRole('button', { name: '重试' });
    expect(retry.closest('[role="alert"]')).toBeNull();

    await fireEvent.click(retry);

    expect(await screen.findByRole('heading', { name: '还没有已安装来源' })).toBeTruthy();
    expect(screen.queryByRole('textbox', { name: '规则 JSON' })).toBeNull();
    expect(screen.getAllByRole('button', { name: '添加来源' })).toHaveLength(1);
    await fireEvent.click(screen.getByRole('button', { name: '添加来源' }));
    expect(await screen.findByRole('textbox', { name: '规则 JSON' })).toBeTruthy();
    expect(store.loadInstalledSources).toHaveBeenCalledTimes(2);
  });

  it('renders installed source facts and opens the sheet installer', async () => {
    store.setSnapshot({ sources: [installedSource] });
    render(SourcesHome);

    const sourceHeading = await screen.findByRole('heading', { name: '真实来源' });
    const frame = document.querySelector('[data-slot="page-frame"]');
    expect(frame?.getAttribute('data-width')).toBe('standard');
    const article = sourceHeading.closest('article');
    expect(article).toBeTruthy();
    expect(within(article!).getByText('2.3.1')).toBeTruthy();
    expect(within(article!).getByText('玄幻')).toBeTruthy();
    expect(within(article!).queryByText('7')).toBeNull();
    expect(within(article!).getByText('搜索')).toBeTruthy();
    expect(within(article!).getByText('解析条目')).toBeTruthy();
    expect(within(article!).queryByRole('link')).toBeNull();
    expect(within(article!).getByText('仅访问目标站点')).toBeTruthy();

    const add = screen.getByRole('button', { name: '添加来源' });
    expect(screen.getAllByRole('button', { name: '添加来源' })).toHaveLength(1);
    expect(screen.queryByRole('textbox', { name: '规则 JSON' })).toBeNull();
    await fireEvent.click(add);
    expect(await screen.findByRole('textbox', { name: '规则 JSON' })).toBeTruthy();
  });

  it('filters the source list with honest pressed-group semantics', async () => {
    store.setSnapshot({ sources: [installedSource, ungroupedSource] });
    render(SourcesHome);

    expect(await screen.findByRole('heading', { name: '真实来源' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: '未分组来源' })).toBeTruthy();

    const filters = screen.getByRole('group', { name: '按分组筛选' });
    const allFilter = within(filters).getByRole('button', { name: '全部' });
    const fantasyFilter = within(filters).getByRole('button', { name: '玄幻' });
    const ungroupedFilter = within(filters).getByRole('button', { name: '未分组' });
    expect(allFilter.getAttribute('aria-pressed')).toBe('true');
    expect(fantasyFilter.getAttribute('aria-pressed')).toBe('false');

    await fireEvent.click(fantasyFilter);
    expect(allFilter.getAttribute('aria-pressed')).toBe('false');
    expect(fantasyFilter.getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByRole('heading', { name: '真实来源' })).toBeTruthy();
    expect(screen.queryByRole('heading', { name: '未分组来源' })).toBeNull();

    await fireEvent.click(ungroupedFilter);
    expect(fantasyFilter.getAttribute('aria-pressed')).toBe('false');
    expect(ungroupedFilter.getAttribute('aria-pressed')).toBe('true');
    expect(screen.queryByRole('heading', { name: '真实来源' })).toBeNull();
    expect(screen.getByRole('heading', { name: '未分组来源' })).toBeTruthy();

    await fireEvent.click(allFilter);
    expect(allFilter.getAttribute('aria-pressed')).toBe('true');
    expect(ungroupedFilter.getAttribute('aria-pressed')).toBe('false');
    expect(screen.getByRole('heading', { name: '真实来源' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: '未分组来源' })).toBeTruthy();
  });

  it('highlights the deep-linked source and announces collapsible group state', async () => {
    appState.page.url = new URL('https://lanjing.test/sources?highlight=source%3Aone');
    store.setSnapshot({ sources: [installedSource, ungroupedSource] });
    render(SourcesHome);

    const sourceHeading = await screen.findByRole('heading', { name: '真实来源' });
    const sourceArticle = sourceHeading.closest('article');
    const ungroupedArticle = screen.getByRole('heading', { name: '未分组来源' }).closest('article');
    expect(sourceArticle?.getAttribute('data-highlighted')).toBe('true');
    expect(ungroupedArticle?.getAttribute('data-highlighted')).toBe('false');

    const collapse = screen.getByRole('button', { name: '折叠分组：玄幻' });
    expect(collapse.getAttribute('aria-expanded')).toBe('true');
    await fireEvent.click(collapse);

    const expand = screen.getByRole('button', { name: '展开分组：玄幻' });
    expect(expand.getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByRole('heading', { name: '真实来源' })).toBeNull();

    await fireEvent.click(expand);
    expect(await screen.findByRole('heading', { name: '真实来源' })).toBeTruthy();
  });

  it('opens the shared sheet from the successful empty state CTA', async () => {
    render(SourcesHome);

    expect(await screen.findByRole('heading', { name: '还没有已安装来源' })).toBeTruthy();
    expect(screen.queryByRole('heading', { name: '安装来源' })).toBeNull();
    expect(screen.queryByRole('textbox', { name: '规则 JSON' })).toBeNull();
    const addSource = screen.getByRole('button', { name: '添加来源' });
    expect(screen.getAllByRole('button', { name: '添加来源' })).toHaveLength(1);
    expect(addSource.closest('[role="status"]')).toBeNull();

    await fireEvent.click(addSource);

    expect(await screen.findByRole('heading', { name: '安装来源' })).toBeTruthy();
    expect(screen.getByRole('textbox', { name: '规则 JSON' })).toBeTruthy();
    await waitFor(() => expect(store.loadInstalledSources).toHaveBeenCalledOnce());
  });
});
