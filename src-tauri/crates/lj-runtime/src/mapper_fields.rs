//! runtime mapper 字段回退 helper。

use lj_core::mapper_vocab::{
    ASSET_CONTENT_KEYS, ASSET_URL_KEYS, DISCOVERY_ACTION_KEYS, DISCOVERY_TITLE_KEYS,
    ITEM_COVER_KEYS, ITEM_CREATOR_KEYS, ITEM_DESCRIPTION_KEYS, ITEM_SOURCE_KEY_KEYS,
    ITEM_TITLE_KEYS, ITEM_URL_KEYS, PLAY_FROM_KEYS, PLAY_URL_KEYS, UNIT_LOCATOR_KEYS,
    UNIT_TITLE_KEYS,
};
use lj_core::media::MediaKind;
use serde_json::Value;

const LOOKS_LIKE_MEDIA_KEYS: &[&str] = &[
    "source_item_id",
    "cover_url",
    "cover",
    "author",
    "description",
    "remarks",
];
const VIDEO_HINT_KEYS: &[&str] = &["play_url", "playUrl", "source_item_id", "vod_id", "vodId"];
const ITEM_SOURCE_KEY_EXT_KEYS: &[&str] = &["vod_id", "vodId"];
const PAYLOAD_ITEM_KEY_KEYS: &[&str] =
    &["vod_id", "vodId", "source_item_id", "id", "url", "book_url"];

#[must_use]
pub(crate) fn text_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(ToString::to_string)
    })
}

#[must_use]
pub(crate) fn discovery_title(value: &Value) -> Option<String> {
    text_field(value, DISCOVERY_TITLE_KEYS)
}

#[must_use]
pub(crate) fn discovery_target(value: &Value) -> Option<String> {
    text_field(value, DISCOVERY_ACTION_KEYS)
}

#[must_use]
pub(crate) fn item_url(value: &Value) -> Option<String> {
    text_field(value, ITEM_URL_KEYS)
}

#[must_use]
pub(crate) fn item_creators(value: &Value) -> Option<String> {
    text_field(value, ITEM_CREATOR_KEYS)
}

#[must_use]
pub(crate) fn item_cover(value: &Value) -> Option<String> {
    text_field(value, ITEM_COVER_KEYS)
}

#[must_use]
pub(crate) fn item_title(value: &Value) -> Option<String> {
    text_field(value, ITEM_TITLE_KEYS)
}

#[must_use]
pub(crate) fn item_description(value: &Value) -> Option<String> {
    text_field(value, ITEM_DESCRIPTION_KEYS)
}

#[must_use]
pub(crate) fn unit_title(value: &Value) -> Option<String> {
    text_field(value, UNIT_TITLE_KEYS)
}

#[must_use]
pub(crate) fn unit_locator(value: &Value) -> Option<String> {
    text_field(value, UNIT_LOCATOR_KEYS)
}

#[must_use]
pub(crate) fn asset_content(value: &Value) -> Option<String> {
    text_field(value, ASSET_CONTENT_KEYS)
}

#[must_use]
pub(crate) fn asset_url(value: &Value) -> Option<String> {
    text_field(value, ASSET_URL_KEYS)
}

#[must_use]
pub(crate) fn play_url(value: &Value) -> Option<String> {
    text_field(value, PLAY_URL_KEYS)
}

#[must_use]
pub(crate) fn play_from(value: &Value) -> Option<String> {
    text_field(value, PLAY_FROM_KEYS)
}

#[must_use]
pub(crate) fn looks_like_media_record(value: &Value) -> bool {
    text_field(value, LOOKS_LIKE_MEDIA_KEYS).is_some()
}

#[must_use]
pub(crate) fn media_kind(value: &Value) -> MediaKind {
    if text_field(value, VIDEO_HINT_KEYS).is_some() {
        MediaKind::Video
    } else {
        MediaKind::Text
    }
}

#[must_use]
pub(crate) fn item_source_key(value: &Value) -> Option<String> {
    text_field(value, ITEM_SOURCE_KEY_KEYS).or_else(|| text_field(value, ITEM_SOURCE_KEY_EXT_KEYS))
}

#[must_use]
pub(crate) fn payload_item_key(value: &Value) -> Option<String> {
    text_field(value, PAYLOAD_ITEM_KEY_KEYS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prefers_neutral_item_url_before_source_url() {
        let value = json!({
            "source_url": "https://fallback.example",
            "book_url": "https://book.example",
            "url": "https://item.example"
        });
        assert_eq!(item_url(&value).as_deref(), Some("https://item.example"));
    }

    #[test]
    fn unit_locator_keeps_chapter_url_fallbacks() {
        let value = json!({ "chapterUrl": "chapter-2" });
        assert_eq!(unit_locator(&value).as_deref(), Some("chapter-2"));
    }

    #[test]
    fn media_kind_still_marks_maccms_records_as_video() {
        let value = json!({ "vod_id": "140789" });
        assert_eq!(media_kind(&value), MediaKind::Video);
    }

    #[test]
    fn payload_item_key_accepts_maccms_and_neutral_keys() {
        let value = json!({ "vodId": "42" });
        assert_eq!(payload_item_key(&value).as_deref(), Some("42"));
    }
}
