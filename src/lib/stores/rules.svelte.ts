//! 已安装来源 store — 只镜像 RuleSystem 的安全 candidate/source DTO。

import { invoke } from '@tauri-apps/api/core';

export type CapabilityGrantPreset = 'none' | 'network_only';

export type RuleInput =
  { kind: 'legado'; source_json: string } | { kind: 'maccms_json'; url: string };

export type StandardIntent =
  'Search' | 'Discover' | 'ResolveItem' | 'ListUnits' | 'ResolveAsset' | 'ContinueAction';

export interface SourceProfile {
  id: string;
  title: string;
  icon_url: string | null;
  version: string | null;
  group: string | null;
  supported_intents: StandardIntent[];
  risk_notes: string[];
}

export interface SourceSpan {
  start: number;
  end: number;
  path: string | null;
}

export interface InstallDiagnostic {
  code: string;
  severity: 'info' | 'warning' | 'error';
  message: string;
  span?: SourceSpan;
}

/** prepare_install 返回的安全候选；不包含 Definition、Plan、body 或 secret。 */
export interface InstallCandidate {
  id: string;
  expected_installed_revision: number;
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

/** 刷新 RuleSystem 管理的已安装来源；失败保持可观察状态并向协调方拒绝。 */
export async function refreshInstalledSources(): Promise<void> {
  loading = true;
  error = null;
  try {
    installedSources = await invoke<InstalledSource[]>('list_installed_sources');
  } catch (caught) {
    error = String(caught);
    throw caught;
  } finally {
    loading = false;
  }
}

/** 读取已安装来源供视图使用；错误由 store 的 error 状态呈现。 */
export async function loadInstalledSources(): Promise<void> {
  try {
    await refreshInstalledSources();
  } catch {
    // 视图通过 getError() 呈现失败；需要拒绝语义的协调方调用 refreshInstalledSources()。
  }
}

function prepareRuleInstall(request: RuleInput): Promise<InstallCandidate> {
  return invoke<InstallCandidate>('prepare_install', { request });
}

/** 将 Legado 原文交给一次性 import-only candidate staging。 */
export function prepareInstall(sourceJson: string): Promise<InstallCandidate> {
  return prepareRuleInstall({ kind: 'legado', source_json: sourceJson });
}

/** 将 Maccms URL 交给一次性 import-only candidate staging。 */
export function prepareMaccmsInstall(url: string): Promise<InstallCandidate> {
  return prepareRuleInstall({ kind: 'maccms_json', url });
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
