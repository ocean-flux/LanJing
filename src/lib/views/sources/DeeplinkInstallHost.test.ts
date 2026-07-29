import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setLocale } from '$lib/i18n';
import type { DeepLinkIntent } from '$lib/deeplink/parse';
import {
  configureDeepLinkSession,
  enqueueDeepLinkIntents,
  resetDeepLinkSessionForTests,
} from '$lib/deeplink/session.svelte';
import type { InstallCandidate, InstalledSource } from '$lib/stores/rules.svelte';
import DeeplinkInstallHost from './DeeplinkInstallHost.svelte';

const store = vi.hoisted(() => ({
  prepareInstall: vi.fn(),
  installCandidate: vi.fn(),
}));

vi.mock('$lib/stores/rules.svelte', () => store);

const fetchImportSrc = vi.fn();
const navigate = vi.fn();

const installIntent: DeepLinkIntent = {
  kind: 'install',
  scheme: 'legado',
  sourceKind: 'bookSource',
  src: 'https://example.test/sources.json',
};

function createCandidate(
  id: string,
  title: string,
  requiredGrant: InstallCandidate['required_grant'] = {
    network: false,
    system: { fs: false, env: false, process: false },
  },
): InstallCandidate {
  return {
    id,
    expected_installed_revision: 0,
    profile: {
      id: `profile:${id}`,
      title,
      icon_url: null,
      version: '1.0.0',
      group: '玄幻',
      supported_intents: ['Search'],
      risk_notes: [],
    },
    required_grant: requiredGrant,
    diagnostics: [],
    definition_hash: `definition:${id}`,
    plan_hash: `plan:${id}`,
    expires_at_ms: Date.now() + 60_000,
  };
}

function createInstalledSource(candidate: InstallCandidate): InstalledSource {
  return {
    source_id: `source:${candidate.id}`,
    version: candidate.profile.version ?? '1.0.0',
    profile: candidate.profile,
    revision: 1,
  };
}

function openCatalog(entries: unknown[]): void {
  fetchImportSrc.mockResolvedValueOnce(JSON.stringify(entries));
  render(DeeplinkInstallHost);
  enqueueDeepLinkIntents([installIntent]);
}

beforeEach(async () => {
  await setLocale('zh-CN', { reload: false });
  resetDeepLinkSessionForTests();
  fetchImportSrc.mockReset();
  navigate.mockReset().mockResolvedValue(undefined);
  configureDeepLinkSession({ fetchImportSrc, navigate });
  store.prepareInstall.mockReset();
  store.installCandidate.mockReset();
});

afterEach(() => {
  resetDeepLinkSessionForTests();
});

describe('DeeplinkInstallHost', () => {
  it('distinguishes candidate validation from installation work', async () => {
    const preparation = Promise.withResolvers<InstallCandidate>();
    store.prepareInstall.mockReturnValueOnce(preparation.promise);
    openCatalog([{ bookSourceName: '待校验源', bookSourceGroup: '测试' }]);

    expect(await screen.findByRole('heading', { name: '导入书源' })).toBeTruthy();
    await fireEvent.click(screen.getByRole('checkbox', { name: '待校验源' }));
    await fireEvent.click(screen.getByRole('button', { name: '安装所选' }));

    await waitFor(() => {
      expect(screen.getByRole('status').textContent).toContain('正在校验');
    });
    expect(store.installCandidate).not.toHaveBeenCalled();

    preparation.resolve(createCandidate('candidate:pending', '待校验源'));
    expect(await screen.findByRole('heading', { name: '确认安装授权' })).toBeTruthy();
  });

  it('keeps group pick, explicit grant, working, and partial failure observable', async () => {
    const candidates: Record<string, InstallCandidate> = {
      甲源: createCandidate('candidate:one', '甲源', {
        network: true,
        system: { fs: false, env: false, process: false },
      }),
      乙源: createCandidate('candidate:two', '乙源', {
        network: true,
        system: { fs: false, env: false, process: false },
      }),
    };
    store.prepareInstall.mockImplementation(async (rawJson: string) => {
      const source = JSON.parse(rawJson) as { bookSourceName: string };
      const candidate = candidates[source.bookSourceName];
      if (!candidate) throw new Error('unexpected source');
      return candidate;
    });
    const secondInstall = Promise.withResolvers<InstalledSource>();
    store.installCandidate
      .mockResolvedValueOnce(createInstalledSource(candidates['甲源']!))
      .mockReturnValueOnce(secondInstall.promise);

    openCatalog([
      { bookSourceName: '甲源', bookSourceGroup: '玄幻' },
      { bookSourceName: '乙源', bookSourceGroup: '玄幻' },
      { bookSourceName: '丙源', bookSourceGroup: '其他' },
    ]);

    expect(await screen.findByRole('heading', { name: '导入书源' })).toBeTruthy();
    await fireEvent.click(screen.getByRole('checkbox', { name: '全选分组 玄幻' }));
    expect(screen.getByTestId('deeplink-selected-count').textContent).toContain('已选 2');
    expect(screen.getByRole('checkbox', { name: '丙源' }).getAttribute('aria-checked')).toBe(
      'false',
    );

    const prepareSelected = screen.getByRole('button', { name: '安装所选' });
    expect((prepareSelected as HTMLButtonElement).disabled).toBe(false);
    await fireEvent.click(prepareSelected);

    expect(await screen.findByRole('heading', { name: '确认安装授权' })).toBeTruthy();
    expect(store.prepareInstall).toHaveBeenCalledTimes(2);
    expect(screen.getByText('甲源')).toBeTruthy();
    expect(screen.getByText('乙源')).toBeTruthy();
    expect(screen.queryByText('丙源')).toBeNull();

    const grant = screen.getByRole('combobox', { name: '网络授权' });
    const confirm = screen.getByRole('button', { name: '安装来源' });
    expect((grant as HTMLSelectElement).value).toBe('none');
    expect((confirm as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.change(grant, { target: { value: 'network_only' } });
    expect((confirm as HTMLButtonElement).disabled).toBe(false);
    await fireEvent.click(confirm);

    await waitFor(() => {
      expect(store.installCandidate).toHaveBeenCalledTimes(2);
      expect(screen.getByRole('status').textContent).toContain('正在安装');
    });
    const workingGrant = screen.getByRole('combobox', { name: '网络授权' });
    expect((workingGrant as HTMLSelectElement).disabled).toBe(true);

    secondInstall.reject(new Error('policy denied'));

    expect(await screen.findByRole('heading', { name: '导入完成' })).toBeTruthy();
    expect(screen.getByRole('status').textContent).toContain('成功 1，失败 1');
    expect(screen.getByRole('alert').textContent).toContain('乙源: Error: policy denied');
    expect(store.installCandidate).toHaveBeenNthCalledWith(1, 'candidate:one', 'network_only');
    expect(store.installCandidate).toHaveBeenNthCalledWith(2, 'candidate:two', 'network_only');
    expect(screen.getAllByRole('button', { name: '关闭' }).length).toBeGreaterThan(0);
  });

  it('blocks confirmation when group selection exceeds the install cap', async () => {
    openCatalog(
      Array.from({ length: 51 }, (_, index) => ({
        bookSourceName: `来源 ${index + 1}`,
        bookSourceGroup: '超限分组',
      })),
    );

    expect(await screen.findByRole('heading', { name: '导入书源' })).toBeTruthy();
    await fireEvent.click(screen.getByRole('checkbox', { name: '全选分组 超限分组' }));

    expect(screen.getByRole('status').textContent).toContain('已选 51 条，单次最多安装 50 条');
    expect(screen.getByTestId('deeplink-selected-count').textContent).toContain('已选 51');
    const confirm = screen.getByRole('button', { name: '安装所选' });
    expect((confirm as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.click(confirm);
    expect(store.prepareInstall).not.toHaveBeenCalled();
  });

  it('shows unsupported system capabilities as a non-installable review', async () => {
    store.prepareInstall.mockResolvedValueOnce(
      createCandidate('candidate:blocked', '系统来源', {
        network: false,
        system: { fs: true, env: false, process: false },
      }),
    );
    openCatalog([{ bookSourceName: '系统来源' }]);

    expect(await screen.findByRole('heading', { name: '导入书源' })).toBeTruthy();
    await fireEvent.click(screen.getByRole('checkbox', { name: '系统来源' }));
    await fireEvent.click(screen.getByRole('button', { name: '安装所选' }));

    expect(await screen.findByRole('heading', { name: '确认安装授权' })).toBeTruthy();
    expect(screen.getByText('此来源请求了当前安装器无法授予的系统能力。')).toBeTruthy();
    expect(screen.queryByRole('combobox', { name: '网络授权' })).toBeNull();
    const confirm = screen.getByRole('button', { name: '安装来源' });
    expect((confirm as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.click(confirm);
    expect(store.installCandidate).not.toHaveBeenCalled();
  });
});
