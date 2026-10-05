import { isTauri } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { Icon } from '@/components/Icon';
import { PageToolbar } from '@/components/PageToolbar';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
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
import { listInstalledSources, type InstalledSource } from '@/shared/tauri/sources';
import { SourceImportDialog } from './SourceImportDialog';
import { SourceInstallDialog } from './SourceInstallDialog';
import { SourceInspector } from './SourceInspector';

export function SourcesHome() {
  const m = useMessages();
  const [searchParams] = useSearchParams();
  const [sources, setSources] = useState<InstalledSource[]>([]);
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading');
  const [error, setError] = useState('');
  const [installOpen, setInstallOpen] = useState(false);
  const [inspectedSource, setInspectedSource] = useState<InstalledSource>();
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const highlightedSourceId = searchParams.get('highlight');

  const load = useCallback(async () => {
    setStatus('loading');
    setError('');
    try {
      setSources(await listInstalledSources());
      setStatus('ready');
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
      setStatus('error');
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <>
      <PageToolbar
        meta={m.sources_group_count({ count: sources.length })}
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => setInstallOpen(true)}>
              <Icon name="plus" />
              {m.sources_import_open()}
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={m.sources_refresh()}
              title={m.sources_refresh()}
              disabled={status === 'loading'}
              onClick={() => void load()}
            >
              <Icon name="arrow-clockwise" className="text-base" />
            </Button>
          </>
        }
      />

      <div className="px-(--page-gutter) py-(--density-section-gap)">
        <p className="mb-2 text-ui-sm text-ink-subtle">
          {isTauri() ? m.sources_device_projection() : m.sources_browser_projection()}
        </p>

        {status === 'loading' ? (
          <div className="flex flex-col gap-px">
            {[0, 1, 2].map((index) => (
              <Skeleton key={index} className="h-(--density-row) w-full" />
            ))}
          </div>
        ) : null}

        {status === 'error' ? (
          <div role="alert" className="border border-hairline p-3">
            <p className="font-medium">{m.sources_load_error()}</p>
            <p className="mt-1 font-mono text-ui-sm wrap-break-word text-ink-muted">{error}</p>
            <Button variant="outline" size="sm" className="mt-3" onClick={() => void load()}>
              {m.action_retry()}
            </Button>
          </div>
        ) : null}

        {status === 'ready' && sources.length === 0 ? (
          <Empty className="border border-dashed border-hairline">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <Icon name="broadcast" className="text-base" />
              </EmptyMedia>
              <EmptyTitle>{m.sources_empty_title()}</EmptyTitle>
              <EmptyDescription>{m.sources_empty_hint()}</EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : null}

        {status === 'ready' && sources.length > 0 ? (
          <ul className="divide-y divide-hairline border-y border-hairline">
            {sources.map((source) => (
              <li key={source.source_id}>
                <button
                  type="button"
                  aria-label={source.profile.title}
                  className="block w-full text-left focus-visible:outline-1 focus-visible:outline-ring"
                  onClick={() => {
                    setInspectedSource(source);
                    setInspectorOpen(true);
                  }}
                >
                  <Item
                    size="xs"
                    className={
                      highlightedSourceId === source.source_id
                        ? 'bg-lantern-soft ring-1 ring-lantern-strong/40'
                        : undefined
                    }
                  >
                    <ItemMedia variant="icon">
                      <Icon name="check-circle" className="text-base text-positive" />
                    </ItemMedia>
                    <ItemContent className="min-w-0">
                      <ItemTitle className="truncate">{source.profile.title}</ItemTitle>
                    </ItemContent>
                    <span className="hidden min-w-0 flex-1 truncate font-mono text-ui-sm text-ink-subtle sm:block">
                      {source.source_id}
                    </span>
                    <span className="hidden shrink-0 gap-1 md:flex">
                      {source.profile.supported_intents.map((intent) => (
                        <Badge key={intent} variant="outline">
                          {intent}
                        </Badge>
                      ))}
                    </span>
                    <span className="shrink-0 font-mono text-ui-sm text-ink-muted">
                      {source.version}
                    </span>
                  </Item>
                </button>
              </li>
            ))}
          </ul>
        ) : null}
      </div>

      <SourceImportDialog onInstalled={() => void load()} />
      <SourceInstallDialog
        open={installOpen}
        onOpenChange={setInstallOpen}
        onInstalled={() => void load()}
      />
      <SourceInspector
        open={inspectorOpen}
        source={inspectedSource}
        onOpenChange={setInspectorOpen}
        onRequestUpdate={() => {
          setInspectorOpen(false);
          setInstallOpen(true);
        }}
        onInstalled={() => void load()}
      />
    </>
  );
}
