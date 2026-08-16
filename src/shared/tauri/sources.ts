import { invoke, isTauri } from '@tauri-apps/api/core';

export type CapabilityGrantPreset = 'none' | 'network_only';
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
  revision: number;
}

/** Prepare_install 返回的安全候选；不含 Definition、Plan、body 或 secret。 */
export interface InstallCandidate {
  id: string;
  expected_installed_revision: number;
  profile: SourceProfile;
  required_grant: {
    network: boolean;
    system: { env: boolean; fs: boolean; process: boolean };
  };
  diagnostics: InstallDiagnostic[];
  definition_hash: string;
  plan_hash: string;
  expires_at_ms: number;
}

export function listInstalledSources(): Promise<InstalledSource[]> {
  if (!isTauri()) return Promise.resolve([]);
  return invoke<InstalledSource[]>('list_installed_sources');
}

export function fetchImportSource(url: string): Promise<string> {
  return invoke<string>('fetch_import_src', { request: { url } });
}

export function prepareSourceInstall(sourceJson: string): Promise<InstallCandidate> {
  return invoke<InstallCandidate>('prepare_install', {
    request: { kind: 'legado', source_json: sourceJson },
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
