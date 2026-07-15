//! 标准媒体模型与资源图增量。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write;

/// 来源内稳定资源 ID。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MediaResourceId(pub String);

/// 构造可逆的媒体主体资源 ID。
#[must_use]
pub fn item_resource_id(source_id: &MediaResourceId, source_key: &str) -> MediaResourceId {
    MediaResourceId(format!(
        "item:{}:{}",
        encode_resource_component(&source_id.0),
        encode_resource_component(source_key)
    ))
}

/// 构造可逆的消费单元资源 ID。
#[must_use]
pub fn unit_resource_id(
    source_id: &MediaResourceId,
    item_source_key: &str,
    unit_source_key: &str,
) -> MediaResourceId {
    MediaResourceId(format!(
        "unit:{}:{}:{}",
        encode_resource_component(&source_id.0),
        encode_resource_component(item_source_key),
        encode_resource_component(unit_source_key)
    ))
}

/// 解析媒体主体资源 ID。
#[must_use]
pub fn parse_item_resource_id(id: &str) -> Option<(MediaResourceId, String)> {
    let mut parts = id.strip_prefix("item:")?.splitn(2, ':');
    let source_id = decode_resource_component(parts.next()?)?;
    let source_key = decode_resource_component(parts.next()?)?;
    Some((MediaResourceId(source_id), source_key))
}

/// 解析消费单元资源 ID。
#[must_use]
pub fn parse_unit_resource_id(id: &str) -> Option<(MediaResourceId, String, String)> {
    let mut parts = id.strip_prefix("unit:")?.splitn(3, ':');
    let source_id = decode_resource_component(parts.next()?)?;
    let item_source_key = decode_resource_component(parts.next()?)?;
    let unit_source_key = decode_resource_component(parts.next()?)?;
    Some((MediaResourceId(source_id), item_source_key, unit_source_key))
}

#[must_use]
fn encode_resource_component(raw: &str) -> String {
    let trimmed = raw.trim();
    let value = if trimmed.is_empty() {
        "unknown"
    } else {
        trimmed
    };
    let mut out = String::with_capacity(value.len() * 2);
    for byte in value.as_bytes() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[must_use]
fn decode_resource_component(encoded: &str) -> Option<String> {
    if encoded.is_empty() || !encoded.len().is_multiple_of(2) {
        return None;
    }
    let mut bytes = Vec::with_capacity(encoded.len() / 2);
    for chunk in encoded.as_bytes().chunks(2) {
        let text = std::str::from_utf8(chunk).ok()?;
        let byte = u8::from_str_radix(text, 16).ok()?;
        bytes.push(byte);
    }
    String::from_utf8(bytes).ok()
}

/// 媒体类型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Text,
    Image,
    Comic,
    Audio,
    Video,
    Article,
    Document,
    Course,
    LiveReplay,
    Mixed,
    LocalResource,
    RemoteResource,
}

/// 资源补全状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceCompleteness {
    Partial,
    Complete,
    Unavailable,
    Restricted,
    Failed,
}

/// 来源资料。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceProfile {
    pub id: MediaResourceId,
    pub title: String,
    pub icon_url: Option<String>,
    pub version: Option<String>,
    pub supported_intents: Vec<lj_capability::StandardIntent>,
    pub risk_notes: Vec<String>,
}

/// 媒体主体。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaItem {
    pub id: MediaResourceId,
    pub source_id: MediaResourceId,
    pub media_kind: MediaKind,
    pub title: String,
    pub subtitle: Option<String>,
    pub creators: Vec<String>,
    pub description: Option<String>,
    pub cover_asset_id: Option<MediaResourceId>,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub completeness: ResourceCompleteness,
    pub updated_at: Option<String>,
}

/// 媒体集合。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaCollection {
    pub id: MediaResourceId,
    pub source_id: MediaResourceId,
    pub title: String,
    pub kind: String,
    pub item_ids: Vec<MediaResourceId>,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub completeness: ResourceCompleteness,
}

/// 可排序消费单元。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaUnit {
    pub id: MediaResourceId,
    pub source_id: MediaResourceId,
    pub item_id: MediaResourceId,
    pub title: String,
    pub position: Option<u32>,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub completeness: ResourceCompleteness,
}

/// 可消费资产。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAsset {
    pub id: MediaResourceId,
    pub source_id: MediaResourceId,
    pub unit_id: Option<MediaResourceId>,
    pub asset_kind: MediaAssetKind,
    pub locator: MediaAssetLocator,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub completeness: ResourceCompleteness,
}

/// 资产类型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaAssetKind {
    Text,
    Image,
    AudioStream,
    VideoStream,
    Subtitle,
    Cover,
    Attachment,
}

/// 资产定位。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum MediaAssetLocator {
    Text(String),
    Url(String),
    FilePath(String),
    Bytes(Vec<u8>),
    Unresolved,
}

/// 媒体关系。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaRelation {
    pub source_id: MediaResourceId,
    pub from_id: MediaResourceId,
    pub to_id: MediaResourceId,
    pub relation_kind: MediaRelationKind,
}

/// 关系类型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaRelationKind {
    Author,
    Series,
    Similar,
    Previous,
    Next,
    SameAlbum,
    SameChannel,
    SourceOrigin,
    Parent,
}

/// 可继续动作。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAction {
    pub id: MediaResourceId,
    pub source_id: MediaResourceId,
    pub label: String,
    pub intent: lj_capability::StandardIntent,
    pub payload: serde_json::Value,
}

/// 展示提示。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationHint {
    pub resource_id: MediaResourceId,
    pub card_density: Option<String>,
    pub cover_ratio: Option<String>,
    pub dominant_color: Option<String>,
    pub preferred_template: Option<String>,
}

/// 资源图增量。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaGraphDelta {
    pub sources: Vec<SourceProfile>,
    pub items: Vec<MediaItem>,
    pub collections: Vec<MediaCollection>,
    pub units: Vec<MediaUnit>,
    pub assets: Vec<MediaAsset>,
    pub relations: Vec<MediaRelation>,
    pub actions: Vec<MediaAction>,
    pub hints: Vec<PresentationHint>,
}

impl MediaGraphDelta {
    /// 合并同 ID 资源，后到资源覆盖同 ID 旧资源。
    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        merge_by_id(&mut self.sources, other.sources, |x| &x.id);
        merge_by_id(&mut self.items, other.items, |x| &x.id);
        merge_by_id(&mut self.collections, other.collections, |x| &x.id);
        merge_by_id(&mut self.units, other.units, |x| &x.id);
        merge_by_id(&mut self.assets, other.assets, |x| &x.id);
        for relation in other.relations {
            if !self.relations.contains(&relation) {
                self.relations.push(relation);
            }
        }
        merge_by_id(&mut self.actions, other.actions, |x| &x.id);
        merge_by_id(&mut self.hints, other.hints, |x| &x.resource_id);
        self
    }
}

fn merge_by_id<T, F>(left: &mut Vec<T>, right: Vec<T>, id: F)
where
    F: Fn(&T) -> &MediaResourceId,
{
    for item in right {
        if let Some(existing) = left.iter_mut().find(|x| id(x) == id(&item)) {
            *existing = item;
        } else {
            left.push(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_unit_asset_roundtrip() {
        let source_id = MediaResourceId("source:demo".to_string());
        let item_id = MediaResourceId("item:book:1".to_string());
        let unit_id = MediaResourceId("unit:book:1:1".to_string());
        let asset_id = MediaResourceId("asset:book:1:1:text".to_string());
        let item = MediaItem {
            id: item_id.clone(),
            source_id: source_id.clone(),
            media_kind: MediaKind::Text,
            title: "测试书".to_string(),
            subtitle: None,
            creators: vec!["作者".to_string()],
            description: None,
            cover_asset_id: None,
            metadata: BTreeMap::new(),
            completeness: ResourceCompleteness::Partial,
            updated_at: None,
        };
        let unit = MediaUnit {
            id: unit_id.clone(),
            source_id: source_id.clone(),
            item_id,
            title: "第一章".to_string(),
            position: Some(1),
            metadata: BTreeMap::new(),
            completeness: ResourceCompleteness::Complete,
        };
        let asset = MediaAsset {
            id: asset_id,
            source_id,
            unit_id: Some(unit_id),
            asset_kind: MediaAssetKind::Text,
            locator: MediaAssetLocator::Text("正文".to_string()),
            metadata: BTreeMap::new(),
            completeness: ResourceCompleteness::Complete,
        };
        let delta = MediaGraphDelta {
            items: vec![item],
            units: vec![unit],
            assets: vec![asset],
            ..MediaGraphDelta::default()
        };
        let json = serde_json::to_string(&delta).unwrap();
        let back: MediaGraphDelta = serde_json::from_str(&json).unwrap();
        assert_eq!(delta, back);
    }

    #[test]
    fn merge_enriches_by_id() {
        let id = MediaResourceId("item:1".to_string());
        let source_id = MediaResourceId("source:demo".to_string());
        let partial = MediaItem {
            id: id.clone(),
            source_id: source_id.clone(),
            media_kind: MediaKind::Video,
            title: "旧标题".to_string(),
            subtitle: None,
            creators: Vec::new(),
            description: None,
            cover_asset_id: None,
            metadata: BTreeMap::new(),
            completeness: ResourceCompleteness::Partial,
            updated_at: None,
        };
        let mut complete = partial.clone();
        complete.title = "新标题".to_string();
        complete.completeness = ResourceCompleteness::Complete;
        let merged = MediaGraphDelta {
            items: vec![partial],
            ..MediaGraphDelta::default()
        }
        .merge(MediaGraphDelta {
            items: vec![complete.clone()],
            ..MediaGraphDelta::default()
        });
        assert_eq!(merged.items, vec![complete]);
    }

    #[test]
    fn resource_id_roundtrip() {
        let source_id = MediaResourceId("source:test".to_string());
        let item_id = item_resource_id(&source_id, "/book/1");
        let unit_id = unit_resource_id(&source_id, "/book/1", "/read/1.html");

        assert_eq!(
            parse_item_resource_id(&item_id.0),
            Some((source_id.clone(), "/book/1".to_string()))
        );
        assert_eq!(
            parse_unit_resource_id(&unit_id.0),
            Some((source_id, "/book/1".to_string(), "/read/1.html".to_string(),))
        );
    }

    #[test]
    fn merge_deduplicates_relations_actions_and_hints() {
        let source_id = MediaResourceId("source:test".to_string());
        let item_id = MediaResourceId("item:test".to_string());
        let target_id = MediaResourceId("item:other".to_string());
        let relation = MediaRelation {
            source_id: source_id.clone(),
            from_id: item_id.clone(),
            to_id: target_id.clone(),
            relation_kind: MediaRelationKind::SourceOrigin,
        };
        let action = MediaAction {
            id: MediaResourceId("action:test".to_string()),
            source_id: source_id.clone(),
            label: "打开".to_string(),
            intent: lj_capability::StandardIntent::ContinueAction,
            payload: serde_json::json!({"key": "old"}),
        };
        let hint = PresentationHint {
            resource_id: item_id.clone(),
            card_density: Some("compact".to_string()),
            cover_ratio: None,
            dominant_color: None,
            preferred_template: None,
        };
        let mut updated_action = action.clone();
        updated_action.payload = serde_json::json!({"key": "new"});
        let merged = MediaGraphDelta {
            relations: vec![relation.clone()],
            actions: vec![action],
            hints: vec![hint.clone()],
            ..MediaGraphDelta::default()
        }
        .merge(MediaGraphDelta {
            relations: vec![relation],
            actions: vec![updated_action.clone()],
            hints: vec![hint],
            ..MediaGraphDelta::default()
        });

        assert_eq!(merged.relations.len(), 1);
        assert_eq!(merged.actions, vec![updated_action]);
        assert_eq!(merged.hints.len(), 1);
    }
}
