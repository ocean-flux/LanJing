//! Maccms 来源专属协议词汇。

/// Maccms10 采集端点的规范路径。
pub const API_PATH: &str = "/api.php/provide/vod";

/// 来源响应中的视频 ID 字段。
pub const VOD_ID_FIELD: &str = "vod_id";
/// 来源响应中的视频名称字段。
pub const VOD_NAME_FIELD: &str = "vod_name";
/// 来源响应中的封面字段。
pub const VOD_PIC_FIELD: &str = "vod_pic";
/// 来源响应中的详情正文片段字段。
pub const VOD_CONTENT_FIELD: &str = "vod_content";
/// 来源响应中的播放地址集合字段。
pub const VOD_PLAY_URL_FIELD: &str = "vod_play_url";
/// 来源响应中的播放线路集合字段。
pub const VOD_PLAY_FROM_FIELD: &str = "vod_play_from";
/// 来源响应中的分类名称字段。
pub const TYPE_NAME_FIELD: &str = "type_name";
/// 来源响应中的更新备注字段。
pub const VOD_REMARKS_FIELD: &str = "vod_remarks";

/// 发现结果生成稳定身份时按优先级尝试的中间字段。
pub const DISCOVERY_IDENTITY_FIELDS: &[&str] = &["source_item_id", "url", "title", "name"];
/// 媒体条目生成稳定身份时按优先级尝试的中间字段。
pub const ITEM_IDENTITY_FIELDS: &[&str] =
    &["source_item_id", "vod_id", "id", "url", "title", "name"];
/// 媒体单元生成稳定身份时按优先级尝试的中间字段。
pub const UNIT_IDENTITY_FIELDS: &[&str] = &["play_url", "source_unit_id", "title", "name"];
/// 媒体资产生成稳定身份时按优先级尝试的中间字段。
pub const ASSET_IDENTITY_FIELDS: &[&str] = &["play_url", "url", "href", "title", "name"];

/// 写入中性 mapper 的标题字段。
pub const NAME_FIELD: &str = "name";
/// 写入中性 mapper 的封面字段。
pub const COVER_FIELD: &str = "cover";
/// 写入标准 intent export 的来源视频 ID 字段。
pub const VOD_ID_EXPORT_FIELD: &str = "vodId";
/// 写入中性 mapper 的分类字段。
pub const KIND_FIELD: &str = "kind";
/// 写入中性 mapper 的更新备注字段。
pub const REMARKS_FIELD: &str = "remarks";
/// 写入中性 mapper 的详情字段。
pub const DESCRIPTION_FIELD: &str = "description";
/// 写入中性 mapper 的播放地址字段。
pub const PLAY_URL_FIELD: &str = "playUrl";
/// 写入中性 mapper 的播放线路字段。
pub const PLAY_FROM_FIELD: &str = "playFrom";
