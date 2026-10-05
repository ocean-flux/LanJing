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
import { NativeSelect, NativeSelectOption } from '@/components/ui/native-select';
import { Spinner } from '@/components/ui/spinner';
import { Textarea } from '@/components/ui/textarea';
import { useMessages } from '@/shared/i18n/messages';
import { CATALOG_INSTALL_CAP } from '@/shared/tauri/catalog';
import type { InstallCandidate, InstalledSource } from '@/shared/tauri/sources';
import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';

import {
  type SourceWorkflowAdapter,
  availableSourceGroups,
  candidateRequestsSystem,
  createSourceWorkflow,
} from './workflow';

interface SourceInstallDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onInstalled: () => void;
  adapter?: SourceWorkflowAdapter;
}

function expiryLabel(expiresAtMs: number, m: ReturnType<typeof useMessages>): string {
  const remaining = expiresAtMs - Date.now();
  if (remaining <= 0) return m.sources_install_expired();
  const minutes = Math.floor(remaining / 60_000);
  if (minutes < 60) return m.sources_install_expires_in_minutes({ minutes });
  return m.sources_install_expires_in_hours({ hours: Math.floor(minutes / 60) });
}

function errorLabel(code: string | null, m: ReturnType<typeof useMessages>): string | null {
  switch (code) {
    case 'input_empty': {
      return m.sources_install_input_empty();
    }
    case 'input_unrecognized': {
      return m.sources_install_input_unrecognized();
    }
    case 'input_file_read_failed': {
      return m.sources_install_file_read_failed();
    }
    case 'input_invalid_json': {
      return m.sources_diagnostic_invalid_json();
    }
    case 'input_not_book_source': {
      return m.sources_deeplink_catalog_not_book_source();
    }
    case 'selection_invalid': {
      return m.sources_install_selection_invalid();
    }
    case 'system_grant_unsupported': {
      return m.sources_install_system_unsupported();
    }
    case 'candidate_stale': {
      return m.sources_install_candidate_stale();
    }
    case 'candidate_expired': {
      return m.sources_install_candidate_expired();
    }
    case 'candidate_consumed': {
      return m.sources_install_candidate_consumed();
    }
    case 'source_revision_conflict': {
      return m.sources_install_conflict();
    }
    case 'source_prepare_failed': {
      return m.sources_install_prepare_failed();
    }
    case 'source_operation_failed': {
      return m.sources_install_prepare_failed();
    }
    case 'source_revision_not_found': {
      return m.sources_error_source_revision_not_found();
    }
    default: {
      return code ? m.sources_install_prepare_failed() : null;
    }
  }
}

function listValue(value: string[] | null | undefined, empty: string): string {
  return value && value.length > 0 ? value.join(', ') : empty;
}

function systemValue(
  value: InstalledSource['grant'],
  empty: string,
  m: ReturnType<typeof useMessages>,
): string {
  const enabled = Object.entries(value)
    .filter(([, isEnabled]) => isEnabled)
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
  return enabled.length > 0 ? enabled.join(', ') : empty;
}

function changeLabel(changed: boolean, m: ReturnType<typeof useMessages>): string {
  return changed ? m.sources_install_changed() : m.sources_install_unchanged();
}

function SourceUpdateDiff({
  candidate,
  previous,
  m,
}: {
  candidate: InstallCandidate;
  previous: InstalledSource;
  m: ReturnType<typeof useMessages>;
}) {
  const versionChanged = previous.version !== candidate.profile.version;
  const titleChanged = previous.profile.title !== candidate.profile.title;
  const groupChanged = previous.profile.group !== candidate.profile.group;
  const intentsChanged =
    previous.profile.supported_intents.join(',') !== candidate.profile.supported_intents.join(',');
  const currentSystem = systemValue(previous.grant, m.sources_value_none(), m);
  const candidateSystem = systemValue(candidate.required_grant, m.sources_value_none(), m);
  const systemChanged = currentSystem !== candidateSystem;

  return (
    <div className="border border-hairline bg-surface-2 p-2">
      <p className="mb-2 font-medium">{m.sources_install_diff_title()}</p>
      <dl className="grid gap-x-3 gap-y-1 text-ui-sm sm:grid-cols-[auto_1fr]">
        <dt className="text-ink-muted">{m.sources_install_current_version()}</dt>
        <dd className="font-mono">
          {previous.version} <Icon name="arrow-right" />{' '}
          {candidate.profile.version ?? m.sources_value_none()}{' '}
          <Badge variant={versionChanged ? 'default' : 'outline'}>
            {changeLabel(versionChanged, m)}
          </Badge>
        </dd>
        <dt className="text-ink-muted">{m.sources_inspector_system()}</dt>
        <dd>
          {currentSystem} <Icon name="arrow-right" /> {candidateSystem}{' '}
          <Badge variant={systemChanged ? 'default' : 'outline'}>
            {changeLabel(systemChanged, m)}
          </Badge>
        </dd>
        <dt className="text-ink-muted">{m.sources_inspector_details()}</dt>
        <dd className="space-y-1">
          <span className="block">
            {previous.profile.title} <Icon name="arrow-right" /> {candidate.profile.title}{' '}
            <Badge variant={titleChanged ? 'default' : 'outline'}>
              {changeLabel(titleChanged, m)}
            </Badge>
          </span>
          <span className="block">
            {previous.profile.group ?? m.sources_value_none()} <Icon name="arrow-right" />{' '}
            {candidate.profile.group ?? m.sources_value_none()}{' '}
            <Badge variant={groupChanged ? 'default' : 'outline'}>
              {changeLabel(groupChanged, m)}
            </Badge>
          </span>
          <span className="block">
            {listValue(previous.profile.supported_intents, m.sources_no_intents())}{' '}
            <Icon name="arrow-right" />{' '}
            {listValue(candidate.profile.supported_intents, m.sources_no_intents())}{' '}
            <Badge variant={intentsChanged ? 'default' : 'outline'}>
              {changeLabel(intentsChanged, m)}
            </Badge>
          </span>
        </dd>
      </dl>
    </div>
  );
}

export function SourceInstallDialog({
  open,
  onOpenChange,
  onInstalled,
  adapter,
}: SourceInstallDialogProps) {
  const m = useMessages();
  const workflow = useMemo(() => createSourceWorkflow(adapter), [adapter]);
  const state = useSyncExternalStore(workflow.subscribe, workflow.getState, workflow.getState);
  const fileInput = useRef<HTMLInputElement>(null);
  const wasOpen = useRef(false);
  const [groupFilter, setGroupFilter] = useState('');

  useEffect(() => {
    if (open && !wasOpen.current) {
      workflow.setInput('');
      void workflow.refreshSources();
      setGroupFilter('');
    }
    wasOpen.current = open;
  }, [open, workflow]);

  const groups = useMemo(() => availableSourceGroups(state.catalog), [state.catalog]);
  const visibleCatalog = useMemo(
    () =>
      groupFilter ? state.catalog.filter((item) => item.group === groupFilter) : state.catalog,
    [groupFilter, state.catalog],
  );
  const requestsSystem = state.prepared.some((entry) => candidateRequestsSystem(entry.candidate));
  const localizedError = errorLabel(state.errorCode, m);

  const readFile = async (file: File) => {
    try {
      workflow.setInput(await file.text(), file.name);
    } catch (error) {
      workflow.setInput('');
      const detail = error instanceof Error ? error.message : String(error);
      // 文件读取失败时不回显文件内容，只保留输入区域的安全错误路径。
      workflow.setError('input_file_read_failed', detail);
    }
  };

  const install = async () => {
    await workflow.install();
    if (workflow.getState().phase === 'done') {
      onInstalled();
      onOpenChange(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{m.sources_install_title()}</DialogTitle>
          <DialogDescription>
            {state.fileName
              ? m.sources_install_file_name({ name: state.fileName })
              : m.sources_install_input_placeholder()}
          </DialogDescription>
        </DialogHeader>

        {state.phase !== 'confirm' && state.phase !== 'installing' && state.phase !== 'done' ? (
          <div className="space-y-2">
            <Label htmlFor="source-install-input">{m.sources_install_input_label()}</Label>
            <Textarea
              id="source-install-input"
              value={state.rawInput}
              placeholder={m.sources_install_input_placeholder()}
              rows={7}
              onChange={(event) => workflow.setInput(event.target.value, state.fileName)}
            />
            <div className="flex flex-wrap items-center gap-2">
              <input
                ref={fileInput}
                type="file"
                accept=".json,application/json"
                className="sr-only"
                onChange={(event) => {
                  const file = event.target.files?.[0];
                  event.target.value = '';
                  if (file) void readFile(file);
                }}
              />
              <Button variant="outline" size="sm" onClick={() => fileInput.current?.click()}>
                <Icon name="upload-simple" />
                {m.sources_install_open_file()}
              </Button>
              {state.fileName ? (
                <span className="text-ui-sm text-ink-muted">
                  {m.sources_install_file_name({ name: state.fileName })}
                </span>
              ) : null}
            </div>
          </div>
        ) : null}

        {state.phase === 'idle' ? (
          <p className="text-ui-sm text-ink-muted">{m.sources_deeplink_import_hint()}</p>
        ) : null}

        {state.phase === 'preparing' ? (
          <p className="flex items-center gap-2" role="status">
            <Spinner className="animate-spin" />
            {m.sources_install_preparing()}
          </p>
        ) : null}

        {localizedError ? (
          <p role="alert" className="wrap-break-word text-danger">
            {localizedError}
            {state.errorDetail ? (
              <span className="ml-1 font-mono text-ui-sm">{state.errorDetail}</span>
            ) : null}
          </p>
        ) : null}

        {state.phase === 'pick' ? (
          <div className="space-y-2">
            {groups.length >= 2 ? (
              <div className="flex items-center gap-2">
                <Label htmlFor="source-group-filter" className="shrink-0">
                  {m.sources_group_filter()}
                </Label>
                <NativeSelect
                  id="source-group-filter"
                  value={groupFilter}
                  onChange={(event) => setGroupFilter(event.target.value)}
                  size="sm"
                >
                  <NativeSelectOption value="">{m.sources_group_all()}</NativeSelectOption>
                  {groups.map((group) => (
                    <NativeSelectOption key={group} value={group}>
                      {group}
                    </NativeSelectOption>
                  ))}
                </NativeSelect>
              </div>
            ) : null}
            <p className="text-ui-sm text-ink-muted">
              {m.sources_deeplink_install_cap({
                selected: state.selectedIds.length,
                cap: CATALOG_INSTALL_CAP,
              })}
            </p>
            <ul className="max-h-56 divide-y divide-hairline overflow-y-auto border border-hairline">
              {visibleCatalog.map((item) => (
                <li key={item.id}>
                  <Label className="flex min-h-(--density-row) items-center gap-2 px-2 hover:bg-surface-2">
                    <Checkbox
                      checked={state.selectedIds.includes(item.id)}
                      onCheckedChange={(checked) => {
                        const selected = state.selectedIds;
                        workflow.setSelectedIds(
                          checked === true
                            ? [...selected, item.id]
                            : selected.filter((id) => id !== item.id),
                        );
                      }}
                    />
                    <span className="min-w-0 flex-1 truncate">{item.name}</span>
                    {item.group ? <Badge variant="outline">{item.group}</Badge> : null}
                  </Label>
                </li>
              ))}
            </ul>
          </div>
        ) : null}

        {state.phase === 'confirm' || state.phase === 'installing' ? (
          <div className="space-y-3">
            <div className="flex items-center justify-between gap-2">
              <h3 className="font-medium">{m.sources_install_preview_title()}</h3>
              <span className="font-mono text-ui-sm text-ink-muted">
                {m.sources_deeplink_selected_summary({ count: state.prepared.length })}
              </span>
            </div>
            <ul className="max-h-72 divide-y divide-hairline overflow-y-auto border-y border-hairline">
              {state.prepared.map(({ candidate, isUpdate, item, previous }) => (
                <li key={candidate.id} className="space-y-2 py-3">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-medium">{item.name}</span>
                    <Badge variant={isUpdate ? 'default' : 'outline'}>
                      {isUpdate ? m.sources_install_update() : m.sources_install_new()}
                    </Badge>
                  </div>
                  <dl className="grid gap-x-3 gap-y-1 text-ui-sm sm:grid-cols-[auto_1fr]">
                    <dt className="text-ink-muted">{m.sources_install_source_name()}</dt>
                    <dd className="min-w-0 truncate font-mono">{candidate.profile.id}</dd>
                    <dt className="text-ink-muted">{m.sources_install_version()}</dt>
                    <dd className="font-mono">
                      {candidate.profile.version ?? m.sources_value_none()}
                    </dd>
                    <dt className="text-ink-muted">{m.sources_install_definition_hash()}</dt>
                    <dd className="font-mono break-all">{candidate.definition_hash}</dd>
                    <dt className="text-ink-muted">{m.sources_install_plan_hash()}</dt>
                    <dd className="font-mono break-all">{candidate.plan_hash}</dd>
                    <dt className="text-ink-muted">{m.sources_install_expires()}</dt>
                    <dd className="font-mono">{expiryLabel(candidate.expires_at_ms, m)}</dd>
                    <dt className="text-ink-muted">{m.sources_supported_intents()}</dt>
                    <dd className="flex flex-wrap gap-1">
                      {candidate.profile.supported_intents.length > 0
                        ? candidate.profile.supported_intents.map((intent) => (
                            <Badge key={intent} variant="outline">
                              {intent}
                            </Badge>
                          ))
                        : m.sources_no_intents()}
                    </dd>
                    <dt className="text-ink-muted">
                      {m.sources_install_diagnostic_summary({
                        count: candidate.diagnostics.length,
                      })}
                    </dt>
                    <dd>
                      {candidate.diagnostics.length === 0
                        ? m.sources_install_no_diagnostics()
                        : candidate.diagnostics.map((diagnostic) => diagnostic.code).join(', ')}
                    </dd>
                  </dl>
                  {previous ? (
                    <SourceUpdateDiff candidate={candidate} previous={previous} m={m} />
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
          </div>
        ) : null}

        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={state.phase === 'installing'}
          >
            {m.action_cancel()}
          </Button>
          {state.phase === 'error' ? (
            <Button onClick={() => void workflow.prepareInput()}>
              {m.sources_install_prepare_input()}
            </Button>
          ) : null}
          {state.phase === 'pick' ? (
            <>
              {state.prepared.length === 0 && state.errorCode ? (
                <Button variant="outline" onClick={() => void workflow.retryPreparation()}>
                  {m.sources_install_retry_prepare()}
                </Button>
              ) : null}
              <Button
                disabled={
                  state.selectedIds.length === 0 || state.selectedIds.length > CATALOG_INSTALL_CAP
                }
                onClick={() => void workflow.prepareSelected()}
              >
                {m.sources_install_prepare_selected()}
              </Button>
            </>
          ) : null}
          {state.phase === 'idle' ? (
            <Button disabled={!state.rawInput.trim()} onClick={() => void workflow.prepareInput()}>
              {m.sources_install_prepare_input()}
            </Button>
          ) : null}
          {state.phase === 'confirm' || state.phase === 'installing' ? (
            <Button
              disabled={state.phase === 'installing' || requestsSystem}
              onClick={() => void install()}
            >
              {state.phase === 'installing'
                ? m.sources_install_installing()
                : m.sources_install_action()}
            </Button>
          ) : null}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
