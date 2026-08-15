import { isTauri } from '@tauri-apps/api/core';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { toast } from 'sonner';
import { Icon } from '@/components/Icon';
import { PageToolbar } from '@/components/PageToolbar';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import { Item, ItemContent, ItemMedia, ItemTitle } from '@/components/ui/item';
import { Label } from '@/components/ui/label';
import { Skeleton } from '@/components/ui/skeleton';
import { Spinner } from '@/components/ui/spinner';
import { useMessages } from '@/shared/i18n/messages';
import {
  CATALOG_INSTALL_CAP,
  parseBookSourceCatalog,
  type CatalogItem,
} from '@/shared/tauri/catalog';
import {
  fetchImportSource,
  installPreparedSource,
  listInstalledSources,
  prepareSourceInstall,
  type InstallCandidate,
  type InstalledSource,
} from '@/shared/tauri/sources';

type ImportPhase =
  | 'idle'
  | 'loading'
  | 'pick'
  | 'preparing'
  | 'confirm'
  | 'installing'
  | 'done'
  | 'error';

type PreparedSource = { candidate: InstallCandidate; item: CatalogItem };

function candidateRequestsSystem(candidate: InstallCandidate) {
  const { env, fs, process } = candidate.required_grant.system;
  return env || fs || process;
}

export function SourcesHome() {
  const m = useMessages();
  const [searchParams, setSearchParams] = useSearchParams();
  const [sources, setSources] = useState<InstalledSource[]>([]);
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading');
  const [error, setError] = useState('');
  const [importPhase, setImportPhase] = useState<ImportPhase>('idle');
  const [importError, setImportError] = useState('');
  const [catalog, setCatalog] = useState<CatalogItem[]>([]);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [prepared, setPrepared] = useState<PreparedSource[]>([]);
  const [allowNetwork, setAllowNetwork] = useState(false);
  const highlightedSourceId = searchParams.get('highlight');
  const pendingImport = searchParams.get('import');

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

  useEffect(() => {
    if (!pendingImport) {
      setImportPhase('idle');
      return;
    }
    let cancelled = false;
    const loadImport = async () => {
      setImportPhase('loading');
      setImportError('');
      setCatalog([]);
      setSelectedIds([]);
      setPrepared([]);
      try {
        const result = parseBookSourceCatalog(await fetchImportSource(pendingImport));
        if (cancelled) return;
        if (!result.ok) {
          setImportPhase('error');
          setImportError(`${m.sources_deeplink_error_title()}: ${result.reason}`);
          return;
        }
        setCatalog(result.items);
        setImportPhase('pick');
      } catch (caught) {
        if (cancelled) return;
        setImportPhase('error');
        setImportError(caught instanceof Error ? caught.message : String(caught));
      }
    };
    void loadImport();
    return () => {
      cancelled = true;
    };
  }, [m, pendingImport]);

  const selectedItems = useMemo(
    () => catalog.filter((item) => selectedIds.includes(item.id)),
    [catalog, selectedIds],
  );
  const requestsSystem = prepared.some((entry) => candidateRequestsSystem(entry.candidate));
  const requestsNetwork = prepared.some((entry) => entry.candidate.required_grant.network);

  const closeImport = useCallback(() => {
    setSearchParams((current) => {
      current.delete('import');
      return current;
    });
  }, [setSearchParams]);

  const toggleSelected = (itemId: string) => {
    setSelectedIds((current) =>
      current.includes(itemId) ? current.filter((id) => id !== itemId) : [...current, itemId],
    );
  };

  const prepareSelected = async () => {
    if (selectedItems.length === 0 || selectedItems.length > CATALOG_INSTALL_CAP) return;
    setImportPhase('preparing');
    setImportError('');
    try {
      const next = await Promise.all(
        selectedItems.map(async (item) => ({
          candidate: await prepareSourceInstall(item.rawJson),
          item,
        })),
      );
      setPrepared(next);
      setAllowNetwork(false);
      setImportPhase('confirm');
    } catch (caught) {
      setImportError(caught instanceof Error ? caught.message : String(caught));
      setImportPhase('pick');
    }
  };

  const installSelected = async () => {
    if (requestsSystem || (requestsNetwork && !allowNetwork)) return;
    setImportPhase('installing');
    setImportError('');
    const results = await Promise.all(
      prepared.map(async (entry) => {
        try {
          await installPreparedSource(
            entry.candidate.id,
            requestsNetwork ? 'network_only' : 'none',
          );
          return { entry, error: undefined };
        } catch (caught) {
          return { entry, error: caught instanceof Error ? caught.message : String(caught) };
        }
      }),
    );
    const failures = results
      .filter((result) => result.error)
      .map((result) => `${result.entry.item.name}: ${result.error}`);
    const successCount = results.length - failures.length;
    await load();
    if (failures.length > 0) {
      setImportError(failures.join('\n'));
      setImportPhase('confirm');
      toast.error(m.sources_deeplink_install_partial({ ok: successCount, fail: failures.length }));
      return;
    }
    setImportPhase('done');
    setPrepared([]);
    setSelectedIds([]);
    closeImport();
    if (successCount === 0) {
      setImportPhase('error');
      return;
    }
    toast.success(m.sources_deeplink_install_success({ count: successCount }));
  };

  const dialogOpen =
    pendingImport !== null &&
    (importPhase === 'loading' ||
      importPhase === 'pick' ||
      importPhase === 'preparing' ||
      importPhase === 'confirm' ||
      importPhase === 'installing' ||
      importPhase === 'error');

  return (
    <>
      <PageToolbar
        meta={m.sources_group_count({ count: sources.length })}
        actions={
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
            <p className="mt-1 font-mono text-ui-sm break-words text-ink-muted">{error}</p>
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
              </li>
            ))}
          </ul>
        ) : null}
      </div>

      <Dialog
        open={dialogOpen}
        onOpenChange={(open) => {
          if (!open) closeImport();
        }}
      >
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>
              {importPhase === 'confirm'
                ? m.sources_deeplink_grant_title()
                : m.sources_deeplink_import_title()}
            </DialogTitle>
            <DialogDescription className="font-mono break-all">
              {pendingImport ?? ''}
            </DialogDescription>
          </DialogHeader>

          {importPhase === 'loading' ? (
            <p className="flex items-center gap-2" role="status">
              <Spinner className="animate-spin" />
              {m.sources_deeplink_fetching()}
            </p>
          ) : null}

          {importPhase === 'error' ? (
            <p role="alert" className="break-words text-danger">
              {importError}
            </p>
          ) : null}

          {importPhase === 'pick' || importPhase === 'preparing' ? (
            <>
              <p className="text-ink-muted">
                {m.sources_deeplink_install_cap({
                  selected: selectedItems.length,
                  cap: CATALOG_INSTALL_CAP,
                })}
              </p>
              <ul className="app-scroll-region max-h-64 divide-y divide-hairline border border-hairline">
                {catalog.map((item) => (
                  <li key={item.id}>
                    <Label className="flex h-(--density-row) items-center gap-2 px-2 hover:bg-surface-2">
                      <Checkbox
                        checked={selectedIds.includes(item.id)}
                        onCheckedChange={() => toggleSelected(item.id)}
                      />
                      <span className="min-w-0 flex-1 truncate">{item.name}</span>
                      {item.group ? <Badge variant="outline">{item.group}</Badge> : null}
                    </Label>
                  </li>
                ))}
              </ul>
              {importError ? (
                <p role="alert" className="break-words text-danger">
                  {importError}
                </p>
              ) : null}
              <DialogFooter>
                <Button variant="outline" onClick={closeImport}>
                  {m.action_cancel()}
                </Button>
                <Button
                  disabled={
                    importPhase === 'preparing' ||
                    selectedItems.length === 0 ||
                    selectedItems.length > CATALOG_INSTALL_CAP
                  }
                  onClick={() => void prepareSelected()}
                >
                  {m.sources_deeplink_install_selected()}
                </Button>
              </DialogFooter>
            </>
          ) : null}

          {importPhase === 'confirm' || importPhase === 'installing' ? (
            <>
              <ul className="divide-y divide-hairline border-y border-hairline">
                {prepared.map((entry) => (
                  <li key={entry.item.id} className="flex h-(--density-row) items-center truncate">
                    {entry.item.name}
                  </li>
                ))}
              </ul>

              {requestsSystem ? (
                <p
                  role="alert"
                  className="flex items-start gap-2 border border-danger bg-danger-soft p-2 text-danger"
                >
                  <Icon name="shield-check" className="mt-0.5 text-base" />
                  {m.sources_install_system_unsupported()}
                </p>
              ) : null}

              {requestsNetwork && !requestsSystem ? (
                <>
                  <p className="text-ink-muted">{m.sources_deeplink_network_required_notice()}</p>
                  <Label className="flex items-center gap-2">
                    <Checkbox
                      checked={allowNetwork}
                      onCheckedChange={(checked) => setAllowNetwork(checked === true)}
                    />
                    {m.sources_install_grant_network_only()}
                  </Label>
                </>
              ) : null}

              {importError ? (
                <p role="alert" className="whitespace-pre-wrap text-danger">
                  {importError}
                </p>
              ) : null}

              <DialogFooter>
                <Button
                  variant="outline"
                  disabled={importPhase === 'installing'}
                  onClick={() => setImportPhase('pick')}
                >
                  {m.sources_deeplink_back_to_pick()}
                </Button>
                <Button
                  disabled={
                    importPhase === 'installing' ||
                    requestsSystem ||
                    (requestsNetwork && !allowNetwork)
                  }
                  onClick={() => void installSelected()}
                >
                  {importPhase === 'installing'
                    ? m.sources_install_installing()
                    : m.sources_install_action()}
                </Button>
              </DialogFooter>
            </>
          ) : null}
        </DialogContent>
      </Dialog>
    </>
  );
}
