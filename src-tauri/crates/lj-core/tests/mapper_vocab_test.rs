use lj_core::mapper_vocab::{
    BOOK_URL_TEMPLATE_VAR, CHAPTER_URL_TEMPLATE_VAR, DISCOVERY_MEDIA_IDENTITY_FIELDS,
    ITEM_IDENTITY_FIELDS, PLAY_URL_KEYS, UNIT_LOCATOR_KEYS,
};

#[test]
fn shared_vocab_exports_expected_aliases() {
    assert!(DISCOVERY_MEDIA_IDENTITY_FIELDS.contains(&"source_item_id"));
    assert!(ITEM_IDENTITY_FIELDS.contains(&"book_url"));
    assert!(UNIT_LOCATOR_KEYS.contains(&"chapterUrl"));
    assert!(PLAY_URL_KEYS.contains(&"playUrl"));
}

#[test]
fn shared_vocab_keeps_source_specific_raw_fields_outside_core() {
    assert!(!ITEM_IDENTITY_FIELDS.contains(&"vod_id"));
    assert!(!DISCOVERY_MEDIA_IDENTITY_FIELDS.contains(&"vod_name"));
    assert!(!DISCOVERY_MEDIA_IDENTITY_FIELDS.contains(&"vod_pic"));
}

#[test]
fn shared_vocab_template_vars_stay_stable() {
    assert_eq!(BOOK_URL_TEMPLATE_VAR, "bookUrl");
    assert_eq!(CHAPTER_URL_TEMPLATE_VAR, "chapterUrl");
}
