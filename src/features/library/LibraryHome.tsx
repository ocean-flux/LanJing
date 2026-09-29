import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from 'react';
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
import { Input } from '@/components/ui/input';
import { Skeleton } from '@/components/ui/skeleton';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useMessages } from '@/shared/i18n/messages';
import type { LibraryEntry } from '@/shared/tauri/library';
import { LibraryInspector } from './LibraryInspector';
import { createLibraryWorkflow } from './workflow';

type LibraryTab = 'all' | 'favorites';

interface LibraryHomeProps {
  initialResourceId?: string;
  onInspectorClose?: () => void;
}

function progressLabel(entry: LibraryEntry, recorded: string, empty: string): string {
  if (!entry.progress) return empty;
  if (entry.progress.total === null) return `${recorded} ${entry.progress.position}`;
  return `${entry.progress.position}/${entry.progress.total}`;
}

function progressRatio(entry: LibraryEntry): number | null {
  if (!entry.progress || entry.progress.total === null || entry.progress.total <= 0) return null;
  return Math.min(1, entry.progress.position / entry.progress.total);
}

export function LibraryHome({ initialResourceId, onInspectorClose }: LibraryHomeProps) {
  const m = useMessages();
  const workflow = useMemo(() => createLibraryWorkflow(), []);
  const state = useSyncExternalStore(workflow.subscribe, workflow.getState, workflow.getState);
  const [query, setQuery] = useState('');
  const [tab, setTab] = useState<LibraryTab>('all');
  const [selectedResourceId, setSelectedResourceId] = useState<string | null>(
    initialResourceId ?? null,
  );

  useEffect(() => {
    void workflow.refresh();
  }, [workflow]);

  useEffect(() => {
    if (initialResourceId) setSelectedResourceId(initialResourceId);
  }, [initialResourceId]);

  const { rows } = state;
  const visibleRows = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return rows.filter(({ entry, media }) => {
      if (tab === 'favorites' && !entry.favorite) return false;
      if (!needle) return true;
      return [entry.resource_id, media?.title ?? '', media?.creator ?? ''].some((value) =>
        value.toLowerCase().includes(needle),
      );
    });
  }, [query, rows, tab]);

  const selectedRow = rows.find((row) => row.entry.resource_id === selectedResourceId);
  const selectedFailure = selectedResourceId ? state.failures[selectedResourceId] : undefined;
  const selectedPending = selectedResourceId
    ? state.pendingResourceIds.includes(selectedResourceId)
    : false;

  const closeInspector = useCallback(
    (open: boolean) => {
      if (open) return;
      setSelectedResourceId(null);
      onInspectorClose?.();
    },
    [onInspectorClose],
  );

  const toggleFavorite = useCallback(
    (entry: LibraryEntry) => {
      void workflow.updateOwnership(
        entry.resource_id,
        !entry.favorite,
        entry.favorite ? false : entry.pinned,
      );
    },
    [workflow],
  );

  const togglePinned = useCallback(
    (entry: LibraryEntry) => {
      void workflow.updateOwnership(
        entry.resource_id,
        entry.favorite || !entry.pinned,
        !entry.pinned,
      );
    },
    [workflow],
  );

  return (
    <>
      <PageToolbar
        meta={m.library_item_count({ count: visibleRows.length })}
        actions={
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={m.library_refresh()}
            title={m.library_refresh()}
            disabled={state.kind === 'loading'}
            onClick={() => void workflow.refresh()}
          >
            <Icon name="arrow-clockwise" className="text-base" />
          </Button>
        }
      >
        <Tabs
          value={tab}
          onValueChange={(value) => {
            if (value === 'all' || value === 'favorites') setTab(value);
          }}
        >
          <TabsList>
            <TabsTrigger value="all">{m.library_tab_all()}</TabsTrigger>
            <TabsTrigger value="favorites">{m.library_tab_favorites()}</TabsTrigger>
          </TabsList>
        </Tabs>
        <div className="ml-auto max-w-xs min-w-0 flex-1 sm:ml-2 sm:flex-none">
          <Input
            type="search"
            autoComplete="off"
            aria-label={m.library_search_label()}
            placeholder={m.library_search_placeholder()}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
        </div>
      </PageToolbar>

      <div className="px-(--page-gutter) py-(--density-section-gap)">
        {state.kind === 'loading' && rows.length === 0 ? (
          <div className="flex flex-col gap-px">
            {[0, 1, 2, 3, 4].map((index) => (
              <Skeleton key={index} className="h-(--density-row) w-full" />
            ))}
          </div>
        ) : null}

        {state.kind === 'error' ? (
          <div role="alert" className="border border-hairline p-3">
            <p className="font-medium">{m.library_load_error()}</p>
            <p className="mt-1 font-mono text-ui-sm wrap-break-word text-ink-muted">
              {state.detail}
            </p>
            <Button
              variant="outline"
              size="sm"
              className="mt-3"
              onClick={() => void workflow.refresh()}
            >
              {m.action_retry()}
            </Button>
          </div>
        ) : null}

        {state.kind === 'ready' && visibleRows.length === 0 ? (
          <Empty className="border border-dashed border-hairline">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <Icon name="books" className="text-base" />
              </EmptyMedia>
              <EmptyTitle>{m.library_empty_title()}</EmptyTitle>
              <EmptyDescription>
                {query || tab === 'favorites'
                  ? m.library_empty_filtered()
                  : m.library_empty_unfiltered()}
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : null}

        {visibleRows.length > 0 ? (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{m.library_col_resource()}</TableHead>
                <TableHead>{m.library_inspector_media()}</TableHead>
                <TableHead className="w-32">{m.library_col_progress()}</TableHead>
                <TableHead className="w-44">{m.library_col_opened()}</TableHead>
                <TableHead className="w-20 text-right">{m.library_favorite()}</TableHead>
                <TableHead className="w-20 text-right">{m.library_pinned()}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {visibleRows.map(({ entry, media }) => {
                const ratio = progressRatio(entry);
                const pending = state.pendingResourceIds.includes(entry.resource_id);
                const failure = state.failures[entry.resource_id];
                return (
                  <TableRow key={entry.resource_id}>
                    <TableCell className="max-w-0">
                      <button
                        type="button"
                        className="block max-w-full truncate text-left font-mono hover:underline focus-visible:outline-1 focus-visible:outline-ring"
                        onClick={() => setSelectedResourceId(entry.resource_id)}
                      >
                        {entry.resource_id}
                      </button>
                      {failure ? (
                        <span
                          className="flex items-center gap-1 text-ui-sm text-danger"
                          role="alert"
                        >
                          <Icon name="warning-circle" />
                          <span className="truncate">
                            {m.library_update_error({ detail: failure })}
                          </span>
                        </span>
                      ) : null}
                    </TableCell>
                    <TableCell className="max-w-0">
                      {media ? (
                        <div className="min-w-0">
                          <p className="truncate">{media.title}</p>
                          <p className="truncate text-ui-sm text-ink-muted">{media.creator}</p>
                        </div>
                      ) : (
                        <span className="text-ink-muted">{m.library_media_missing()}</span>
                      )}
                    </TableCell>
                    <TableCell>
                      <div className="flex items-center gap-2">
                        <span className="w-16 shrink-0 font-mono text-ink-muted tabular-nums">
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
                      {entry.last_opened_at ?? m.library_value_none()}
                    </TableCell>
                    <TableCell className="text-right">
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        aria-label={entry.favorite ? m.library_unfavorite() : m.library_favorite()}
                        title={entry.favorite ? m.library_unfavorite() : m.library_favorite()}
                        disabled={pending}
                        onClick={() => toggleFavorite(entry)}
                      >
                        <Icon name={entry.favorite ? 'star-fill' : 'star'} className="text-base" />
                      </Button>
                    </TableCell>
                    <TableCell className="text-right">
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        aria-label={entry.pinned ? m.library_unpin() : m.library_pin()}
                        title={entry.pinned ? m.library_unpin() : m.library_pin()}
                        disabled={pending}
                        onClick={() => togglePinned(entry)}
                      >
                        <Icon
                          name={entry.pinned ? 'push-pin-fill' : 'push-pin'}
                          className="text-base"
                        />
                      </Button>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        ) : null}
      </div>

      <LibraryInspector
        open={selectedResourceId !== null}
        row={selectedRow}
        pending={selectedPending}
        failure={selectedFailure}
        onOpenChange={closeInspector}
        onFavoriteChange={() => {
          if (selectedRow) toggleFavorite(selectedRow.entry);
        }}
        onPinnedChange={() => {
          if (selectedRow) togglePinned(selectedRow.entry);
        }}
      />
    </>
  );
}
