import type { BeforeNavigate } from '@sveltejs/kit';
import { tick } from 'svelte';

export type LeaveAction = 'save' | 'discard' | 'continue';
export type LeaveResolution = 'resolved' | 'blocked';
export type LeaveIntentKind = 'route' | 'surface' | 'window';
export type LeaveCoordinatorPhase = 'idle' | 'resolving';
export type LeaveCoordinatorErrorCode =
  | 'guard_check_failed'
  | 'guard_resolution_failed'
  | 'leave_replay_failed'
  | 'focus_restore_failed'
  | 'tauri_listener_failed'
  | 'tauri_listener_cleanup_failed'
  | 'tauri_close_handler_failed'
  | 'tauri_close_failed';

export type LeaveCoordinatorSnapshot = Readonly<{
  open: boolean;
  kind: LeaveIntentKind | null;
  phase: LeaveCoordinatorPhase;
  action: LeaveAction | null;
  error: LeaveCoordinatorErrorCode | null;
}>;

export interface LeaveGuard {
  canLeave(): boolean;
  resolveLeave(action: LeaveAction): LeaveResolution | Promise<LeaveResolution>;
  focusEditor?(): void;
}

export type LeaveReplay = () => void | Promise<void>;
export type LeaveFocusTarget = Pick<HTMLElement, 'focus' | 'isConnected'>;
export type LeaveRequestOutcome = 'replayed' | 'pending' | 'ignored';
export type RouteLeaveOutcome = 'allow' | 'native_prompt' | 'pending' | 'ignored';

export type TauriCloseRequestEvent = {
  preventDefault(): void;
};

export type TauriCloseWindow = {
  onCloseRequested(
    handler: (event: TauriCloseRequestEvent) => void | Promise<void>,
  ): Promise<() => void>;
  destroy(): Promise<void>;
};

type GuardRegistration = {
  token: symbol;
  guard: LeaveGuard;
};

type PendingLeaveIntent = {
  generation: number;
  kind: LeaveIntentKind;
  replay: LeaveReplay;
  focusTarget: LeaveFocusTarget | null;
  registration: GuardRegistration;
};

let registration: GuardRegistration | null = null;
let pendingIntent = $state.raw<PendingLeaveIntent | null>(null);
let phase = $state<LeaveCoordinatorPhase>('idle');
let resolvingAction = $state<LeaveAction | null>(null);
let error = $state<LeaveCoordinatorErrorCode | null>(null);
let resolutionPromise: Promise<LeaveResolution> | null = null;
let routeReplayAllowance = false;
let generation = 0;

/** 壳层离开门禁的只读响应式快照；不复制业务 session 状态。 */
export function getLeaveCoordinatorSnapshot(): LeaveCoordinatorSnapshot {
  return {
    open: pendingIntent !== null,
    kind: pendingIntent?.kind ?? null,
    phase,
    action: resolvingAction,
    error,
  };
}

/** 注册当前页面唯一离开门禁；返回的清理函数只注销本次 owner。 */
export function registerLeaveGuard(guard: LeaveGuard): () => void {
  if (registration) throw new Error('leave_guard_already_registered');

  const current: GuardRegistration = { token: Symbol('leave-guard'), guard };
  registration = current;

  return () => {
    if (registration?.token !== current.token) return;
    registration = null;
    generation += 1;
    if (pendingIntent?.registration.token === current.token) pendingIntent = null;
    resolutionPromise = null;
    routeReplayAllowance = false;
    error = null;
    phase = 'idle';
    resolvingAction = null;
  };
}

function setError(code: LeaveCoordinatorErrorCode): void {
  error = code;
}

function currentGuardBlocksLeave(): boolean {
  if (pendingIntent || resolutionPromise) return true;
  if (!registration) return false;

  try {
    return !registration.guard.canLeave();
  } catch {
    setError('guard_check_failed');
    return true;
  }
}

function queuePendingIntent(
  kind: LeaveIntentKind,
  replay: LeaveReplay,
  focusTarget: LeaveFocusTarget | null,
): LeaveRequestOutcome {
  if (pendingIntent || resolutionPromise) return 'ignored';
  if (!registration) return 'ignored';

  const keepGuardError = error === 'guard_check_failed';
  pendingIntent = {
    generation,
    kind,
    replay,
    focusTarget,
    registration,
  };
  if (!keepGuardError) error = null;
  return 'pending';
}

async function restoreAvailableFocus(
  focusTarget: LeaveFocusTarget | null,
  guard: LeaveGuard | undefined,
  expectedGeneration: number,
): Promise<void> {
  await tick();
  if (expectedGeneration !== generation) return;

  if (focusTarget?.isConnected !== false) {
    try {
      focusTarget?.focus();
      if (focusTarget) return;
    } catch {
      // 触发点失效后回退到 editor owner。
    }
  }

  try {
    if (guard?.focusEditor) {
      guard.focusEditor();
      return;
    }
  } catch {
    setError('focus_restore_failed');
    return;
  }

  if (focusTarget) setError('focus_restore_failed');
}

function restoreFocus(intent: PendingLeaveIntent): Promise<void> {
  return restoreAvailableFocus(intent.focusTarget, intent.registration.guard, intent.generation);
}

function replayWithoutPrompt(kind: LeaveIntentKind, replay: LeaveReplay): LeaveRequestOutcome {
  try {
    const replayResult = replay();
    void Promise.resolve(replayResult).then(undefined, () => {
      setError(kind === 'window' ? 'tauri_close_failed' : 'leave_replay_failed');
    });
  } catch {
    setError(kind === 'window' ? 'tauri_close_failed' : 'leave_replay_failed');
  }
  return 'replayed';
}

function requestLeave(
  kind: LeaveIntentKind,
  replay: LeaveReplay,
  focusTarget: LeaveFocusTarget | null,
): LeaveRequestOutcome {
  if (pendingIntent || resolutionPromise) return 'ignored';
  if (!currentGuardBlocksLeave()) return replayWithoutPrompt(kind, replay);
  return queuePendingIntent(kind, replay, focusTarget);
}

/** 工作区 Sheet/面板关闭入口；业务层只提供原动作与可选触发点。 */
export function requestSurfaceClose(
  replay: LeaveReplay,
  focusTarget: LeaveFocusTarget | null = null,
): LeaveRequestOutcome {
  return requestLeave('surface', replay, focusTarget);
}

/**
 * ModeShell 的 beforeNavigate 适配器。
 * willUnload 只调用 SvelteKit cancel 触发浏览器原生提示，不创建三动作 Dialog。
 */
export function interceptRouteLeave(
  navigation: Pick<BeforeNavigate, 'cancel' | 'willUnload'>,
  replay: LeaveReplay,
  focusTarget: LeaveFocusTarget | null = null,
): RouteLeaveOutcome {
  if (routeReplayAllowance) {
    routeReplayAllowance = false;
    return 'allow';
  }
  if (!currentGuardBlocksLeave()) return 'allow';

  navigation.cancel();
  if (navigation.willUnload) return 'native_prompt';

  const outcome = queuePendingIntent('route', replay, focusTarget);
  return outcome === 'pending' ? 'pending' : 'ignored';
}

function intentStillOwned(intent: PendingLeaveIntent): boolean {
  return intent.generation === generation && registration?.token === intent.registration.token;
}

async function resolveIntent(
  intent: PendingLeaveIntent,
  action: LeaveAction,
): Promise<LeaveResolution> {
  let resolution: LeaveResolution;
  try {
    resolution = await intent.registration.guard.resolveLeave(action);
  } catch {
    resolution = 'blocked';
  }

  if (!intentStillOwned(intent)) return 'blocked';
  if (resolution !== 'resolved') {
    setError('guard_resolution_failed');
    return 'blocked';
  }

  if (action === 'continue') {
    pendingIntent = null;
    error = null;
    await restoreFocus(intent);
    return 'resolved';
  }

  pendingIntent = null;
  error = null;
  await tick();
  if (!intentStillOwned(intent)) return 'blocked';

  // surface close 也可能通过 goto 完成；仅放行本次 coordinator replay 触发的一次 route。
  routeReplayAllowance = true;
  try {
    await intent.replay();
  } catch {
    routeReplayAllowance = false;
    if (intentStillOwned(intent)) {
      pendingIntent = intent;
      setError(intent.kind === 'window' ? 'tauri_close_failed' : 'leave_replay_failed');
    }
    return 'blocked';
  } finally {
    routeReplayAllowance = false;
  }

  return 'resolved';
}

/** Dialog 三动作入口；并发点击共享同一 promise，保证 save/discard/replay 各一次。 */
export function resolvePendingLeave(action: LeaveAction): Promise<LeaveResolution> {
  if (resolutionPromise) return resolutionPromise;
  const intent = pendingIntent;
  if (!intent) return Promise.resolve('blocked');

  phase = 'resolving';
  resolvingAction = action;
  error = null;
  const task = resolveIntent(intent, action).finally(() => {
    if (resolutionPromise !== task) return;
    resolutionPromise = null;
    phase = 'idle';
    resolvingAction = null;
  });
  resolutionPromise = task;
  return task;
}

/** Tauri closeRequested 适配器；dirty 时同步 preventDefault，再交由共享 Dialog。 */
export function interceptTauriCloseRequested(
  event: TauriCloseRequestEvent,
  replay: LeaveReplay,
): LeaveRequestOutcome | 'allow' | 'error' {
  if (!currentGuardBlocksLeave()) return 'allow';

  try {
    event.preventDefault();
  } catch {
    setError('tauri_close_handler_failed');
    return 'error';
  }

  return queuePendingIntent('window', replay, null);
}

/** 注册当前 Tauri 窗口关闭事件；target 参数仅用于最窄边界测试。 */
export async function startTauriCloseRequested(target?: TauriCloseWindow): Promise<() => void> {
  try {
    const appWindow =
      target ??
      ((await import('@tauri-apps/api/window')).getCurrentWindow() as unknown as TauriCloseWindow);
    const unlisten = await appWindow.onCloseRequested((event) => {
      interceptTauriCloseRequested(event, () => appWindow.destroy());
    });

    return () => {
      try {
        unlisten();
      } catch {
        setError('tauri_listener_cleanup_failed');
      }
    };
  } catch {
    setError('tauri_listener_failed');
    return () => undefined;
  }
}

/** 仅供邻接测试隔离模块单例；产品代码不得调用。 */
export function resetLeaveCoordinatorForTests(): void {
  generation += 1;
  registration = null;
  pendingIntent = null;
  phase = 'idle';
  resolvingAction = null;
  error = null;
  resolutionPromise = null;
  routeReplayAllowance = false;
}
