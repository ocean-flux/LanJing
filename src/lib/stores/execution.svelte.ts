//! 执行状态 store — 管理按段执行的状态
//!
//! 监听 rule-output / node-output 事件，按端串联：search → 选书 → detail+toc → 选章 → content。
//! 字段名与 Rust serde 序列化一致（snake_case）。

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

/** 图书媒体（与 Rust `BookMedia` 对应）。 */
export interface BookMedia {
  title: string;
  author: string | null;
  cover_url: string | null;
  description: string | null;
  kind: string | null;
  last_chapter: string | null;
  book_url: string | null;
  chapters: Chapter[];
}

/** 图书章节（与 Rust `BookChapter` 对应）。 */
export interface Chapter {
  title: string;
  chapter_url: string;
  content: string | null;
}

/** 节点输出摘要（与 Rust `NodeDataSummary` 对应）。 */
export interface NodeOutput {
  node_id: string;
  variant: string;
  summary: string;
}

// ===== 状态 =====

let books = $state<BookMedia[]>([]);
let rawContent = $state<string | null>(null);
let nodeOutputs = $state<NodeOutput[]>([]);
let loading = $state(false);
let error = $state<string | null>(null);
let currentSegment = $state<'search' | 'detail_toc' | 'content' | null>(null);
let selectedBook = $state<BookMedia | null>(null);
let selectedChapter = $state<Chapter | null>(null);

// event listeners
let unlistenRuleOutput: UnlistenFn | null = null;
let unlistenNodeOutput: UnlistenFn | null = null;
let unlistenRuleError: UnlistenFn | null = null;

// ===== 内部逻辑 =====

/** 重置所有状态回到初始。 */
function reset(): void {
  books = [];
  rawContent = null;
  nodeOutputs = [];
  error = null;
  currentSegment = null;
  selectedBook = null;
  selectedChapter = null;
}

/** 设置双 event listener。 */
async function setupListeners(): Promise<void> {
  // node-output 泛型用 unknown 绕过深度嵌套 enum 类型
  unlistenRuleOutput = await listen<unknown>('rule-output', (e: { payload: unknown }) => {
    const p = e.payload as Record<string, unknown>;
    // 提取 NodeData::Media(Media::Book(...))
    if (p && typeof p === 'object') {
      const mediaVal = p['Media'];
      if (mediaVal && typeof mediaVal === 'object') {
        const bookVal = (mediaVal as Record<string, unknown>)['Book'];
        if (bookVal) {
          books.push(bookVal as BookMedia);
          return;
        }
      }
      // NodeData::Raw(...) — content 段正文
      const rawVal = p['Raw'];
      if (typeof rawVal === 'string') {
        rawContent = (rawContent ?? '') + rawVal;
        return;
      }
    }
  });

  unlistenNodeOutput = await listen<NodeOutput>('node-output', (e: { payload: NodeOutput }) => {
    nodeOutputs.push(e.payload);
  });

  unlistenRuleError = await listen<string>('rule-error', (e: { payload: string }) => {
    error = e.payload;
  });
}

/** 清理 event listeners。 */
function teardownListeners(): void {
  unlistenRuleOutput?.();
  unlistenNodeOutput?.();
  unlistenRuleError?.();
  unlistenRuleOutput = null;
  unlistenNodeOutput = null;
  unlistenRuleError = null;
}

/** 执行一段，统一错误处理。 */
async function runSegment(params: {
  ruleId: string;
  endpointKind: string;
  query?: string | null;
  bookUrl?: string | null;
  chapterUrl?: string | null;
}): Promise<void> {
  try {
    await invoke('execute_segment', {
      request: {
        rule_id: params.ruleId,
        endpoint_kind: params.endpointKind,
        query: params.query ?? null,
        book_url: params.bookUrl ?? null,
        chapter_url: params.chapterUrl ?? null,
      },
    });
  } catch (e) {
    error = String(e);
    throw e;
  }
}

// ===== 公开 API =====

/** 通用段启动器（搜索/发现）。 */
async function startSegment(
  kind: 'Search' | 'Discover',
  ruleId: string,
  query?: string,
): Promise<void> {
  reset();
  loading = true;
  currentSegment = 'search';
  error = null;

  await setupListeners();

  try {
    await runSegment({ ruleId, endpointKind: kind, query: query ?? null });
  } catch {
    // error 已在 runSegment 中设置
  } finally {
    loading = false;
  }
}

/** 启动搜索段。 */
export async function startSearch(ruleId: string, query: string): Promise<void> {
  return startSegment('Search', ruleId, query);
}

/** 启动发现/分类浏览段。 */
export async function startDiscover(ruleId: string): Promise<void> {
  return startSegment('Discover', ruleId);
}

/** 选中一本书，自动推进 detail + toc。 */
export async function selectBook(book: BookMedia, ruleId: string): Promise<void> {
  selectedBook = book;
  // 停止接收更多搜索结果
  teardownListeners();
  books = [];
  rawContent = null;
  nodeOutputs = [];
  error = null;
  loading = true;
  currentSegment = 'detail_toc';

  try {
    await Promise.all([
      runSegment({ ruleId, endpointKind: 'Detail', bookUrl: book.book_url }),
      runSegment({ ruleId, endpointKind: 'Toc', bookUrl: book.book_url }),
    ]);
  } catch {
    // error 已在 runSegment 中设置
  } finally {
    loading = false;
  }
}

/** 选中一个章节，拉取正文。 */
export async function selectChapter(chapter: Chapter, ruleId: string): Promise<void> {
  selectedChapter = chapter;
  rawContent = null;
  error = null;
  loading = true;
  currentSegment = 'content';

  try {
    await runSegment({ ruleId, endpointKind: 'Content', chapterUrl: chapter.chapter_url });
  } catch {
    // error 已在 runSegment 中设置
  } finally {
    loading = false;
  }
}

/** 退出当前段，返回上一级。 */
export function goBack(): void {
  if (currentSegment === 'content') {
    teardownListeners();
    selectedChapter = null;
    rawContent = null;
    currentSegment = 'detail_toc';
  } else if (currentSegment === 'detail_toc') {
    selectedBook = null;
    currentSegment = 'search';
    // 重新设置 listener 以接收新搜索结果
    setupListeners().catch(() => {});
  }
}

/** 组件销毁时调用。 */
export function cleanup(): void {
  teardownListeners();
}

// ===== Getters =====

export function getBooks(): BookMedia[] {
  return books;
}
export function getRawContent(): string | null {
  return rawContent;
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
export function getCurrentSegment(): 'search' | 'detail_toc' | 'content' | null {
  return currentSegment;
}
export function getSelectedBook(): BookMedia | null {
  return selectedBook;
}
export function getSelectedChapter(): Chapter | null {
  return selectedChapter;
}
