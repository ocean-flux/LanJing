import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

import {
  cancelExecution,
  cleanup,
  getDiagnostics,
  getLoading,
  getResolvedText,
  startSearch,
} from './execution.svelte';

type Listener = (event: { payload: unknown }) => void;

describe('execution RuleSystem wire', () => {
  let listener: Listener | null = null;
  let replayDelivery: (() => void) | null = null;

  beforeEach(() => {
    cleanup();
    listener = null;
    replayDelivery = null;
    invoke.mockReset();
    listen.mockReset();
    listen.mockImplementation(async (_eventName: string, callback: Listener) => {
      listener = callback;
      return vi.fn();
    });
  });

  it('backfills a sequence gap before applying the queued terminal event', async () => {
    invoke.mockImplementation(
      async (command: string, args?: { request?: { after_sequence?: number } }) => {
        if (command === 'execute') return { execution_id: 'execution:one' };
        if (command === 'catch_up_execution') {
          if (args?.request?.after_sequence === 1) {
            replayDelivery = () => {
              listener?.({
                payload: {
                  execution_id: 'execution:one',
                  sequence: 2,
                  trace_id: 'trace:one',
                  occurred_at_ms: 2,
                  kind: { kind: 'diagnostic', code: 'retry', message: 'recovered' },
                },
              });
            };
            return {
              execution_id: 'execution:one',
              replayed_count: 1,
              delivered_through_sequence: 2,
            };
          }
          return {
            execution_id: 'execution:one',
            replayed_count: 0,
            delivered_through_sequence: 0,
          };
        }
        if (command === 'cancel_execution') {
          return { execution_id: 'execution:one', changed: false };
        }
        throw new Error(`unexpected command: ${command}`);
      },
    );

    const running = startSearch('source:one', 'query');
    await vi.waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('catch_up_execution', {
        request: { execution_id: 'execution:one', after_sequence: 0 },
      }),
    );
    await Promise.resolve();

    listener?.({
      payload: {
        execution_id: 'execution:one',
        sequence: 1,
        trace_id: 'trace:one',
        occurred_at_ms: 1,
        kind: { kind: 'started' },
      },
    });
    listener?.({
      payload: {
        execution_id: 'execution:one',
        sequence: 3,
        trace_id: 'trace:one',
        occurred_at_ms: 3,
        kind: { kind: 'completed' },
      },
    });
    await vi.waitFor(() => expect(replayDelivery).not.toBeNull());
    await Promise.resolve();
    replayDelivery?.();

    await running;

    expect(invoke).toHaveBeenCalledWith('execute', {
      request: {
        source_id: 'source:one',
        intent: 'Search',
        input: { type: 'Query', value: 'query' },
        mode: { mode: 'live' },
      },
    });
    expect(invoke).toHaveBeenCalledWith('catch_up_execution', {
      request: { execution_id: 'execution:one', after_sequence: 1 },
    });
    expect(getDiagnostics()).toEqual([{ code: 'retry', message: 'recovered' }]);
    expect(getLoading()).toBe(false);
    await expect(cancelExecution()).resolves.toEqual({
      execution_id: 'execution:one',
      changed: false,
    });
    expect(invoke).toHaveBeenCalledWith('cancel_execution', {
      request: { execution_id: 'execution:one' },
    });
  });
  it('projects normalized text assets from delta_committed without effect payloads', async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === 'execute') return { execution_id: 'execution:asset' };
      if (command === 'catch_up_execution') {
        return {
          execution_id: 'execution:asset',
          replayed_count: 0,
          delivered_through_sequence: 0,
        };
      }
      throw new Error(`unexpected command: ${command}`);
    });

    const running = startSearch('source:one', 'query');
    await vi.waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('catch_up_execution', {
        request: { execution_id: 'execution:asset', after_sequence: 0 },
      }),
    );
    await Promise.resolve();

    listener?.({
      payload: {
        execution_id: 'execution:asset',
        sequence: 1,
        trace_id: 'trace:asset',
        occurred_at_ms: 1,
        kind: { kind: 'started' },
      },
    });
    listener?.({
      payload: {
        execution_id: 'execution:asset',
        sequence: 2,
        trace_id: 'trace:asset',
        occurred_at_ms: 2,
        kind: {
          kind: 'delta_committed',
          global_revision: 2,
          source_revision: 1,
          delta: {
            sources: [],
            items: [],
            units: [],
            assets: [
              {
                id: 'asset:reading',
                source_id: 'source:one',
                unit_id: 'unit:one',
                asset_kind: 'text',
                locator: { type: 'text', value: 'Normalized reading text' },
                completeness: 'complete',
              },
            ],
          },
        },
      },
    });
    listener?.({
      payload: {
        execution_id: 'execution:asset',
        sequence: 3,
        trace_id: 'trace:asset',
        occurred_at_ms: 3,
        kind: { kind: 'completed' },
      },
    });

    await running;

    expect(getResolvedText()).toBe('Normalized reading text');
  });
});
