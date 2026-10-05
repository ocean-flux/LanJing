import { useEffect, useMemo, useState } from 'react';
import { toast } from 'sonner';
import { Icon } from '@/components/Icon';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { Separator } from '@/components/ui/separator';
import { Spinner } from '@/components/ui/spinner';
import { useMessages } from '@/shared/i18n/messages';
import {
  installPreparedSource,
  listSourceRevisions,
  prepareSourceRollback,
  type CapabilityGrantPreset,
  type InstallCandidate,
  type InstalledSource,
  type SourceRevision,
} from '@/shared/tauri/sources';
import { candidateRequestsNetwork, candidateRequestsSystem } from './workflow';

export interface SourceInspectorAdapter {
  listRevisions: (sourceId: string) => Promise<SourceRevision[]>;
  prepareRollback: (sourceId: string, revision: number) => Promise<InstallCandidate>;
  install: (candidateId: string, grant: CapabilityGrantPreset) => Promise<InstalledSource>;
}

const tauriSourceInspectorAdapter: SourceInspectorAdapter = {
  listRevisions: listSourceRevisions,
  prepareRollback: prepareSourceRollback,
  install: installPreparedSource,
};

interface SourceInspectorProps {
  open: boolean;
  source: InstalledSource | undefined;
  onOpenChange: (open: boolean) => void;
  onRequestUpdate: () => void;
  onInstalled: () => void;
  adapter?: SourceInspectorAdapter;
}

type RollbackPhase = 'idle' | 'preparing' | 'confirm' | 'installing' | 'error';

function formatTimestamp(value: number): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(value));
}

function grantLabel(value: boolean, m: ReturnType<typeof useMessages>): string {
  return value ? m.sources_install_network_required() : m.sources_install_network_not_required();
}

function systemGrantLabel(
  grant: InstalledSource['grant'],
  m: ReturnType<typeof useMessages>,
): string {
  const values = Object.entries(grant.system)
    .filter(([, enabled]) => enabled)
    .map(([name]) => {
      switch (name) {
        case 'env': {
          return m.sources_capability_env();
        }
        case 'fs': {
          return m.sources_capability_fs();
        }
        case 'process': {
          return m.sources_capability_process();
        }
        default: {
          return name;
        }
      }
    });
  return values.length > 0 ? values.join(', ') : m.sources_value_none();
}

function errorCode(error: unknown): string {
  if (typeof error === 'object' && error !== null && 'code' in error) {
    const { code } = error as { code?: unknown };
    if (typeof code === 'string' && code.length > 0) return code;
  }
  return 'source_operation_failed';
}

function errorDetail(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const { message } = error as { message?: unknown };
    if (typeof message === 'string') return message;
  }
  return String(error);
}

function rollbackErrorLabel(code: string | null, m: ReturnType<typeof useMessages>): string | null {
  switch (code) {
    case 'candidate_stale': {
      return m.sources_install_candidate_stale();
    }
    case 'candidate_expired': {
      return m.sources_install_candidate_expired();
    }
    case 'candidate_consumed': {
      return m.sources_install_candidate_consumed();
    }
    case 'source_revision_not_found': {
      return m.sources_error_source_revision_not_found();
    }
    case 'grant_insufficient': {
      return m.sources_install_network_required_notice();
    }
    case 'system_grant_unsupported': {
      return m.sources_install_system_unsupported();
    }
    case 'source_operation_failed': {
      return m.sources_inspector_rollback_failed();
    }
    default: {
      return code ? m.sources_inspector_rollback_failed() : null;
    }
  }
}

export function SourceInspector({
  open,
  source,
  onOpenChange,
  onRequestUpdate,
  onInstalled,
  adapter = tauriSourceInspectorAdapter,
}: SourceInspectorProps) {
  const m = useMessages();
  const [history, setHistory] = useState<SourceRevision[]>([]);
  const [historyState, setHistoryState] = useState<'idle' | 'loading' | 'ready' | 'error'>('idle');
  const [historyError, setHistoryError] = useState('');
  const [rollbackTarget, setRollbackTarget] = useState<SourceRevision | null>(null);
  const [rollbackCandidate, setRollbackCandidate] = useState<InstallCandidate | null>(null);
  const [rollbackPhase, setRollbackPhase] = useState<RollbackPhase>('idle');
  const [rollbackError, setRollbackError] = useState<string | null>(null);
  const [rollbackDetail, setRollbackDetail] = useState('');

  useEffect(() => {
    if (!open || !source) return;
    let cancelled = false;
    setHistoryState('loading');
    setHistoryError('');
    setHistory([]);
    setRollbackTarget(null);
    setRollbackCandidate(null);
    setRollbackPhase('idle');
    setRollbackError(null);
    setRollbackDetail('');
    const loadHistory = async () => {
      try {
        const next = await adapter.listRevisions(source.source_id);
        if (cancelled) return;
        setHistory(next);
        setHistoryState('ready');
      } catch (error) {
        if (cancelled) return;
        setHistoryState('error');
        setHistoryError(errorDetail(error));
      }
    };
    void loadHistory();
    return () => {
      cancelled = true;
    };
  }, [adapter, open, source]);

  const currentSystemGrant = useMemo(
    () => (source ? systemGrantLabel(source.grant, m) : m.sources_value_none()),
    [m, source],
  );
  const candidateNeedsSystem = rollbackCandidate
    ? candidateRequestsSystem(rollbackCandidate)
    : false;
  const candidateNeedsNetwork = rollbackCandidate
    ? candidateRequestsNetwork(rollbackCandidate)
    : false;
  const rollbackErrorLabelText = rollbackErrorLabel(rollbackError, m);

  const prepareRollback = async (revision: SourceRevision) => {
    if (!source || revision.revision === source.revision) return;
    setRollbackTarget(revision);
    setRollbackCandidate(null);
    setRollbackError(null);
    setRollbackDetail('');
    setRollbackPhase('preparing');
    try {
      const candidate = await adapter.prepareRollback(source.source_id, revision.revision);
      setRollbackCandidate(candidate);
      setRollbackPhase('confirm');
    } catch (error) {
      setRollbackError(errorCode(error));
      setRollbackDetail(errorDetail(error));
      setRollbackPhase('error');
    }
  };

  const installRollback = async () => {
    if (!rollbackCandidate) return;
    if (candidateNeedsSystem) {
      setRollbackError('system_grant_unsupported');
      setRollbackDetail('');
      setRollbackPhase('error');
      return;
    }
    setRollbackPhase('installing');
    setRollbackError(null);
    setRollbackDetail('');
    try {
      await adapter.install(rollbackCandidate.id, candidateNeedsNetwork ? 'network_only' : 'none');
      toast.success(m.sources_inspector_rollback_success());
      onInstalled();
      onOpenChange(false);
    } catch (error) {
      setRollbackError(errorCode(error));
      setRollbackDetail(errorDetail(error));
      setRollbackPhase('error');
    }
  };

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="w-full overflow-hidden sm:max-w-lg">
        <SheetHeader>
          <SheetTitle>{m.sources_inspector_title()}</SheetTitle>
          <SheetDescription className="truncate font-mono">
            {source?.source_id ?? m.sources_value_none()}
          </SheetDescription>
        </SheetHeader>

        <div className="flex-1 overflow-y-auto px-4">
          {source === undefined ? (
            <div className="flex items-center gap-2 py-4" role="status">
              <Spinner className="animate-spin" />
              {m.sources_loading()}
            </div>
          ) : (
            <>
              <section aria-labelledby="source-inspector-current" className="py-3">
                <div className="mb-2 flex items-center justify-between gap-2">
                  <h3 id="source-inspector-current" className="font-medium">
                    {m.sources_inspector_current()}
                  </h3>
                  <Button variant="outline" size="sm" onClick={onRequestUpdate}>
                    <Icon name="arrow-clockwise" />
                    {m.sources_inspector_update()}
                  </Button>
                </div>
                <dl className="divide-y divide-hairline border-y border-hairline text-ui-sm">
                  <div className="flex min-h-(--density-row) items-center justify-between gap-4">
                    <dt className="text-ink-muted">{m.sources_install_source_name()}</dt>
                    <dd className="max-w-[65%] truncate font-mono">{source.profile.id}</dd>
                  </div>
                  <div className="flex min-h-(--density-row) items-center justify-between gap-4">
                    <dt className="text-ink-muted">{m.sources_install_version()}</dt>
                    <dd className="font-mono">{source.version}</dd>
                  </div>
                  <div className="flex min-h-(--density-row) items-center justify-between gap-4">
                    <dt className="text-ink-muted">{m.sources_inspector_revision()}</dt>
                    <dd className="font-mono tabular-nums">{source.revision}</dd>
                  </div>
                  <div className="flex min-h-(--density-row) items-center justify-between gap-4">
                    <dt className="text-ink-muted">{m.sources_inspector_grant()}</dt>
                    <dd>{grantLabel(source.grant.network, m)}</dd>
                  </div>
                  <div className="flex min-h-(--density-row) items-center justify-between gap-4">
                    <dt className="text-ink-muted">{m.sources_inspector_system()}</dt>
                    <dd>{currentSystemGrant}</dd>
                  </div>
                </dl>
              </section>

              <Separator />

              <section aria-labelledby="source-inspector-details" className="py-3">
                <h3 id="source-inspector-details" className="mb-2 font-medium">
                  {m.sources_inspector_details()}
                </h3>
                <div className="space-y-2 text-ui-sm">
                  <p>
                    <span className="text-ink-muted">{m.sources_inspector_details()}: </span>
                    {source.profile.title}
                  </p>
                  <p>
                    <span className="text-ink-muted">{m.sources_group_label()}: </span>
                    {source.profile.group ?? m.sources_group_ungrouped()}
                  </p>
                  <div>
                    <p className="mb-1 text-ink-muted">{m.sources_inspector_intents()}</p>
                    <div className="flex flex-wrap gap-1">
                      {source.profile.supported_intents.length > 0
                        ? source.profile.supported_intents.map((intent) => (
                            <Badge key={intent} variant="outline">
                              {intent}
                            </Badge>
                          ))
                        : m.sources_no_intents()}
                    </div>
                  </div>
                  <div>
                    <p className="mb-1 text-ink-muted">{m.sources_inspector_risk()}</p>
                    {source.profile.risk_notes.length > 0 ? (
                      <ul className="list-disc space-y-1 pl-5">
                        {source.profile.risk_notes.map((note) => (
                          <li key={note}>{note}</li>
                        ))}
                      </ul>
                    ) : (
                      <p>{m.sources_inspector_no_risk()}</p>
                    )}
                  </div>
                </div>
              </section>

              <Separator />

              <section aria-labelledby="source-inspector-history" className="py-3">
                <h3 id="source-inspector-history" className="mb-2 font-medium">
                  {m.sources_inspector_history()}
                </h3>
                {historyState === 'loading' ? (
                  <p className="flex items-center gap-2 text-ui-sm" role="status">
                    <Spinner className="animate-spin" />
                    {m.sources_inspector_loading_history()}
                  </p>
                ) : null}
                {historyState === 'error' ? (
                  <div role="alert" className="space-y-2 text-ui-sm">
                    <p>{m.sources_inspector_history_error()}</p>
                    <p className="font-mono wrap-break-word text-ink-muted">{historyError}</p>
                  </div>
                ) : null}
                {historyState === 'ready' && history.length === 0 ? (
                  <p className="text-ui-sm text-ink-muted">{m.sources_inspector_history_empty()}</p>
                ) : null}
                {history.length > 0 ? (
                  <ul className="divide-y divide-hairline border-y border-hairline">
                    {history.map((revision) => {
                      const current = revision.revision === source.revision;
                      return (
                        <li key={revision.revision}>
                          {current ? (
                            <div className="space-y-1 py-2 text-ui-sm">
                              <div className="flex items-center gap-2">
                                <span className="font-mono">
                                  {m.sources_inspector_revision()} {revision.revision}
                                </span>
                                <Badge>{m.sources_inspector_current_badge()}</Badge>
                              </div>
                              <p className="text-ink-muted">{revision.version}</p>
                              <p className="font-mono text-ink-muted">
                                {formatTimestamp(revision.installed_at_ms)}
                              </p>
                            </div>
                          ) : (
                            <button
                              type="button"
                              className="block w-full space-y-1 py-2 text-left text-ui-sm hover:bg-surface-2 focus-visible:outline-1 focus-visible:outline-ring"
                              disabled={
                                rollbackPhase === 'preparing' || rollbackPhase === 'installing'
                              }
                              onClick={() => void prepareRollback(revision)}
                            >
                              <div className="flex items-center justify-between gap-2">
                                <span className="font-mono">
                                  {m.sources_inspector_revision()} {revision.revision}
                                </span>
                                <span className="text-ink-muted">
                                  {m.sources_inspector_rollback()}
                                </span>
                              </div>
                              <p className="text-ink-muted">{revision.version}</p>
                              <p className="font-mono text-ink-muted">
                                {formatTimestamp(revision.installed_at_ms)}
                              </p>
                            </button>
                          )}
                        </li>
                      );
                    })}
                  </ul>
                ) : null}
              </section>

              {rollbackTarget ? <Separator /> : null}

              {rollbackTarget ? (
                <section aria-labelledby="source-inspector-rollback" className="space-y-3 py-3">
                  <div className="flex items-center justify-between gap-2">
                    <h3 id="source-inspector-rollback" className="font-medium">
                      {m.sources_inspector_rollback_target({ revision: rollbackTarget.revision })}
                    </h3>
                    {rollbackPhase === 'preparing' || rollbackPhase === 'installing' ? (
                      <Spinner className="animate-spin" />
                    ) : null}
                  </div>
                  {rollbackPhase === 'preparing' ? (
                    <p className="text-ui-sm text-ink-muted">
                      {m.sources_inspector_rollback_prepare()}
                    </p>
                  ) : null}
                  {rollbackCandidate ? (
                    <div className="space-y-3 border border-hairline bg-surface-2 p-2 text-ui-sm">
                      <p className="text-ink-muted">{m.sources_inspector_rollback_review()}</p>
                      <dl className="grid gap-x-3 gap-y-1 sm:grid-cols-[auto_1fr]">
                        <dt className="text-ink-muted">{m.sources_install_version()}</dt>
                        <dd className="font-mono">
                          {rollbackCandidate.profile.version ?? m.sources_value_none()}
                        </dd>
                        <dt className="text-ink-muted">{m.sources_install_definition_hash()}</dt>
                        <dd className="font-mono break-all">{rollbackCandidate.definition_hash}</dd>
                        <dt className="text-ink-muted">{m.sources_install_plan_hash()}</dt>
                        <dd className="font-mono break-all">{rollbackCandidate.plan_hash}</dd>
                        <dt className="text-ink-muted">
                          {m.sources_install_diagnostic_summary({
                            count: rollbackCandidate.diagnostics.length,
                          })}
                        </dt>
                        <dd>
                          {rollbackCandidate.diagnostics.length === 0
                            ? m.sources_install_no_diagnostics()
                            : rollbackCandidate.diagnostics.map((item) => item.code).join(', ')}
                        </dd>
                      </dl>
                      {candidateNeedsSystem ? (
                        <p
                          role="alert"
                          className="border border-danger bg-danger-soft p-2 text-danger"
                        >
                          {m.sources_install_system_unsupported()}
                        </p>
                      ) : null}
                      {candidateNeedsNetwork && !candidateNeedsSystem ? (
                        <p className="text-ink-muted">
                          {m.sources_install_network_required_notice()}
                        </p>
                      ) : null}
                    </div>
                  ) : null}
                  {rollbackErrorLabelText ? (
                    <p role="alert" className="wrap-break-word text-danger">
                      {rollbackErrorLabelText}
                      {rollbackDetail ? (
                        <span className="ml-1 font-mono text-ui-sm">{rollbackDetail}</span>
                      ) : null}
                    </p>
                  ) : null}
                  <div className="flex flex-wrap gap-2">
                    {(rollbackPhase === 'error' || rollbackPhase === 'confirm') &&
                    rollbackCandidate ? (
                      <Button onClick={() => void installRollback()}>
                        {m.sources_install_action()}
                      </Button>
                    ) : null}
                    {rollbackPhase === 'error' ? (
                      <Button
                        variant="outline"
                        onClick={() => void prepareRollback(rollbackTarget)}
                      >
                        {m.sources_install_retry_review()}
                      </Button>
                    ) : null}
                  </div>
                </section>
              ) : null}
            </>
          )}
        </div>
      </SheetContent>
    </Sheet>
  );
}
