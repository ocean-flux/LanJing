import { Icon } from '@/components/Icon';
import { Button } from '@/components/ui/button';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { Separator } from '@/components/ui/separator';
import { useMessages } from '@/shared/i18n/messages';
import type { LibraryWorkflowRow } from './workflow';

interface LibraryInspectorProps {
  open: boolean;
  row: LibraryWorkflowRow | undefined;
  pending: boolean;
  failure: string | undefined;
  onOpenChange: (open: boolean) => void;
  onFavoriteChange: () => void;
  onPinnedChange: () => void;
}

function progressValue(row: LibraryWorkflowRow, empty: string): string {
  if (!row.entry.progress) return empty;
  if (row.entry.progress.total === null) return String(row.entry.progress.position);
  return `${row.entry.progress.position} / ${row.entry.progress.total}`;
}

export function LibraryInspector({
  open,
  row,
  pending,
  failure,
  onOpenChange,
  onFavoriteChange,
  onPinnedChange,
}: LibraryInspectorProps) {
  const m = useMessages();

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="w-full sm:max-w-md">
        <SheetHeader>
          <SheetTitle>{m.library_inspector_title()}</SheetTitle>
          <SheetDescription className="truncate font-mono">
            {row?.entry.resource_id ?? m.library_value_none()}
          </SheetDescription>
        </SheetHeader>

        <div className="flex-1 overflow-y-auto px-4">
          {(() => {
            if (!row) {
              return (
                <div className="flex flex-col gap-px">
                  {[0, 1, 2, 3].map((index) => (
                    <div key={index} className="h-(--density-row) animate-pulse bg-surface-2" />
                  ))}
                </div>
              );
            }
            return (
              <>
                <section aria-labelledby="library-inspector-media" className="py-3">
                  <h3 id="library-inspector-media" className="mb-2 font-medium">
                    {m.library_inspector_media()}
                  </h3>
                  {row.media ? (
                    <div className="space-y-1">
                      <p className="font-heading text-ui-lg">{row.media.title}</p>
                      <p className="text-ink-muted">
                        {m.library_media_creator()}: {row.media.creator}
                      </p>
                      <p className="text-ink-muted">
                        {m.library_media_kind()}: {row.media.kind}
                      </p>
                      <p className="text-ink-muted">
                        {m.library_inspector_source()}: {row.media.source ?? m.library_value_none()}
                      </p>
                    </div>
                  ) : (
                    <p className="text-ink-muted">{m.library_media_missing()}</p>
                  )}
                </section>

                <Separator />

                <section aria-labelledby="library-inspector-local-state" className="py-3">
                  <h3 id="library-inspector-local-state" className="mb-2 font-medium">
                    {m.library_inspector_local_state()}
                  </h3>
                  <dl className="divide-y divide-hairline border-y border-hairline">
                    <div className="flex min-h-(--density-row) items-center justify-between gap-4">
                      <dt className="text-ink-muted">{m.library_inspector_progress()}</dt>
                      <dd className="font-mono tabular-nums">
                        {progressValue(row, m.library_value_none())}
                      </dd>
                    </div>
                    <div className="flex min-h-(--density-row) items-center justify-between gap-4">
                      <dt className="text-ink-muted">{m.library_field_opened()}</dt>
                      <dd className="max-w-[65%] truncate font-mono">
                        {row.entry.last_opened_at ?? m.library_value_none()}
                      </dd>
                    </div>
                    <div className="flex min-h-(--density-row) items-center justify-between gap-4">
                      <dt className="text-ink-muted">{m.library_field_revision()}</dt>
                      <dd className="font-mono tabular-nums">{row.entry.revision}</dd>
                    </div>
                  </dl>
                </section>

                <Separator />

                <section className="py-3" aria-label={m.library_inspector_title()}>
                  <div className="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={
                        row.entry.favorite ? m.library_unfavorite() : m.library_favorite()
                      }
                      title={row.entry.favorite ? m.library_unfavorite() : m.library_favorite()}
                      disabled={pending}
                      onClick={onFavoriteChange}
                    >
                      <Icon
                        name={row.entry.favorite ? 'star-fill' : 'star'}
                        className="text-base"
                      />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={row.entry.pinned ? m.library_unpin() : m.library_pin()}
                      title={row.entry.pinned ? m.library_unpin() : m.library_pin()}
                      disabled={pending}
                      onClick={onPinnedChange}
                    >
                      <Icon
                        name={row.entry.pinned ? 'push-pin-fill' : 'push-pin'}
                        className="text-base"
                      />
                    </Button>
                    {pending ? (
                      <Icon name="spinner" className="ml-1 animate-spin text-base" />
                    ) : null}
                  </div>
                  {failure ? (
                    <p
                      role="alert"
                      className="mt-2 font-mono text-ui-sm wrap-break-word text-danger"
                    >
                      {m.library_update_error({ detail: failure })}
                    </p>
                  ) : null}
                </section>
              </>
            );
          })()}
        </div>

        <SheetFooter />
      </SheetContent>
    </Sheet>
  );
}
