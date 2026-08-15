import { isTauri } from '@tauri-apps/api/core';
import {
  ArrowRight,
  CheckCircle2,
  Code2,
  Download,
  FileCode2,
  RefreshCw,
  ShieldAlert,
} from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { Badge, Button, Card, CardContent, Input } from '@/components/ui';
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

  const load = async () => {
    setStatus('loading');
    setError('');
    try {
      setSources(await listInstalledSources());
      setStatus('ready');
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
      setStatus('error');
    }
  };

  useEffect(() => {
    void load();
  }, []);

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
      return;
    }
    setImportPhase('done');
    setPrepared([]);
    setSelectedIds([]);
    setSearchParams((current) => {
      current.delete('import');
      return current;
    });
    if (successCount === 0) setImportPhase('error');
  };

  return (
    <div className="mx-auto max-w-7xl px-5 py-8 sm:px-8 lg:px-12 lg:py-12">
      <div className="border-b border-(--border) pb-8">
        <p className="eyebrow">{m.sources_title()}</p>
        <h1 className="font-display mt-2 text-4xl font-semibold text-balance">
          {m.sources_deeplink_import_title()}
        </h1>
        <p className="mt-3 max-w-xl text-(--muted-text)">{m.sources_deeplink_import_hint()}</p>
      </div>

      {pendingImport && (
        <Card className="mt-6 border-(--accent)/40 bg-(--surface-2)">
          <CardContent className="pt-5">
            <div className="flex items-start gap-3">
              <Download
                className="mt-0.5 shrink-0 text-(--accent-strong)"
                size={19}
                aria-hidden="true"
              />
              <div className="min-w-0 flex-1">
                <h2 className="font-medium">{m.sources_deeplink_import_title()}</h2>
                <p className="mt-1 text-sm break-words text-(--muted-text)">{pendingImport}</p>
              </div>
              <Button
                variant="ghost"
                size="sm"
                onClick={() =>
                  setSearchParams((current) => {
                    current.delete('import');
                    return current;
                  })
                }
              >
                {m.action_close()}
              </Button>
            </div>

            {importPhase === 'loading' && (
              <p className="mt-4 text-sm">{m.sources_deeplink_fetching()}</p>
            )}
            {importPhase === 'error' && (
              <p className="mt-4 text-sm break-words text-(--danger)" role="alert">
                {importError}
              </p>
            )}
            {importPhase === 'pick' && (
              <div className="mt-4">
                <p className="text-sm text-(--muted-text)">
                  {m.sources_deeplink_install_cap({
                    selected: selectedItems.length,
                    cap: CATALOG_INSTALL_CAP,
                  })}
                </p>
                <ul className="mt-3 max-h-64 divide-y divide-(--border) overflow-y-auto border border-(--border)">
                  {catalog.map((item) => (
                    <li key={item.id}>
                      <label className="flex items-center gap-3 px-3 py-2 hover:bg-(--surface)">
                        <Input
                          type="checkbox"
                          checked={selectedIds.includes(item.id)}
                          onChange={() => toggleSelected(item.id)}
                          aria-label={item.name}
                          className="h-4 w-4"
                        />
                        <span className="min-w-0 flex-1 truncate text-sm">{item.name}</span>
                        {item.group && <Badge>{item.group}</Badge>}
                      </label>
                    </li>
                  ))}
                </ul>
                <Button
                  className="mt-4"
                  disabled={
                    selectedItems.length === 0 || selectedItems.length > CATALOG_INSTALL_CAP
                  }
                  onClick={() => void prepareSelected()}
                >
                  {m.sources_deeplink_install_selected()}
                </Button>
              </div>
            )}
            {importPhase === 'confirm' && (
              <div className="mt-4 space-y-4">
                <div>
                  <h3 className="font-medium">{m.sources_deeplink_grant_title()}</h3>
                  <ul className="mt-2 text-sm text-(--muted-text)">
                    {prepared.map((entry) => (
                      <li key={entry.item.id}>{entry.item.name}</li>
                    ))}
                  </ul>
                </div>
                {requestsSystem && (
                  <div
                    className="flex gap-2 border border-(--danger) bg-(--danger-soft) p-3 text-sm text-(--danger)"
                    role="alert"
                  >
                    <ShieldAlert size={18} aria-hidden="true" />
                    {m.sources_install_system_unsupported()}
                  </div>
                )}
                {requestsNetwork && !requestsSystem && (
                  <label htmlFor="network-grant" className="flex items-start gap-2 text-sm">
                    <Input
                      id="network-grant"
                      type="checkbox"
                      onChange={(event) => setAllowNetwork(event.target.checked)}
                      className="mt-0.5 h-4 w-4"
                    />
                    {m.sources_install_grant_network_only()}
                  </label>
                )}
                {importError && (
                  <p className="text-sm whitespace-pre-wrap text-(--danger)" role="alert">
                    {importError}
                  </p>
                )}
                <div className="flex gap-2">
                  <Button variant="outline" onClick={() => setImportPhase('pick')}>
                    {m.sources_deeplink_back_to_pick()}
                  </Button>
                  <Button
                    disabled={requestsSystem || (requestsNetwork && !allowNetwork)}
                    onClick={() => void installSelected()}
                  >
                    {m.sources_deeplink_install_selected()}
                  </Button>
                </div>
              </div>
            )}
            {importPhase === 'installing' && (
              <p className="mt-4 text-sm" role="status">
                {m.sources_install_installing()}
              </p>
            )}
            {importPhase === 'done' && (
              <p className="mt-4 text-sm text-(--accent-strong)" role="status">
                {m.sources_deeplink_done_title()}
              </p>
            )}
          </CardContent>
        </Card>
      )}

      <section className="mt-8">
        <div className="flex items-center justify-between gap-4">
          <div>
            <h2 className="font-display text-2xl font-semibold">{m.sources_installed_title()}</h2>
            <p className="mt-1 text-sm text-(--muted-text)">
              {isTauri() ? m.sources_device_projection() : m.sources_browser_projection()}
            </p>
          </div>
          <Button variant="outline" onClick={() => void load()} disabled={status === 'loading'}>
            <RefreshCw size={16} aria-hidden="true" />
            {status === 'loading' ? m.sources_loading() : m.library_refresh()}
          </Button>
        </div>

        {status === 'error' && (
          <p className="mt-5 text-sm break-words text-(--danger)" role="alert">
            {m.sources_load_error()} {error}
          </p>
        )}
        {status === 'ready' && sources.length === 0 && (
          <p className="mt-5 border border-dashed border-(--border) p-8 text-center text-sm text-(--muted-text)">
            {m.sources_empty_title()}
          </p>
        )}

        <div className="mt-5 grid gap-3">
          {sources.map((source) => (
            <Card
              key={source.source_id}
              className={
                highlightedSourceId === source.source_id
                  ? 'border-(--accent) ring-2 ring-(--ring)/30'
                  : undefined
              }
            >
              <CardContent className="flex items-start gap-4 pt-5">
                <span className="grid h-10 w-10 shrink-0 place-items-center rounded-md bg-(--surface-2) text-(--accent-strong)">
                  <CheckCircle2 size={19} aria-hidden="true" />
                </span>
                <div className="min-w-0 flex-1">
                  <p className="font-medium break-words">{source.profile.title}</p>
                  <p className="mt-1 text-xs break-all text-(--muted-text)">{source.source_id}</p>
                  <div className="mt-3 flex flex-wrap gap-2">
                    {source.profile.supported_intents.map((intent) => (
                      <Badge key={intent}>{intent}</Badge>
                    ))}
                  </div>
                </div>
                <Badge>{source.version}</Badge>
              </CardContent>
            </Card>
          ))}
        </div>
      </section>

      <div className="mt-8 border-t border-(--border) pt-5">
        <Link
          to="/sources/rules"
          className="inline-flex items-center gap-2 text-sm text-(--accent-strong) hover:underline focus-visible:ring-2 focus-visible:ring-(--ring) focus-visible:outline-none"
        >
          <Code2 size={15} aria-hidden="true" />
          {m.rules_workspace_title()} <ArrowRight size={14} aria-hidden="true" />
        </Link>
      </div>
    </div>
  );
}

export function RulesMigration() {
  const m = useMessages();
  return (
    <div className="mx-auto max-w-5xl px-5 py-8 sm:px-8 lg:px-12 lg:py-12">
      <Link to="/sources" className="text-sm text-(--accent-strong) hover:underline">
        ← {m.sources_title()}
      </Link>
      <div className="mt-8 border-b border-(--border) pb-8">
        <p className="eyebrow">{m.sources_rules_migration_eyebrow()}</p>
        <h1 className="font-display mt-2 text-4xl font-semibold">
          {m.sources_rules_migration_title()}
        </h1>
        <p className="mt-3 max-w-2xl leading-7 text-(--muted-text)">
          {m.sources_rules_migration_description()}
        </p>
      </div>
      <Card className="mt-8">
        <CardContent className="flex items-start gap-4 pt-5">
          <FileCode2 className="mt-0.5 shrink-0 text-(--accent-strong)" aria-hidden="true" />
          <div>
            <h2 className="font-medium">{m.sources_rules_migration_boundary_title()}</h2>
            <p className="mt-1 text-sm leading-6 text-(--muted-text)">
              {m.sources_rules_migration_boundary_description()}
            </p>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
