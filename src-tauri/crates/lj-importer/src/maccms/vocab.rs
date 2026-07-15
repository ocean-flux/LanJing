//! Maccms 来源专属协议词汇。

pub const API_PATH: &str = "/api.php/provide/vod";
pub const LIST_ACTION: &str = "list";
pub const DETAIL_ACTION: &str = "detail";
pub const ACTION_QUERY_KEY: &str = "ac";
pub const IDS_QUERY_KEY: &str = "ids";
pub const VOD_ID_PLACEHOLDER: &str = "{{vod_id}}";

pub const VOD_ID_FIELD: &str = "vod_id";
pub const VOD_NAME_FIELD: &str = "vod_name";
pub const VOD_PIC_FIELD: &str = "vod_pic";
pub const VOD_CONTENT_FIELD: &str = "vod_content";
pub const VOD_PLAY_URL_FIELD: &str = "vod_play_url";
pub const VOD_PLAY_FROM_FIELD: &str = "vod_play_from";
pub const TYPE_NAME_FIELD: &str = "type_name";
pub const VOD_REMARKS_FIELD: &str = "vod_remarks";

pub const DISCOVERY_IDENTITY_FIELDS: &[&str] = &["source_item_id", "url", "title", "name"];
pub const ITEM_IDENTITY_FIELDS: &[&str] =
    &["source_item_id", "vod_id", "id", "url", "title", "name"];
pub const UNIT_IDENTITY_FIELDS: &[&str] = &["play_url", "source_unit_id", "title", "name"];
pub const ASSET_IDENTITY_FIELDS: &[&str] = &["play_url", "url", "href", "title", "name"];

pub const NAME_FIELD: &str = "name";
pub const COVER_FIELD: &str = "cover";
pub const VOD_ID_EXPORT_FIELD: &str = "vodId";
pub const KIND_FIELD: &str = "kind";
pub const REMARKS_FIELD: &str = "remarks";
pub const DESCRIPTION_FIELD: &str = "description";
pub const PLAY_URL_FIELD: &str = "playUrl";
pub const PLAY_FROM_FIELD: &str = "playFrom";
