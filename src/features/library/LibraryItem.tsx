import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { Icon } from '@/components/Icon';
import { PageToolbar } from '@/components/PageToolbar';
import { Button } from '@/components/ui/button';
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from '@/components/ui/empty';
import { Skeleton } from '@/components/ui/skeleton';
import { useMessages } from '@/shared/i18n/messages';
import { loadLibraryProjection, projectLibrary, type LibraryEntry } from '@/shared/tauri/library';

type LoadState =
  | { kind: 'loading' }
  | { kind: 'ready'; entry: LibraryEntry | undefined }
  | { kind: 'error'; detail: string };

export function LibraryItem({ resourceId }: { resourceId: string }) {
  const m = useMessages();
  const [state, setState] = useState<LoadState>({ kind: 'loading' });
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    // 切换条目时先回到 loading，避免上一条的数据短暂串到新地址上。
    setState({ kind: 'loading' });
    let cancelled = false;
    void loadLibraryProjection()
      .then((projection) => {
        if (cancelled) return;
        setState({
          kind: 'ready',
          entry: projectLibrary(projection).find((item) => item.resource_id === resourceId),
        });
      })
      .catch((caught: unknown) => {
        if (cancelled) return;
        setState({
          kind: 'error',
          detail: caught instanceof Error ? caught.message : String(caught),
        });
      });
    return () => {
      cancelled = true;
    };
  }, [resourceId]);

  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), 1500);
    return () => clearTimeout(timer);
  }, [copied]);

  if (!resourceId) {
    return (
      <div className="px-(--page-gutter) py-(--density-section-gap)">
        <p role="alert">{m.library_detail_missing_id()}</p>
      </div>
    );
  }

  const entry = state.kind === 'ready' ? state.entry : undefined;
  const yes = m.library_value_yes();
  const no = m.library_value_no();
  const none = m.library_value_none();

  const fields: { label: string; value: string }[] = entry
    ? [
        {
          label: m.library_field_progress(),
          value: entry.progress
            ? entry.progress.total
              ? `${entry.progress.position} / ${entry.progress.total}`
              : String(entry.progress.position)
            : none,
        },
        { label: m.library_field_pinned(), value: entry.pinned ? yes : no },
        { label: m.library_field_favorite(), value: entry.favorite ? yes : no },
        { label: m.library_field_opened(), value: entry.last_opened_at ?? none },
        { label: m.library_field_revision(), value: String(entry.revision) },
      ]
    : [];

  return (
    <>
      <PageToolbar
        actions={
          <Button variant="ghost" size="sm" render={<Link to="/library" />}>
            <Icon name="arrow-left" className="text-base" />
            {m.library_detail_back()}
          </Button>
        }
      />

      <div className="max-w-(--measure-detail) px-(--page-gutter) py-(--density-section-gap)">
        <div className="flex items-center gap-2 border-b border-hairline pb-2">
          <span className="min-w-0 flex-1 truncate font-mono text-ui-lg">{resourceId}</span>
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={m.action_copy()}
            title={copied ? m.action_copied() : m.action_copy()}
            onClick={() => {
              void navigator.clipboard.writeText(resourceId).then(() => setCopied(true));
            }}
          >
            <Icon name={copied ? 'check' : 'copy'} className="text-base" />
          </Button>
        </div>

        {state.kind === 'loading' ? (
          <div className="mt-3 flex flex-col gap-px">
            {[0, 1, 2, 3, 4].map((index) => (
              <Skeleton key={index} className="h-(--density-row) w-full" />
            ))}
          </div>
        ) : null}

        {state.kind === 'error' ? (
          <p role="alert" className="mt-3 font-mono text-ui-sm break-words text-ink-muted">
            {m.library_local_state_error({ detail: state.detail })}
          </p>
        ) : null}

        {state.kind === 'ready' && !entry ? (
          <Empty className="mt-3 border border-dashed border-hairline">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <Icon name="books" className="text-base" />
              </EmptyMedia>
              <EmptyTitle>{m.library_detail_not_found()}</EmptyTitle>
            </EmptyHeader>
          </Empty>
        ) : null}

        {entry ? (
          <dl className="mt-3 divide-y divide-hairline border-b border-hairline">
            {fields.map((field) => (
              <div
                key={field.label}
                className="flex h-(--density-row) items-center justify-between gap-4"
              >
                <dt className="shrink-0 text-ink-muted">{field.label}</dt>
                <dd className="truncate font-mono tabular-nums">{field.value}</dd>
              </div>
            ))}
          </dl>
        ) : null}
      </div>
    </>
  );
}
