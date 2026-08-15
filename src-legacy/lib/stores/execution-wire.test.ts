import { describe, expect, it } from 'vitest';

import type { ExecutionDelta, RuleExecutionEventKind } from './execution-wire';

describe('execution wire contract', () => {
  it('mirrors all normalized media delta vectors', () => {
    const delta = {
      sources: [],
      items: [],
      collections: [],
      units: [],
      assets: [],
      relations: [],
      actions: [],
      hints: [],
    } satisfies ExecutionDelta;

    expect(Object.keys(delta)).toEqual([
      'sources',
      'items',
      'collections',
      'units',
      'assets',
      'relations',
      'actions',
      'hints',
    ]);
  });

  it('preserves artifact references on captured effects', () => {
    const event = {
      kind: 'effect_captured',
      effect_id: 'effect:1',
      artifact_refs: [{ hash: 'artifact-hash', codec: 'zstd' }],
      output_hash: 'output-hash',
    } satisfies RuleExecutionEventKind;

    expect(event.artifact_refs).toEqual([{ hash: 'artifact-hash', codec: 'zstd' }]);
  });

  it('preserves complete safe RuleError fields', () => {
    const event = {
      kind: 'failed',
      error: {
        stage: 'persistence',
        code: 'CURRENT_SCHEMA_REQUIRED',
        message: '当前存储 schema 必须为最新版本',
        trace_id: 'trace:1',
        retryable: false,
        diagnostics: [],
      },
    } satisfies RuleExecutionEventKind;

    expect(event.error).toMatchObject({
      stage: 'persistence',
      code: 'CURRENT_SCHEMA_REQUIRED',
      trace_id: 'trace:1',
      retryable: false,
    });
  });
});
