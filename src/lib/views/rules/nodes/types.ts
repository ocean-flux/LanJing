//! 画布节点视图类型：跨切片共享的节点 data 契约。
//!
//! FlowAdapter（src/lib/rules/native-authoring/flow-adapter.ts）是
//! FlowViewNode/FlowViewEdge/FlowNodeData 的类型 owner，本模块只做 re-export，
//! 保证画布切片与 adapter 投影使用同一类型身份，避免漂移。
//! 纯 TS，无 runes、无 .svelte.ts。

import type { InstallDiagnostic } from '$lib/rules/native-authoring/wire';

/** 节点级诊断视图（与 InstallDiagnostic 同构）。 */
export type NodeDiagnosticView = InstallDiagnostic;

export type {
  FlowNodeData,
  FlowViewNode,
  FlowViewEdge,
  FlowEdgeData,
} from '$lib/rules/native-authoring/flow-adapter';
