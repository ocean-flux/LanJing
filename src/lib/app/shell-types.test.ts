import { describe, expect, it } from 'vitest';
import { demoSources, mediaAppCards, noSourceRealmState } from './demo-state';

describe('demo shell state', () => {
  it('keeps no-source realm state actionable', () => {
    expect(noSourceRealmState.kind).toBe('no-source');
    expect(noSourceRealmState.primaryAction).toBe('添加来源');
    expect(noSourceRealmState.secondaryAction).toBe('导入本地文件');
  });

  it('exposes isolated failure actions and trust facts for failed sources', () => {
    const failed = demoSources.find((source) => source.status === 'failed');

    expect(failed?.actions).toEqual(expect.arrayContaining(['重试', '查看原因', '禁用']));
    expect(failed?.trustFacts.map((fact) => fact.label)).toEqual(
      expect.arrayContaining(['来源', '网络访问', '远程解析', '失败隔离']),
    );
  });

  it('makes novel explorable without enabling every app', () => {
    const novel = mediaAppCards.find((app) => app.key === 'novel');
    const video = mediaAppCards.find((app) => app.key === 'video');

    expect(novel?.href).toBe('/apps/novel');
    expect(novel?.status).toBe('explorable');
    expect(video?.href).toBeUndefined();
    expect(video?.status).toBe('unconnected');
  });
});
