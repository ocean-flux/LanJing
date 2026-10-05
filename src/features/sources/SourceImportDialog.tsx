import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { toast } from 'sonner';
import { Icon } from '@/components/Icon';
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
import { Label } from '@/components/ui/label';
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
  prepareSourceInstall,
  type InstallCandidate,
} from '@/shared/tauri/sources';
import { NETWORK_CONSENT_REQUIRED, deepLinkImportStep } from './workflow';

type ImportPhase =
  | 'idle'
  | 'loading'
  | 'consent'
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

interface SourceImportDialogProps {
  onInstalled: () => void;
}

/**
 * 深链来源导入弹窗（`?import=<url>`）。
 *
 * 抓取 `import` 链接前先过用户可见的「允许联网」开关: 开关为关时不发请求, 只显示同意步骤与
 * 开关本身; 待导入链接留在查询参数上, 用户打开开关后同一步继续。抓取成功后按 catalog 勾选、
 * 逐个 prepare 并提交 install。
 */
export function SourceImportDialog({ onInstalled }: SourceImportDialogProps) {
  const m = useMessages();
  const [searchParams, setSearchParams] = useSearchParams();
  const [importPhase, setImportPhase] = useState<ImportPhase>('idle');
  const [importError, setImportError] = useState('');
  const [catalog, setCatalog] = useState<CatalogItem[]>([]);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [prepared, setPrepared] = useState<PreparedSource[]>([]);
  const [allowNetwork, setAllowNetwork] = useState(false);
  const pendingImport = searchParams.get('import');
  /** 已发起抓取的导入链接; 防止安装授权开关的变化重跑抓取。 */
  const importedUrlRef = useRef<string | null>(null);

  useEffect(() => {
    if (!pendingImport) {
      importedUrlRef.current = null;
      setImportPhase('idle');
      return;
    }
    // 同一个导入链接只抓一次: 之后的开关变化只影响安装授权, 不重新抓取。
    if (importedUrlRef.current === pendingImport) return;
    if (deepLinkImportStep(allowNetwork) === NETWORK_CONSENT_REQUIRED) {
      // 联网开关为关: 不发出请求, 先请用户授权; 待导入链接仍在 ?import= 上, 打开开关后同一步继续。
      setImportError('');
      setImportPhase('consent');
      return;
    }
    importedUrlRef.current = pendingImport;
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
  }, [allowNetwork, m, pendingImport]);

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
    onInstalled();
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
      importPhase === 'consent' ||
      importPhase === 'pick' ||
      importPhase === 'preparing' ||
      importPhase === 'confirm' ||
      importPhase === 'installing' ||
      importPhase === 'error');

  return (
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

        {importPhase === 'consent' ? (
          <>
            <p className="text-ink-muted">{m.sources_deeplink_network_consent_notice()}</p>
            <Label className="flex items-center gap-2">
              <Checkbox
                checked={allowNetwork}
                onCheckedChange={(checked) => setAllowNetwork(checked === true)}
              />
              {m.sources_deeplink_network_consent_switch()}
            </Label>
            <DialogFooter>
              <Button variant="outline" onClick={closeImport}>
                {m.action_cancel()}
              </Button>
            </DialogFooter>
          </>
        ) : null}

        {importPhase === 'error' ? (
          <p role="alert" className="wrap-break-word text-danger">
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
              <p role="alert" className="wrap-break-word text-danger">
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
                <li
                  key={entry.item.id}
                  className="flex h-(--density-row) items-center gap-2 truncate"
                >
                  <span className="min-w-0 flex-1 truncate">{entry.item.name}</span>
                  {entry.candidate.operation === 'update' ? (
                    <Badge variant="default">{m.sources_install_update()}</Badge>
                  ) : null}
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
  );
}
