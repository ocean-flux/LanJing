import { BookOpen, Clock3, FolderHeart, RefreshCw } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import { Badge, Button, Card, CardContent, Input } from '@/components/ui';
import { useMessages } from '@/shared/i18n/messages';
import { loadLibraryProjection, projectLibrary, type LibraryEntry } from '@/shared/tauri/library';

export function LibraryHome() {
  const m = useMessages();
  const [entries, setEntries] = useState<LibraryEntry[]>([]);
  const [query, setQuery] = useState('');
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading');
  const [error, setError] = useState('');
  const latestLoadRequest = useRef(0);

  const load = async () => {
    const requestId = latestLoadRequest.current + 1;
    latestLoadRequest.current = requestId;
    setStatus('loading');
    setError('');
    try {
      const projection = await loadLibraryProjection();
      if (requestId !== latestLoadRequest.current) return;
      setEntries(projectLibrary(projection));
      setStatus('ready');
    } catch (caught) {
      if (requestId !== latestLoadRequest.current) return;
      setError(caught instanceof Error ? caught.message : String(caught));
      setStatus('error');
    }
  };

  useEffect(() => {
    void load();
  }, []);

  const filtered = useMemo(
    () => entries.filter((entry) => entry.resource_id.toLowerCase().includes(query.toLowerCase())),
    [entries, query],
  );

  return (
    <div className="mx-auto max-w-7xl px-5 py-8 sm:px-8 lg:px-12 lg:py-12">
      <div className="flex flex-col justify-between gap-5 border-b border-(--border) pb-8 sm:flex-row sm:items-end">
        <div>
          <p className="eyebrow">{m.library_eyebrow()}</p>
          <h1 className="font-display mt-2 text-4xl font-semibold text-balance">
            {m.library_heading()}
          </h1>
          <p className="mt-3 text-(--muted-text)">{m.library_description()}</p>
        </div>
        <div className="flex items-center gap-3">
          <Badge className="w-fit">{m.library_item_count({ count: entries.length })}</Badge>
          <Button
            variant="outline"
            size="icon"
            onClick={() => void load()}
            aria-label={m.library_refresh()}
            title={m.library_refresh()}
          >
            <RefreshCw size={16} aria-hidden="true" />
          </Button>
        </div>
      </div>

      <div className="mt-7 max-w-md">
        <label className="sr-only" htmlFor="library-search">
          {m.library_search_label()}
        </label>
        <Input
          id="library-search"
          name="library-search"
          type="search"
          autoComplete="off"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={m.library_search_placeholder()}
        />
      </div>

      {status === 'error' && (
        <div className="mt-6 border border-(--border) p-4" role="alert">
          <p className="font-medium">{m.library_load_error()}</p>
          <p className="mt-1 text-sm break-words text-(--muted-text)">{error}</p>
        </div>
      )}
      {status === 'ready' && filtered.length === 0 && (
        <div className="mt-6 border border-dashed border-(--border) p-8 text-center">
          <FolderHeart className="mx-auto text-(--muted-text)" size={24} aria-hidden="true" />
          <p className="mt-3 font-medium">{m.library_empty_title()}</p>
          <p className="mt-2 text-sm text-(--muted-text)">
            {query ? m.library_empty_filtered() : m.library_empty_unfiltered()}
          </p>
        </div>
      )}
      <div className="mt-8 grid gap-4 lg:grid-cols-3">
        {filtered.map((entry) => (
          <Link
            key={entry.resource_id}
            to={`/library/item/${encodeURIComponent(entry.resource_id)}`}
          >
            <Card className="h-full transition-[transform,box-shadow] hover:-translate-y-0.5 hover:shadow-md">
              <CardContent className="pt-5">
                <div className="flex items-start justify-between gap-4">
                  <span className="grid h-11 w-11 place-items-center rounded-md bg-(--accent-soft) text-(--accent-strong)">
                    <BookOpen size={20} aria-hidden="true" />
                  </span>
                  {entry.pinned && <Badge>{m.library_pinned()}</Badge>}
                </div>
                <h2 className="mt-8 font-mono text-base font-semibold break-words">
                  {entry.resource_id}
                </h2>
                {entry.progress && (
                  <div className="mt-6">
                    <div className="flex justify-between text-xs text-(--muted-text)">
                      <span>{m.library_progress()}</span>
                      <span>
                        {entry.progress.total
                          ? `${Math.round((entry.progress.position / entry.progress.total) * 100)}%`
                          : m.library_progress_recorded()}
                      </span>
                    </div>
                    {entry.progress.total && (
                      <div className="mt-2 h-1 overflow-hidden rounded-full bg-(--surface-3)">
                        <div
                          className="h-full rounded-full bg-(--accent)"
                          style={{
                            width: `${Math.min(100, (entry.progress.position / entry.progress.total) * 100)}%`,
                          }}
                        />
                      </div>
                    )}
                  </div>
                )}
              </CardContent>
            </Card>
          </Link>
        ))}
      </div>
    </div>
  );
}

export function LibraryItem({ resourceId }: { resourceId: string }) {
  const m = useMessages();
  const [entry, setEntry] = useState<LibraryEntry>();
  const [loadError, setLoadError] = useState('');

  useEffect(() => {
    let cancelled = false;
    setEntry(undefined);
    setLoadError('');
    void loadLibraryProjection()
      .then((projection) => {
        if (cancelled) return;
        setEntry(projectLibrary(projection).find((item) => item.resource_id === resourceId));
      })
      .catch((caught: unknown) => {
        if (cancelled) return;
        setLoadError(caught instanceof Error ? caught.message : String(caught));
      });
    return () => {
      cancelled = true;
    };
  }, [resourceId]);

  return (
    <div className="mx-auto max-w-4xl px-5 py-8 sm:px-8 lg:px-12 lg:py-12">
      <Link to="/library" className="text-sm text-(--accent-strong) hover:underline">
        ← {m.library_detail_back()}
      </Link>
      <div className="mt-10 max-w-2xl">
        <Badge>{m.library_local_resource()}</Badge>
        <h1 className="mt-4 font-mono text-3xl font-semibold break-words sm:text-5xl">
          {resourceId}
        </h1>
        <p className="mt-8 leading-8 text-(--muted-text)">{m.library_detail_description()}</p>
      </div>
      {loadError && (
        <p className="mt-6 text-sm break-words text-(--muted-text)" role="alert">
          {m.library_local_state_error({ detail: loadError })}
        </p>
      )}
      <Card className="mt-10">
        <CardContent className="flex items-center gap-4 pt-5">
          <Clock3 className="text-(--accent-strong)" aria-hidden="true" />
          <div>
            <p className="font-medium">
              {entry ? m.library_local_state_found() : m.library_local_state_waiting()}
            </p>
            <p className="mt-1 text-sm text-(--muted-text)">
              {entry?.progress ? m.library_progress_loaded() : m.library_progress_missing()}
            </p>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
