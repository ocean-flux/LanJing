import { useCallback, useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { Icon } from '@/components/Icon';
import { PageToolbar } from '@/components/PageToolbar';
import { Button } from '@/components/ui/button';
import { ButtonGroup } from '@/components/ui/button-group';
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import { Item, ItemContent, ItemMedia, ItemTitle } from '@/components/ui/item';
import { Skeleton } from '@/components/ui/skeleton';
import { useMessages } from '@/shared/i18n/messages';
import { loadLibraryProjection, projectLibrary, type LibraryEntry } from '@/shared/tauri/library';
import { listNativeRuleDocuments } from '@/shared/tauri/rules';
import { listInstalledSources } from '@/shared/tauri/sources';

const CONTINUE_LIMIT = 5;

type RealmData = {
  continueEntries: LibraryEntry[];
  sourceCount: number;
  libraryCount: number;
  ruleCount: number;
};

const EMPTY_DATA: RealmData = {
  continueEntries: [],
  sourceCount: 0,
  libraryCount: 0,
  ruleCount: 0,
};

function progressPercent(entry: LibraryEntry): number | null {
  const { progress } = entry;
  if (!progress?.total) return null;
  return Math.min(100, Math.round((progress.position / progress.total) * 100));
}

export function RealmHome() {
  const m = useMessages();
  const [data, setData] = useState<RealmData>(EMPTY_DATA);
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading');
  const [error, setError] = useState('');

  const load = useCallback(async (signal: { cancelled: boolean }) => {
    setStatus('loading');
    setError('');
    try {
      const [projection, sources, rules] = await Promise.all([
        loadLibraryProjection(),
        listInstalledSources(),
        listNativeRuleDocuments(),
      ]);
      if (signal.cancelled) return;
      const entries = projectLibrary(projection);
      setData({
        continueEntries: entries
          .filter((entry) => entry.progress !== null)
          .sort((left, right) =>
            (right.last_opened_at ?? '').localeCompare(left.last_opened_at ?? ''),
          )
          .slice(0, CONTINUE_LIMIT),
        sourceCount: sources.length,
        libraryCount: entries.length,
        ruleCount: rules.length,
      });
      setStatus('ready');
    } catch (caught) {
      if (signal.cancelled) return;
      setError(caught instanceof Error ? caught.message : String(caught));
      setStatus('error');
    }
  }, []);

  useEffect(() => {
    const signal = { cancelled: false };
    void load(signal);
    return () => {
      signal.cancelled = true;
    };
  }, [load]);

  const stats = [
    { label: m.realm_stat_sources(), value: data.sourceCount },
    { label: m.realm_stat_library(), value: data.libraryCount },
    { label: m.realm_stat_rules(), value: data.ruleCount },
  ];

  return (
    <>
      <PageToolbar
        actions={
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={m.realm_refresh()}
            title={m.realm_refresh()}
            onClick={() => void load({ cancelled: false })}
          >
            <Icon name="arrow-clockwise" className="text-base" />
          </Button>
        }
      />

      <div className="grid gap-(--density-section-gap) px-(--page-gutter) py-(--density-section-gap) lg:grid-cols-[minmax(0,1fr)_18rem]">
        <section aria-labelledby="realm-continue" className="min-w-0">
          <h2 id="realm-continue" className="mb-2 text-ui-sm font-medium text-ink-muted">
            {m.realm_continue()}
          </h2>

          {status === 'loading' ? (
            <div className="flex flex-col gap-px">
              {[0, 1, 2].map((index) => (
                <Skeleton key={index} className="h-(--density-row) w-full" />
              ))}
            </div>
          ) : null}

          {status === 'error' ? (
            <div role="alert" className="border border-hairline p-3">
              <p className="font-medium">{m.realm_load_error()}</p>
              <p className="mt-1 font-mono text-ui-sm wrap-break-word text-ink-muted">{error}</p>
              <Button
                variant="outline"
                size="sm"
                className="mt-3"
                onClick={() => void load({ cancelled: false })}
              >
                {m.action_retry()}
              </Button>
            </div>
          ) : null}

          {status === 'ready' && data.continueEntries.length === 0 ? (
            <Empty className="border border-dashed border-hairline">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <Icon name="book-open" className="text-base" />
                </EmptyMedia>
                <EmptyTitle>{m.realm_continue_empty()}</EmptyTitle>
                <EmptyDescription>{m.realm_continue_empty_hint()}</EmptyDescription>
              </EmptyHeader>
              <Button
                variant="outline"
                size="sm"
                nativeButton={false}
                render={<Link to="/library" />}
              >
                {m.realm_open_library()}
              </Button>
            </Empty>
          ) : null}

          {status === 'ready' && data.continueEntries.length > 0 ? (
            <ul className="divide-y divide-hairline border-y border-hairline">
              {data.continueEntries.map((entry) => {
                const percent = progressPercent(entry);
                return (
                  <li key={entry.resource_id}>
                    <Item
                      size="xs"
                      className="hover:bg-surface-2"
                      render={
                        <Link to={`/library/item/${encodeURIComponent(entry.resource_id)}`} />
                      }
                    >
                      <ItemMedia variant="icon">
                        <Icon name="book-open" className="text-base text-ink-subtle" />
                      </ItemMedia>
                      <ItemContent>
                        <ItemTitle className="truncate font-mono">{entry.resource_id}</ItemTitle>
                      </ItemContent>
                      <span className="shrink-0 font-mono text-ui-sm text-ink-subtle tabular-nums">
                        {percent === null ? m.library_progress_recorded() : `${percent}%`}
                      </span>
                    </Item>
                  </li>
                );
              })}
            </ul>
          ) : null}
        </section>

        <div className="flex min-w-0 flex-col gap-(--density-section-gap)">
          <section aria-labelledby="realm-overview">
            <h2 id="realm-overview" className="mb-2 text-ui-sm font-medium text-ink-muted">
              {m.realm_overview()}
            </h2>
            <dl className="divide-y divide-hairline border-y border-hairline">
              {stats.map((stat) => (
                <div
                  key={stat.label}
                  className="flex h-(--density-row) items-center justify-between gap-2"
                >
                  <dt className="truncate text-ink-muted">{stat.label}</dt>
                  <dd className="font-mono tabular-nums">
                    {status === 'ready' ? stat.value : '—'}
                  </dd>
                </div>
              ))}
            </dl>
          </section>

          <section aria-labelledby="realm-actions">
            <h2 id="realm-actions" className="mb-2 text-ui-sm font-medium text-ink-muted">
              {m.realm_quick_actions()}
            </h2>
            <ButtonGroup className="w-full">
              <Button
                variant="outline"
                size="sm"
                nativeButton={false}
                render={<Link to="/sources" />}
              >
                <Icon name="download-simple" className="text-base" />
                {m.realm_action_import_source()}
              </Button>
              <Button
                variant="outline"
                size="sm"
                nativeButton={false}
                render={<Link to="/sources/rules" />}
              >
                <Icon name="tree-structure" className="text-base" />
                {m.realm_action_new_rule()}
              </Button>
              <Button
                variant="outline"
                size="sm"
                nativeButton={false}
                render={<Link to="/settings" />}
              >
                <Icon name="gear-six" className="text-base" />
                {m.realm_action_settings()}
              </Button>
            </ButtonGroup>
          </section>
        </div>
      </div>
    </>
  );
}
