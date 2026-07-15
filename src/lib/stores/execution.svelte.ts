//! 执行状态 store — 管理标准意图执行与媒体资源图增量。
//!
//! 监听 rule-output / node-output 事件，将 `MediaGraphDelta` 合并为前端可消费的
//! 媒体条目、媒体单元和媒体资产状态。字段名与 Rust serde 序列化保持一致。

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type StandardIntent =
  | 'Search'
  | 'Discover'
  | 'ResolveItem'
  | 'ListUnits'
  | 'ResolveAsset'
  | 'ContinueAction';

export type IntentInput =
  | { type: 'Query'; value: string }
  | { type: 'ItemId'; value: string }
  | { type: 'UnitId'; value: string }
  | { type: 'ActionId'; value: string }
  | { type: 'Opaque'; value: unknown }
  | { type: 'Page'; value: string }
  | { type: 'None' };

export interface SourceProfile {
  id: string;
  title: string;
  icon_url: string | null;
  version: string | null;
  supported_intents: StandardIntent[];
  risk_notes: string[];
}

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

export interface MediaCollection {
  id: string;
  source_id: string;
  title: string;
  kind: string;
  item_ids: string[];
  metadata: Record<string, unknown>;
  completeness: string;
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

export interface MediaAssetLocator {
  type: string;
  value?: unknown;
}

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

export interface MediaGraphDelta {
  sources?: SourceProfile[];
  items?: MediaItem[];
  collections?: MediaCollection[];
  units?: MediaUnit[];
  assets?: MediaAsset[];
  relations?: MediaRelation[];
  actions?: MediaAction[];
  hints?: PresentationHint[];
}

export interface NodeOutput {
  node_id: string;
  variant: string;
  summary: string;
}

interface RuleErrorPayload {
  message: string;
}

type Stage = 'results' | 'units' | 'asset' | null;

let sourceProfiles = $state<SourceProfile[]>([]);
let mediaItems = $state<MediaItem[]>([]);
let mediaCollections = $state<MediaCollection[]>([]);
let mediaUnits = $state<MediaUnit[]>([]);
let mediaAssets = $state<MediaAsset[]>([]);
let mediaRelations = $state<MediaRelation[]>([]);
let mediaActions = $state<MediaAction[]>([]);
let presentationHints = $state<PresentationHint[]>([]);
let resolvedText = $state<string | null>(null);
let nodeOutputs = $state<NodeOutput[]>([]);
let loading = $state(false);
let error = $state<string | null>(null);
let currentStage = $state<Stage>(null);
let selectedItem = $state<MediaItem | null>(null);
let selectedUnit = $state<MediaUnit | null>(null);

let unlistenRuleOutput: UnlistenFn | null = null;
let unlistenNodeOutput: UnlistenFn | null = null;
let unlistenRuleError: UnlistenFn | null = null;

function mergeById<T extends { id: string }>(current: T[], incoming: T[] | undefined): T[] {
  if (!incoming || incoming.length === 0) return current;

  const merged = [...current];
  for (const item of incoming) {
    let existingIndex = -1;
    for (let index = 0; index < merged.length; index += 1) {
      if (merged[index].id === item.id) {
        existingIndex = index;
        break;
      }
    }

    if (existingIndex === -1) {
      merged.push(item);
    } else {
      merged[existingIndex] = item;
    }
  }
  return merged;
}

function appendItems<T>(current: T[], incoming: T[] | undefined): T[] {
  return incoming && incoming.length > 0 ? [...current, ...incoming] : current;
}

function assetText(asset: MediaAsset): string | null {
  if (asset.locator.type !== 'text') return null;
  return typeof asset.locator.value === 'string' ? asset.locator.value : null;
}

function applyDelta(delta: MediaGraphDelta): void {
  sourceProfiles = mergeById(sourceProfiles, delta.sources);
  mediaItems = mergeById(mediaItems, delta.items);
  mediaCollections = mergeById(mediaCollections, delta.collections);
  mediaUnits = mergeById(mediaUnits, delta.units);
  mediaAssets = mergeById(mediaAssets, delta.assets);
  mediaRelations = appendItems(mediaRelations, delta.relations);
  mediaActions = appendItems(mediaActions, delta.actions);
  presentationHints = appendItems(presentationHints, delta.hints);

  const text = mediaAssets
    .filter((asset) => !selectedUnit || asset.unit_id === selectedUnit.id)
    .flatMap((asset) => {
      const value = assetText(asset);
      return value === null ? [] : [value];
    })
    .join('\n\n');
  resolvedText = text || resolvedText;
}

function reset(): void {
  sourceProfiles = [];
  mediaItems = [];
  mediaCollections = [];
  mediaUnits = [];
  mediaAssets = [];
  mediaRelations = [];
  mediaActions = [];
  presentationHints = [];
  resolvedText = null;
  nodeOutputs = [];
  error = null;
  currentStage = null;
  selectedItem = null;
  selectedUnit = null;
}

async function setupListeners(): Promise<void> {
  if (unlistenRuleOutput) return;

  unlistenRuleOutput = await listen<unknown>('rule-output', (event: { payload: unknown }) => {
    const payload = event.payload;
    if (!payload || typeof payload !== 'object') return;

    const delta = (payload as Record<string, unknown>)['Delta'];
    if (delta && typeof delta === 'object') {
      applyDelta(delta as MediaGraphDelta);
    }
  });

  unlistenNodeOutput = await listen<NodeOutput>('node-output', (event: { payload: NodeOutput }) => {
    nodeOutputs.push(event.payload);
  });

  unlistenRuleError = await listen<RuleErrorPayload>(
    'rule-error',
    (event: { payload: RuleErrorPayload }) => {
      error = event.payload.message;
    },
  );
}

function teardownListeners(): void {
  unlistenRuleOutput?.();
  unlistenNodeOutput?.();
  unlistenRuleError?.();
  unlistenRuleOutput = null;
  unlistenNodeOutput = null;
  unlistenRuleError = null;
}

async function runIntent(params: {
  ruleId: string;
  intent: StandardIntent;
  input: IntentInput;
}): Promise<void> {
  try {
    await invoke('execute_segment', {
      request: {
        rule_id: params.ruleId,
        intent: params.intent,
        input: params.input,
      },
    });
  } catch (caught) {
    error = String(caught);
    throw caught;
  }
}

async function startIntent(
  intent: 'Search' | 'Discover',
  ruleId: string,
  input: IntentInput,
): Promise<void> {
  reset();
  loading = true;
  currentStage = 'results';
  error = null;

  await setupListeners();

  try {
    await runIntent({ ruleId, intent, input });
  } catch {
    // error 已在 runIntent 中设置。
  } finally {
    loading = false;
  }
}

export async function startSearch(ruleId: string, query: string): Promise<void> {
  return startIntent('Search', ruleId, { type: 'Query', value: query });
}

export async function startDiscover(ruleId: string): Promise<void> {
  return startIntent('Discover', ruleId, { type: 'None' });
}

export async function selectMediaItem(item: MediaItem, ruleId: string): Promise<void> {
  selectedItem = item;
  selectedUnit = null;
  resolvedText = null;
  mediaUnits = mediaUnits.filter((unit) => unit.item_id === item.id);
  error = null;
  loading = true;
  currentStage = 'units';

  try {
    await runIntent({ ruleId, intent: 'ResolveItem', input: { type: 'ItemId', value: item.id } });
    await runIntent({ ruleId, intent: 'ListUnits', input: { type: 'ItemId', value: item.id } });
  } catch {
    // error 已在 runIntent 中设置。
  } finally {
    loading = false;
  }
}

export async function selectMediaUnit(unit: MediaUnit, ruleId: string): Promise<void> {
  selectedUnit = unit;
  resolvedText = null;
  error = null;
  loading = true;
  currentStage = 'asset';

  try {
    await runIntent({ ruleId, intent: 'ResolveAsset', input: { type: 'UnitId', value: unit.id } });
  } catch {
    // error 已在 runIntent 中设置。
  } finally {
    loading = false;
  }
}

export function goBack(): void {
  if (currentStage === 'asset') {
    selectedUnit = null;
    resolvedText = null;
    currentStage = 'units';
  } else if (currentStage === 'units') {
    selectedItem = null;
    currentStage = 'results';
  }
}

export function cleanup(): void {
  teardownListeners();
}

export function getSourceProfiles(): SourceProfile[] {
  return sourceProfiles;
}

export function getMediaItems(): MediaItem[] {
  return mediaItems;
}

export function getMediaCollections(): MediaCollection[] {
  return mediaCollections;
}

export function getMediaUnits(): MediaUnit[] {
  return selectedItem ? mediaUnits.filter((unit) => unit.item_id === selectedItem?.id) : mediaUnits;
}

export function getMediaAssets(): MediaAsset[] {
  return mediaAssets;
}

export function getMediaRelations(): MediaRelation[] {
  return mediaRelations;
}

export function getMediaActions(): MediaAction[] {
  return mediaActions;
}

export function getPresentationHints(): PresentationHint[] {
  return presentationHints;
}

export function getResolvedText(): string | null {
  return resolvedText;
}

export function getNodeOutputs(): NodeOutput[] {
  return nodeOutputs;
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
