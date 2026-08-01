import type { InstallDiagnostic, SourceProfile, StandardIntent } from './rules.svelte';

export type { SourceProfile, StandardIntent } from './rules.svelte';

export type IntentInput =
  | { type: 'Query'; value: string }
  | { type: 'ItemId'; value: string }
  | { type: 'UnitId'; value: string }
  | { type: 'ActionId'; value: string }
  | { type: 'Opaque'; value: unknown }
  | { type: 'Page'; value: string }
  | { type: 'None' };

export interface MediaItem {
  id: string;
  source_id: string;
  media_kind: string;
  title: string;
  subtitle: string | null;
  creators: string[];
  description: string | null;
  cover_asset_id: string | null;
  metadata: Record<string, unknown>;
  completeness: string;
  updated_at: string | null;
}

export interface MediaUnit {
  id: string;
  source_id: string;
  item_id: string;
  title: string;
  position: number | null;
  metadata: Record<string, unknown>;
  completeness: string;
}

export interface MediaCollection {
  id: string;
  source_id: string;
  title: string;
  kind: string;
  item_ids: string[];
  metadata: Record<string, unknown>;
  completeness: string;
}

export type MediaAssetLocator =
  | { type: 'text'; value: string }
  | { type: 'url'; value: string }
  | { type: 'file_path'; value: string }
  | { type: 'bytes'; value: number[] }
  | { type: 'unresolved' };

export interface MediaAsset {
  id: string;
  source_id: string;
  unit_id: string | null;
  asset_kind: string;
  locator: MediaAssetLocator;
  metadata: Record<string, unknown>;
  completeness: string;
}

export interface MediaRelation {
  source_id: string;
  from_id: string;
  to_id: string;
  relation_kind: string;
}

export interface MediaAction {
  id: string;
  source_id: string;
  label: string;
  intent: StandardIntent;
  payload: unknown;
}

export interface PresentationHint {
  resource_id: string;
  card_density: string | null;
  cover_ratio: string | null;
  dominant_color: string | null;
  preferred_template: string | null;
}

/** DeltaCommitted 中可供界面消费的规范化资源；没有 effect 原始 body。 */
export interface ExecutionDelta {
  sources: SourceProfile[];
  items: MediaItem[];
  collections: MediaCollection[];
  units: MediaUnit[];
  assets: MediaAsset[];
  relations: MediaRelation[];
  actions: MediaAction[];
  hints: PresentationHint[];
}

export interface ArtifactRef {
  hash: string;
  codec: string;
}

export type RuleErrorStage =
  | 'import'
  | 'validation'
  | 'compile'
  | 'candidate'
  | 'install'
  | 'capability'
  | 'execution'
  | 'effect'
  | 'persistence'
  | 'replay'
  | 'cancelled'
  | 'internal';

export interface RuleError {
  stage: RuleErrorStage;
  code: string;
  message: string;
  trace_id: string;
  retryable: boolean;
  diagnostics: InstallDiagnostic[];
}

export type RuleExecutionEventKind =
  | { kind: 'started' }
  | { kind: 'diagnostic'; code: string; message: string }
  | {
      kind: 'effect_captured';
      effect_id: string;
      artifact_refs: ArtifactRef[];
      output_hash: string;
    }
  | {
      kind: 'delta_committed';
      global_revision: number;
      source_revision: number;
      delta: ExecutionDelta;
    }
  | { kind: 'completed' }
  | { kind: 'failed'; error: RuleError }
  | { kind: 'cancelled' };

export interface RuleExecutionEvent {
  execution_id: string;
  sequence: number;
  trace_id: string;
  occurred_at_ms: number;
  kind: RuleExecutionEventKind;
}

export interface ExecuteRequest {
  source_id: string;
  intent: StandardIntent;
  input: IntentInput;
  mode: { mode: 'live' } | { mode: 'replay'; execution_id: string };
}

export interface CancelExecutionResponse {
  execution_id: string;
  changed: boolean;
}

export interface CatchUpExecutionResponse {
  execution_id: string;
  replayed_count: number;
  delivered_through_sequence: number;
}
