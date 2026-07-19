import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import type {
  CancelExecutionResponse,
  CatchUpExecutionResponse,
  ExecuteRequest,
  RuleExecutionEvent,
} from './execution-wire';

interface ExecuteResponse {
  execution_id: string;
}

interface DeliveryHandlers {
  onEvent: (event: RuleExecutionEvent) => void;
}

/**
 * RuleSystem 唯一事件通道的 delivery 边界。
 *
 * 它只维护 execution_id 和连续 sequence；投影与 UI stage 留在 execution store。
 */
export function createExecutionDelivery({ onEvent }: DeliveryHandlers) {
  let unlistenExecutionEvent: UnlistenFn | null = null;
  let activeExecutionId: string | null = null;
  let lastSequence = 0;
  let pendingEvents = new SvelteMap<number, RuleExecutionEvent>();
  const terminalExecutionIds = new SvelteSet<string>();
  const terminalWaiters = new SvelteMap<string, () => void>();
  let catchUpPromise: Promise<void> | null = null;
  let catchUpDeliveryWaiter: {
    executionId: string;
    throughSequence: number;
    resolve: () => void;
  } | null = null;

  function applyEvent(event: RuleExecutionEvent): void {
    lastSequence = event.sequence;
    onEvent(event);

    if (
      catchUpDeliveryWaiter &&
      catchUpDeliveryWaiter.executionId === event.execution_id &&
      lastSequence >= catchUpDeliveryWaiter.throughSequence
    ) {
      const { resolve } = catchUpDeliveryWaiter;
      catchUpDeliveryWaiter = null;
      resolve();
    }

    if (
      event.kind.kind === 'completed' ||
      event.kind.kind === 'failed' ||
      event.kind.kind === 'cancelled'
    ) {
      terminalExecutionIds.add(event.execution_id);
      terminalWaiters.get(event.execution_id)?.();
      terminalWaiters.delete(event.execution_id);
    }
  }

  function drainContiguousEvents(): void {
    while (activeExecutionId && pendingEvents.has(lastSequence + 1)) {
      const event = pendingEvents.get(lastSequence + 1);
      if (!event) return;
      pendingEvents.delete(event.sequence);
      applyEvent(event);
    }
  }

  function receiveExecutionEvent(event: RuleExecutionEvent): void {
    if (event.execution_id !== activeExecutionId || event.sequence <= lastSequence) return;

    if (event.sequence > lastSequence + 1) {
      pendingEvents.set(event.sequence, event);
      if (!catchUpPromise) void catchUpExecution(event.execution_id);
      return;
    }

    applyEvent(event);
    drainContiguousEvents();
  }

  async function setupListeners(): Promise<void> {
    if (unlistenExecutionEvent) return;

    unlistenExecutionEvent = await listen<RuleExecutionEvent>(
      'rule-execution-event',
      (event: { payload: RuleExecutionEvent }) => receiveExecutionEvent(event.payload),
    );
  }

  function waitForTerminal(executionId: string): Promise<void> {
    if (terminalExecutionIds.has(executionId)) return Promise.resolve();

    const { promise, resolve } = Promise.withResolvers<void>();
    terminalWaiters.set(executionId, resolve);
    return promise;
  }

  function waitForCatchUpDelivery(executionId: string, throughSequence: number): Promise<void> {
    if (lastSequence >= throughSequence) return Promise.resolve();

    const { promise, resolve } = Promise.withResolvers<void>();
    catchUpDeliveryWaiter = { executionId, throughSequence, resolve };
    return promise;
  }

  function catchUpExecution(executionId: string): Promise<void> {
    if (executionId !== activeExecutionId) return Promise.resolve();
    if (catchUpPromise) return catchUpPromise;

    const afterSequence = lastSequence;
    const recovery = invoke<CatchUpExecutionResponse>('catch_up_execution', {
      request: { execution_id: executionId, after_sequence: afterSequence },
    })
      .then(async (receipt) => {
        if (receipt.execution_id !== executionId) {
          throw new Error('execution catch-up returned a different execution_id');
        }
        if (receipt.delivered_through_sequence < afterSequence) {
          throw new Error('execution catch-up regressed sequence');
        }
        await waitForCatchUpDelivery(executionId, receipt.delivered_through_sequence);
      })
      .finally(() => {
        if (catchUpPromise !== recovery) return;
        catchUpPromise = null;
        drainContiguousEvents();
        if (activeExecutionId === executionId && pendingEvents.size > 0) {
          void catchUpExecution(executionId);
        }
      });

    catchUpPromise = recovery;
    return recovery;
  }

  async function execute(request: ExecuteRequest): Promise<void> {
    await setupListeners();
    const response = await invoke<ExecuteResponse>('execute', { request });

    activeExecutionId = response.execution_id;
    lastSequence = 0;
    pendingEvents = new SvelteMap<number, RuleExecutionEvent>();
    terminalExecutionIds.delete(response.execution_id);
    await catchUpExecution(response.execution_id);
    await waitForTerminal(response.execution_id);
  }

  function cancel(): Promise<CancelExecutionResponse | null> {
    if (!activeExecutionId) return Promise.resolve(null);
    return invoke<CancelExecutionResponse>('cancel_execution', {
      request: { execution_id: activeExecutionId },
    });
  }

  function cleanup(): void {
    unlistenExecutionEvent?.();
    unlistenExecutionEvent = null;
    for (const resolve of terminalWaiters.values()) resolve();
    terminalWaiters.clear();
    catchUpDeliveryWaiter?.resolve();
    catchUpDeliveryWaiter = null;
  }

  return { execute, cancel, catchUpExecution, cleanup };
}
