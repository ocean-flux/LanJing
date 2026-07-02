//! Maccms10 视频源 importer 类型 — 输入采集 API URL + format。

use serde::{Deserialize, Serialize};

/// Maccms 采集 API format（`at` 参数）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MaccmsFormat {
    /// JSON 响应（`at=json`，默认）。
    #[default]
    Json,
    /// XML 响应（`at=xml`）。
    Xml,
}

/// Maccms10 视频源导入输入 — 采集 API URL + 可选 format。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaccmsSourceUrl {
    /// 采集 API URL（如 `https://example.com/api.php/provide/vod/`）。
    pub url: String,
    /// 响应 format（默认 `Json`，KTD2）。
    #[serde(default = "default_format")]
    pub at: MaccmsFormat,
}

fn default_format() -> MaccmsFormat {
    MaccmsFormat::Json
}
