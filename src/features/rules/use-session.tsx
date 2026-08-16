//! 编辑器 session 的 React 绑定。
//!
//! session store 是 vanilla zustand，本身不依赖 React。这里用 context 传递实例，
//! 组件通过选择器订阅自己关心的切片 —— 拖拽/连线这类高频变更不会重渲染整棵树。

import { createContext, useContext, useMemo, type ReactNode } from 'react';
import { useStore } from 'zustand';
import {
  createRuleEditorSession,
  type RuleEditorSession,
  type SessionStore,
} from './model/session';

const SessionContext = createContext<RuleEditorSession | null>(null);

/**
 * 提供一个编辑器 session。
 *
 * `key` 变化时由调用方重挂载 Provider，从而换一个干净的 session
 * （切换文档不应继承上一份撤销历史）。
 */
export function RuleEditorSessionProvider({
  children,
  session,
}: {
  children: ReactNode;
  session?: RuleEditorSession;
}) {
  const fallback = useMemo(() => createRuleEditorSession(), []);
  return <SessionContext.Provider value={session ?? fallback}>{children}</SessionContext.Provider>;
}

/** 取 session store 实例本身（调 action、或在 effect 里读快照）。 */
export function useRuleEditorSessionStore(): RuleEditorSession {
  const store = useContext(SessionContext);
  if (!store) {
    throw new Error('useRuleEditorSession 必须在 RuleEditorSessionProvider 内使用');
  }
  return store;
}

/** 按选择器订阅 session 切片。 */
export function useRuleEditorSession<T>(selector: (state: SessionStore) => T): T {
  return useStore(useRuleEditorSessionStore(), selector);
}
