import { useCallback, useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { Icon } from '@/components/Icon';
import { PageToolbar } from '@/components/PageToolbar';
import { Button } from '@/components/ui/button';
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import { InputGroup, InputGroupAddon, InputGroupInput } from '@/components/ui/input-group';
import { Skeleton } from '@/components/ui/skeleton';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { useMessages } from '@/shared/i18n/messages';
import { loadLibraryProjection, projectLibrary, type LibraryEntry } from '@/shared/tauri/library';

function progressLabel(entry: LibraryEntry, recorded: string, none: string): string {
  const { progress } = entry;
  if (!progress) return none;
  if (!progress.total) return recorded;
  return `${Math.min(100, Math.round((progress.position / progress.total) * 100))}%`;
}

function progressRatio(entry: LibraryEntry): number | null {
  const { progress } = entry;
  if (!progress?.total) return null;
  return Math.min(1, progress.position / progress.total);
}

export function LibraryHome() {
  const m = useMessages();
  const [entries, setEntries] = useState<LibraryEntry[]>([]);
  const [query, setQuery] = useState('');
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading');
  const [error, setError] = useState('');

  const load = useCallback(async (signal: { cancelled: boolean }) => {
    setStatus('loading');
    setError('');
    try {
      const projection = await loadLibraryProjection();
      if (signal.cancelled) return;
      setEntries(projectLibrary(projection));
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

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return entries;
    return entries.filter((entry) => entry.resource_id.toLowerCase().includes(needle));
  }, [entries, query]);

  return (
    <>
      <PageToolbar
        meta={m.library_item_count({ count: filtered.length })}
        actions={
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={m.library_refresh()}
            title={m.library_refresh()}
            onClick={() => void load({ cancelled: false })}
          >
            <Icon name="arrow-clockwise" className="text-base" />
          </Button>
        }
      >
        <InputGroup className="ml-2 max-w-xs">
          <InputGroupAddon>
            <Icon name="magnifying-glass" className="text-base text-ink-subtle" />
          </InputGroupAddon>
          <InputGroupInput
            type="search"
            autoComplete="off"
            aria-label={m.library_search_label()}
            placeholder={m.library_search_placeholder()}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
        </InputGroup>
      </PageToolbar>

      <div className="px-(--page-gutter) py-(--density-section-gap)">
        {status === 'loading' ? (
          <div className="flex flex-col gap-px">
            {[0, 1, 2, 3, 4].map((index) => (
              <Skeleton key={index} className="h-(--density-row) w-full" />
            ))}
          </div>
        ) : null}

        {status === 'error' ? (
          <div role="alert" className="border border-hairline p-3">
            <p className="font-medium">{m.library_load_error()}</p>
            <p className="mt-1 font-mono text-ui-sm break-words text-ink-muted">{error}</p>
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

        {status === 'ready' && filtered.length === 0 ? (
          <Empty className="border border-dashed border-hairline">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <Icon name="books" className="text-base" />
              </EmptyMedia>
              <EmptyTitle>{m.library_empty_title()}</EmptyTitle>
              <EmptyDescription>
                {query ? m.library_empty_filtered() : m.library_empty_unfiltered()}
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : null}

        {status === 'ready' && filtered.length > 0 ? (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{m.library_col_resource()}</TableHead>
                <TableHead className="w-32">{m.library_col_progress()}</TableHead>
                <TableHead className="w-24">{m.library_col_state()}</TableHead>
                <TableHead className="w-44">{m.library_col_opened()}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {filtered.map((entry) => {
                const ratio = progressRatio(entry);
                return (
                  <TableRow key={entry.resource_id}>
                    <TableCell className="max-w-0">
                      <Link
                        to={`/library/item/${encodeURIComponent(entry.resource_id)}`}
                        className="block truncate font-mono hover:underline"
                      >
                        {entry.resource_id}
                      </Link>
                    </TableCell>
                    <TableCell>
                      <div className="flex items-center gap-2">
                        <span className="w-10 shrink-0 font-mono text-ink-muted tabular-nums">
                          {progressLabel(
                            entry,
                            m.library_progress_recorded(),
                            m.library_value_none(),
                          )}
                        </span>
                        {ratio === null ? null : (
                          <span className="h-0.5 min-w-0 flex-1 bg-surface-3">
                            <span
                              className="block h-full bg-lantern-strong"
                              style={{ width: `${ratio * 100}%` }}
                            />
                          </span>
                        )}
                      </div>
                    </TableCell>
                    <TableCell className="text-ink-muted">
                      {entry.pinned ? m.library_pinned() : null}
                    </TableCell>
                    <TableCell className="font-mono text-ink-muted">
                      {entry.last_opened_at ?? m.library_value_none()}
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        ) : null}
      </div>
    </>
  );
}
