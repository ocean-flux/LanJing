//! 执行会话投影与 UI stage store。
//!
//! wire DTO 在 `execution-wire.ts`，唯一事件通道与 sequence 补齐在
//! `execution-delivery.svelte.ts`；本模块只拥有规范化媒体投影和页面 stage。

import { createExecutionDelivery } from './execution-delivery.svelte';
import type {
  CancelExecutionResponse,
  ExecutionDelta,
  IntentInput,
  MediaAsset,
  MediaItem,
  MediaUnit,
  RuleExecutionEvent,
  SourceProfile,
  StandardIntent,
} from './execution-wire';

export type {
  CancelExecutionResponse,
  ExecutionDelta,
  IntentInput,
  MediaAsset,
  MediaAssetLocator,
  MediaItem,
  MediaUnit,
  RuleExecutionEvent,
  RuleExecutionEventKind,
  SourceProfile,
  StandardIntent,
} from './execution-wire';

type Stage = 'results' | 'units' | 'asset' | null;

let sourceProfiles = $state<SourceProfile[]>([]);
let mediaItems = $state<MediaItem[]>([]);
let mediaUnits = $state<MediaUnit[]>([]);
let mediaAssets = $state<MediaAsset[]>([]);
let resolvedText = $state<string | null>(null);
let diagnostics = $state<{ code: string; message: string }[]>([]);
let loading = $state(false);
let error = $state<string | null>(null);
let currentStage = $state<Stage>(null);
let selectedItem = $state<MediaItem | null>(null);
let selectedUnit = $state<MediaUnit | null>(null);

function mergeById<T extends { id: string }>(current: T[], incoming: T[]): T[] {
  if (incoming.length === 0) return current;

  const merged = [...current];
  for (const item of incoming) {
    const existingIndex = merged.findIndex((existing) => existing.id === item.id);
    if (existingIndex === -1) {
      merged.push(item);
    } else {
      merged[existingIndex] = item;
    }
  }
  return merged;
}

function applyDelta(delta: ExecutionDelta): void {
  sourceProfiles = mergeById(sourceProfiles, delta.sources);
  mediaItems = mergeById(mediaItems, delta.items);
  mediaUnits = mergeById(mediaUnits, delta.units);
  mediaAssets = mergeById(mediaAssets, delta.assets);

  const text = mediaAssets
    .filter((asset) => !selectedUnit || asset.unit_id === selectedUnit.id)
    .flatMap((asset) =>
      asset.asset_kind === 'text' && asset.locator.type === 'text' ? [asset.locator.value] : [],
    )
    .join('\n\n');
  resolvedText = text || resolvedText;
}

function applyExecutionEvent(event: RuleExecutionEvent): void {
  switch (event.kind.kind) {
    case 'started':
      loading = true;
      break;
    case 'diagnostic':
      diagnostics = [...diagnostics, { code: event.kind.code, message: event.kind.message }];
      break;
    case 'effect_captured':
      // artifact 引用仅供 runtime replay；UI 不缓存或显示 body/secret。
      break;
    case 'delta_committed':
      applyDelta(event.kind.delta);
      break;
    case 'failed':
      error = event.kind.error.message;
      break;
    case 'completed':
    case 'cancelled':
      break;
  }

  if (
    event.kind.kind === 'completed' ||
    event.kind.kind === 'failed' ||
    event.kind.kind === 'cancelled'
  ) {
    loading = false;
  }
}

const delivery = createExecutionDelivery({ onEvent: applyExecutionEvent });

function reset(): void {
  sourceProfiles = [];
  mediaItems = [];
  mediaUnits = [];
  mediaAssets = [];
  resolvedText = null;
  diagnostics = [];
  error = null;
  currentStage = null;
  selectedItem = null;
  selectedUnit = null;
}

async function runIntent(params: {
  sourceId: string;
  intent: StandardIntent;
  input: IntentInput;
}): Promise<void> {
  try {
    await delivery.execute({
      source_id: params.sourceId,
      intent: params.intent,
      input: params.input,
      mode: { mode: 'live' },
    });
  } catch (caught) {
    error = String(caught);
    loading = false;
    throw caught;
  }
}

async function startIntent(
  intent: 'Search' | 'Discover',
  sourceId: string,
  input: IntentInput,
): Promise<void> {
  reset();
  loading = true;
  currentStage = 'results';

  try {
    await runIntent({ sourceId, intent, input });
  } catch {
    // IPC 错误已写入安全错误状态。
  }
}

export async function startSearch(sourceId: string, query: string): Promise<void> {
  return startIntent('Search', sourceId, { type: 'Query', value: query });
}

export async function startDiscover(sourceId: string): Promise<void> {
  return startIntent('Discover', sourceId, { type: 'None' });
}

export async function selectMediaItem(item: MediaItem, sourceId: string): Promise<void> {
  selectedItem = item;
  selectedUnit = null;
  mediaUnits = mediaUnits.filter((unit) => unit.item_id === item.id);
  resolvedText = null;
  error = null;
  loading = true;
  currentStage = 'units';

  try {
    await runIntent({ sourceId, intent: 'ResolveItem', input: { type: 'ItemId', value: item.id } });
    await runIntent({ sourceId, intent: 'ListUnits', input: { type: 'ItemId', value: item.id } });
  } catch {
    // IPC 错误已写入安全错误状态。
  }
}

export async function selectMediaUnit(unit: MediaUnit, sourceId: string): Promise<void> {
  selectedUnit = unit;
  resolvedText = null;
  error = null;
  loading = true;
  currentStage = 'asset';

  try {
    await runIntent({
      sourceId,
      intent: 'ResolveAsset',
      input: { type: 'UnitId', value: unit.id },
    });
  } catch {
    // IPC 错误已写入安全错误状态。
  }
}

export function cancelExecution(): Promise<CancelExecutionResponse | null> {
  return delivery.cancel();
}

export function goBack(): void {
  if (currentStage === 'asset') {
    selectedUnit = null;
    currentStage = 'units';
  } else if (currentStage === 'units') {
    selectedItem = null;
    currentStage = 'results';
  }
}

export function cleanup(): void {
  delivery.cleanup();
}

export function getSourceProfiles(): SourceProfile[] {
  return sourceProfiles;
}

export function getMediaItems(): MediaItem[] {
  return mediaItems;
}

export function getMediaUnits(): MediaUnit[] {
  const item = selectedItem;
  return item ? mediaUnits.filter((unit) => unit.item_id === item.id) : mediaUnits;
}

export function getResolvedText(): string | null {
  return resolvedText;
}

export function getDiagnostics(): { code: string; message: string }[] {
  return diagnostics;
}

export function getLoading(): boolean {
  return loading;
}

export function getError(): string | null {
  return error;
}

export function getCurrentStage(): Stage {
  return currentStage;
}

export function getSelectedItem(): MediaItem | null {
  return selectedItem;
}

export function getSelectedUnit(): MediaUnit | null {
  return selectedUnit;
}
