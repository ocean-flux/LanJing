//! 已安装来源 store — 只镜像 RuleSystem 的安全 candidate/source DTO。

import { invoke } from '@tauri-apps/api/core';

export type CapabilityGrantPreset = 'none' | 'network_only';

export type StandardIntent =
  'Search' | 'Discover' | 'ResolveItem' | 'ListUnits' | 'ResolveAsset' | 'ContinueAction';

export interface SourceProfile {
  id: string;
  title: string;
  icon_url: string | null;
  version: string | null;
  supported_intents: StandardIntent[];
  risk_notes: string[];
}

export interface InstallDiagnostic {
  code: string;
  message: string;
}

/** prepare_install 返回的安全候选；不包含 Definition、Plan、body 或 secret。 */
export interface InstallCandidate {
  id: string;
  profile: SourceProfile;
  required_grant: {
    network: boolean;
    system: {
      fs: boolean;
      env: boolean;
      process: boolean;
    };
  };
  diagnostics: InstallDiagnostic[];
  definition_hash: string;
  plan_hash: string;
  expires_at_ms: number;
}

/** 已安装来源的安全摘要；后续执行只使用 source_id。 */
export interface InstalledSource {
  source_id: string;
  version: string;
  profile: SourceProfile;
  revision: number;
}

let installedSources = $state<InstalledSource[]>([]);
let loading = $state(false);
let error = $state<string | null>(null);

/** 读取 RuleSystem 管理的已安装来源。 */
export async function loadInstalledSources(): Promise<void> {
  loading = true;
  error = null;
  try {
    installedSources = await invoke<InstalledSource[]>('list_installed_sources');
  } catch (caught) {
    error = String(caught);
  } finally {
    loading = false;
  }
}

/** 暂存 Legado 书源，并只返回可安全展示的候选信息。 */
export function prepareInstall(sourceJson: string): Promise<InstallCandidate> {
  return invoke<InstallCandidate>('prepare_install', {
    request: { kind: 'legado', source_json: sourceJson },
  });
}

/** 原子安装已暂存 candidate，并刷新来源列表。 */
export async function installCandidate(
  candidateId: string,
  grant: CapabilityGrantPreset,
): Promise<InstalledSource> {
  const source = await invoke<InstalledSource>('install', {
    request: { candidate_id: candidateId, grant },
  });
  await loadInstalledSources();
  return source;
}

export function getInstalledSources(): InstalledSource[] {
  return installedSources;
}

export function getLoading(): boolean {
  return loading;
}

export function getError(): string | null {
  return error;
}
