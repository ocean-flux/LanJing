import type { SourceProfile, StandardIntent } from './rules.svelte';

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
  completeness: string;
  updated_at: string | null;
}

export interface MediaUnit {
  id: string;
  source_id: string;
  item_id: string;
  title: string;
  position: number | null;
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
  completeness: string;
}

/** DeltaCommitted 中可供界面消费的规范化资源；没有 effect 原始 body。 */
export interface ExecutionDelta {
  sources: SourceProfile[];
  items: MediaItem[];
  units: MediaUnit[];
  assets: MediaAsset[];
}

export type RuleExecutionEventKind =
  | { kind: 'started' }
  | { kind: 'diagnostic'; code: string; message: string }
  | { kind: 'effect_captured'; effect_id: string; output_hash: string }
  | {
      kind: 'delta_committed';
      global_revision: number;
      source_revision: number;
      delta: ExecutionDelta;
    }
  | { kind: 'completed' }
  | { kind: 'failed'; error: { message: string } }
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
