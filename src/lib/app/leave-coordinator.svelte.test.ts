import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  getLeaveCoordinatorSnapshot,
  interceptRouteLeave,
  interceptTauriCloseRequested,
  registerLeaveGuard,
  requestSurfaceClose,
  resetLeaveCoordinatorForTests,
  resolvePendingLeave,
  startTauriCloseRequested,
  type LeaveFocusTarget,
  type LeaveGuard,
  type LeaveResolution,
  type TauriCloseRequestEvent,
  type TauriCloseWindow,
} from './leave-coordinator.svelte';

afterEach(() => {
  resetLeaveCoordinatorForTests();
});

function dirtyGuard(
  resolveLeave: LeaveGuard['resolveLeave'] = () => 'resolved',
  focusEditor: LeaveGuard['focusEditor'] = undefined,
): LeaveGuard {
  return {
    canLeave: () => false,
    resolveLeave,
    focusEditor,
  };
}

function closeEvent() {
  return { preventDefault: vi.fn<() => void>() } satisfies TauriCloseRequestEvent;
}

describe('leave coordinator', () => {
  it('serializes repeated route intents and allows exactly one coordinator replay', async () => {
    let finishSave: ((resolution: LeaveResolution) => void) | undefined;
    const resolveLeave = vi.fn(
      () =>
        new Promise<LeaveResolution>((resolve) => {
          finishSave = resolve;
        }),
    );
    registerLeaveGuard(dirtyGuard(resolveLeave));

    const firstNavigation = { cancel: vi.fn(), willUnload: false };
    const repeatedNavigation = { cancel: vi.fn(), willUnload: false };
    const replayNavigation = { cancel: vi.fn(), willUnload: false };
    const replay = vi.fn(() => {
      expect(interceptRouteLeave(replayNavigation, vi.fn())).toBe('allow');
    });
    const ignoredReplay = vi.fn();

    expect(interceptRouteLeave(firstNavigation, replay)).toBe('pending');
    expect(interceptRouteLeave(repeatedNavigation, ignoredReplay)).toBe('ignored');
    expect(firstNavigation.cancel).toHaveBeenCalledOnce();
    expect(repeatedNavigation.cancel).toHaveBeenCalledOnce();

    const firstSave = resolvePendingLeave('save');
    const duplicateSave = resolvePendingLeave('save');
    expect(firstSave).toBe(duplicateSave);
    expect(resolveLeave).toHaveBeenCalledOnce();

    finishSave?.('resolved');
    await firstSave;

    expect(replay).toHaveBeenCalledOnce();
    expect(ignoredReplay).not.toHaveBeenCalled();
    expect(replayNavigation.cancel).not.toHaveBeenCalled();
    expect(getLeaveCoordinatorSnapshot()).toMatchObject({ open: false, phase: 'idle' });
  });

  it('keeps the pending intent and never replays when save is blocked', async () => {
    const replay = vi.fn();
    const resolveLeave = vi.fn<LeaveGuard['resolveLeave']>(() => 'blocked');
    registerLeaveGuard(dirtyGuard(resolveLeave));

    expect(requestSurfaceClose(replay)).toBe('pending');
    await expect(resolvePendingLeave('save')).resolves.toBe('blocked');

    expect(resolveLeave).toHaveBeenCalledOnce();
    expect(resolveLeave).toHaveBeenCalledWith('save');
    expect(replay).not.toHaveBeenCalled();
    expect(getLeaveCoordinatorSnapshot()).toMatchObject({
      open: true,
      phase: 'idle',
      error: 'guard_resolution_failed',
    });
  });

  it('continue closes the prompt without replay and restores the original focus target', async () => {
    const replay = vi.fn();
    const focus = vi.fn();
    const focusEditor = vi.fn();
    const target = { focus, isConnected: true } as LeaveFocusTarget;
    registerLeaveGuard(dirtyGuard(() => 'resolved', focusEditor));

    expect(requestSurfaceClose(replay, target)).toBe('pending');
    await expect(resolvePendingLeave('continue')).resolves.toBe('resolved');

    expect(replay).not.toHaveBeenCalled();
    expect(focus).toHaveBeenCalledOnce();
    expect(focusEditor).not.toHaveBeenCalled();
    expect(getLeaveCoordinatorSnapshot().open).toBe(false);
  });

  it('shares duplicate discard actions and replays the surface close once', async () => {
    const nestedNavigation = { cancel: vi.fn(), willUnload: false };
    const replay = vi.fn(() => {
      expect(interceptRouteLeave(nestedNavigation, vi.fn())).toBe('allow');
    });
    const resolveLeave = vi.fn<LeaveGuard['resolveLeave']>(() => Promise.resolve('resolved'));
    registerLeaveGuard(dirtyGuard(resolveLeave));
    requestSurfaceClose(replay);

    const firstDiscard = resolvePendingLeave('discard');
    const duplicateDiscard = resolvePendingLeave('discard');
    expect(firstDiscard).toBe(duplicateDiscard);
    await firstDiscard;

    expect(resolveLeave).toHaveBeenCalledOnce();
    expect(resolveLeave).toHaveBeenCalledWith('discard');
    expect(replay).toHaveBeenCalledOnce();
    expect(nestedNavigation.cancel).not.toHaveBeenCalled();
  });

  it('uses only the native browser prompt for willUnload navigation', () => {
    const replay = vi.fn();
    const resolveLeave = vi.fn<LeaveGuard['resolveLeave']>(() => 'resolved');
    const navigation = { cancel: vi.fn(), willUnload: true };
    registerLeaveGuard(dirtyGuard(resolveLeave));

    expect(interceptRouteLeave(navigation, replay)).toBe('native_prompt');

    expect(navigation.cancel).toHaveBeenCalledOnce();
    expect(replay).not.toHaveBeenCalled();
    expect(resolveLeave).not.toHaveBeenCalled();
    expect(getLeaveCoordinatorSnapshot().open).toBe(false);
  });

  it('prevents repeated Tauri close requests but saves and destroys the window once', async () => {
    let handler: ((event: TauriCloseRequestEvent) => void | Promise<void>) | undefined;
    const destroy = vi.fn(() => Promise.resolve());
    const unlisten = vi.fn();
    const appWindow: TauriCloseWindow = {
      destroy,
      onCloseRequested: vi.fn(async (nextHandler) => {
        handler = nextHandler;
        return unlisten;
      }),
    };
    const resolveLeave = vi.fn<LeaveGuard['resolveLeave']>(() => Promise.resolve('resolved'));
    registerLeaveGuard(dirtyGuard(resolveLeave));
    const stop = await startTauriCloseRequested(appWindow);

    const first = closeEvent();
    const repeated = closeEvent();
    handler?.(first);
    handler?.(repeated);

    expect(first.preventDefault).toHaveBeenCalledOnce();
    expect(repeated.preventDefault).toHaveBeenCalledOnce();
    await resolvePendingLeave('save');

    expect(resolveLeave).toHaveBeenCalledOnce();
    expect(resolveLeave).toHaveBeenCalledWith('save');
    expect(destroy).toHaveBeenCalledOnce();
    stop();
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it('keeps a Tauri close failure observable and does not auto-retry', async () => {
    let handler: ((event: TauriCloseRequestEvent) => void | Promise<void>) | undefined;
    const destroy = vi.fn(() => Promise.reject(new Error('host close failed')));
    const appWindow: TauriCloseWindow = {
      destroy,
      onCloseRequested: vi.fn(async (nextHandler) => {
        handler = nextHandler;
        return vi.fn();
      }),
    };
    registerLeaveGuard(dirtyGuard());
    await startTauriCloseRequested(appWindow);

    handler?.(closeEvent());
    await expect(resolvePendingLeave('discard')).resolves.toBe('blocked');

    expect(destroy).toHaveBeenCalledOnce();
    expect(getLeaveCoordinatorSnapshot()).toMatchObject({
      open: true,
      error: 'tauri_close_failed',
    });
  });

  it('records close handler and listener failures as typed observable errors', async () => {
    registerLeaveGuard(dirtyGuard());
    const brokenEvent = {
      preventDefault: () => {
        throw new Error('prevent failed');
      },
    };

    expect(interceptTauriCloseRequested(brokenEvent, vi.fn())).toBe('error');
    expect(getLeaveCoordinatorSnapshot().error).toBe('tauri_close_handler_failed');

    resetLeaveCoordinatorForTests();
    const brokenWindow: TauriCloseWindow = {
      destroy: vi.fn(() => Promise.resolve()),
      onCloseRequested: vi.fn(() => Promise.reject(new Error('listen failed'))),
    };
    await startTauriCloseRequested(brokenWindow);
    expect(getLeaveCoordinatorSnapshot().error).toBe('tauri_listener_failed');
  });
});
