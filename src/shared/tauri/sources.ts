import { invoke, isTauri } from '@tauri-apps/api/core';

export type CapabilityGrantPreset = 'none' | 'network_only';
export type SourcePrepareRequest =
  | { kind: 'legado'; source_json: string }
  | { kind: 'maccms_json'; url: string }
  | { kind: 'package'; source_json: string };
export type StandardIntent =
  | 'Search'
  | 'Discover'
  | 'ResolveItem'
  | 'ListUnits'
  | 'ResolveAsset'
  | 'ContinueAction';

export interface SourceProfile {
  id: string;
  title: string;
  icon_url: string | null;
  version: string | null;
  group: string | null;
  supported_intents: StandardIntent[];
  risk_notes: string[];
}

/** 诊断源码定位（镜像 lj-rule-model 的 SourceSpan）。 */
export interface SourceSpan {
  start: number;
  end: number;
  path: string | null;
}

/** 导入 / 编译诊断（镜像 lj-rule-model 的 Diagnostic）。 */
export interface InstallDiagnostic {
  code: string;
  severity: 'info' | 'warning' | 'error';
  message: string;
  span?: SourceSpan;
}

export interface InstalledSource {
  source_id: string;
  version: string;
  profile: SourceProfile;
  grant: CapabilityGrant;
  revision: number;
}

export interface CapabilityGrant {
  network: boolean;
  system: { env: boolean; fs: boolean; process: boolean };
}

export interface SourceRevision {
  source_id: string;
  revision: number;
  version: string;
  profile: SourceProfile;
  grant: CapabilityGrant;
  definition_hash: string;
  plan_hash: string;
  installed_at_ms: number;
}

/** 后端依据已安装状态判定的来源操作。 */
export type SourceOperation = 'install' | 'update';

/** Prepare_install 返回的安全候选；不含 Definition、Plan、body 或 secret。 */
export interface InstallCandidate {
  id: string;
  expected_installed_revision: number;
  operation: SourceOperation;
  profile: SourceProfile;
  required_grant: CapabilityGrant;
  diagnostics: InstallDiagnostic[];
  definition_hash: string;
  plan_hash: string;
  expires_at_ms: number;
}

export function listInstalledSources(): Promise<InstalledSource[]> {
  if (!isTauri()) return Promise.resolve([]);
  return invoke<InstalledSource[]>('list_installed_sources');
}

export function listSourceRevisions(sourceId: string): Promise<SourceRevision[]> {
  if (!isTauri()) return Promise.resolve([]);
  return invoke<SourceRevision[]>('list_source_revisions', {
    request: { source_id: sourceId },
  });
}

export function fetchImportSource(url: string): Promise<string> {
  return invoke<string>('fetch_import_src', { request: { url } });
}

export function prepareSourceInstall(
  request: SourcePrepareRequest | string,
): Promise<InstallCandidate> {
  const input: SourcePrepareRequest =
    typeof request === 'string' ? { kind: 'legado', source_json: request } : request;
  return invoke<InstallCandidate>('prepare_install', {
    request: input,
  });
}

export function installPreparedSource(
  candidateId: string,
  grant: CapabilityGrantPreset,
): Promise<InstalledSource> {
  return invoke<InstalledSource>('install', {
    request: { candidate_id: candidateId, grant },
  });
}

export function prepareSourceRollback(
  sourceId: string,
  revision: number,
): Promise<InstallCandidate> {
  return invoke<InstallCandidate>('prepare_source_rollback', {
    request: { source_id: sourceId, revision },
  });
}
