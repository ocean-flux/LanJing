//! 规则升级算法 — 首刀 stub,返回 `ImportOutcome::New`。

use crate::preview::ImportOutcome;

/// 升级算法 stub:全量替换,提示用户编辑将丢失。
///
/// 首刀 stub,完整差异合并推迟到二次导入成为真实用户场景。
#[must_use]
pub fn upgrade_rule() -> ImportOutcome {
    ImportOutcome::New
}
