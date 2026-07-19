//! 受控 Mapper：把来源中间记录收敛为标准媒体资源图增量。

use std::collections::BTreeMap;
use std::fmt::Write;

use lj_capability::{IntentInput, StandardIntent};
use lj_media::{
    MediaAction, MediaAsset, MediaAssetKind, MediaAssetLocator, MediaCollection, MediaGraphDelta,
    MediaItem, MediaResourceId, MediaUnit, ResourceCompleteness, SourceProfile, item_resource_id,
    parse_item_resource_id, parse_unit_resource_id, unit_resource_id,
};
use lj_rule_model::{ControlledMapper, definition::MapperOutputKind};
use serde_json::Value;

use crate::mapper_fields::{
    asset_content, asset_url, discovery_target, discovery_title, item_cover, item_creators,
    item_description, item_source_key, item_title, item_url, looks_like_media_record, media_kind,
    payload_item_key, play_from, play_url, text_field, unit_locator, unit_title,
};

/// 单次执行中的受控映射上下文。
#[derive(Clone)]
pub(crate) struct MapperContext {
    source_id: MediaResourceId,
    source_title: String,
    supported_intents: Vec<StandardIntent>,
}

impl MapperContext {
    /// 从 immutable Plan 的已安装来源元数据创建 mapper 上下文。
    ///
    /// 此构造器不读取旧 `Graph`，使 Plan runtime 能复用同一受控媒体映射语义。
    #[must_use]
    pub(crate) fn for_plan(
        source_id: MediaResourceId,
        source_title: String,
        supported_intents: Vec<StandardIntent>,
    ) -> Self {
        Self {
            source_id,
            source_title,
            supported_intents,
        }
    }

    /// 将 Plan 中的 `ControlledMapper` 和 JSON 中间记录转换为标准媒体增量。
    #[must_use]
    pub(crate) fn map_plan_json(
        &self,
        mapper: &ControlledMapper,
        intent: StandardIntent,
        input: &IntentInput,
        value: &Value,
    ) -> MediaGraphDelta {
        self.map_json(mapper.output, intent, input, value)
    }

    #[must_use]
    fn map_json(
        &self,
        output: MapperOutputKind,
        intent: StandardIntent,
        input: &IntentInput,
        value: &Value,
    ) -> MediaGraphDelta {
        match output {
            MapperOutputKind::Items => {
                let completeness = if intent == StandardIntent::ResolveItem {
                    ResourceCompleteness::Complete
                } else {
                    ResourceCompleteness::Partial
                };
                self.map_item_records(value, input, completeness)
            }
            MapperOutputKind::Discovery => self.map_discovery_records(value, input),
            MapperOutputKind::Units => self.map_unit_records(value, input),
            MapperOutputKind::Assets => self.map_asset_records(value, input),
        }
    }

    #[must_use]
    fn map_discovery_records(&self, value: &Value, input: &IntentInput) -> MediaGraphDelta {
        let records = records(value);
        let has_media_records = records.iter().any(|record| looks_like_media_record(record));
        if has_media_records {
            let mut delta = self.map_item_records(value, input, ResourceCompleteness::Partial);
            let item_ids = delta.items.iter().map(|item| item.id.clone()).collect();
            delta.collections.push(MediaCollection {
                id: MediaResourceId(format!("collection:{}:discover", self.source_id.0)),
                source_id: self.source_id.clone(),
                title: "发现".to_string(),
                kind: "discover".to_string(),
                item_ids,
                metadata: BTreeMap::new(),
                completeness: ResourceCompleteness::Partial,
            });
            return delta;
        }

        let mut delta = MediaGraphDelta {
            sources: vec![self.source_profile()],
            ..MediaGraphDelta::default()
        };
        for (index, record) in records.iter().enumerate() {
            let Some(title) = discovery_title(record) else {
                continue;
            };
            let action_id = MediaResourceId(format!(
                "action:{}:{}",
                self.source_id.0,
                id_component(&format!("{index}:{title}"))
            ));
            if discovery_target(record).is_some() {
                delta.actions.push(MediaAction {
                    id: action_id,
                    source_id: self.source_id.clone(),
                    label: title,
                    intent: StandardIntent::ContinueAction,
                    payload: (*record).clone(),
                });
            } else {
                delta.collections.push(MediaCollection {
                    id: MediaResourceId(format!(
                        "collection:{}:{}",
                        self.source_id.0,
                        id_component(&format!("{index}:{title}"))
                    )),
                    source_id: self.source_id.clone(),
                    title,
                    kind: "section".to_string(),
                    item_ids: Vec::new(),
                    metadata: metadata_from(record),
                    completeness: ResourceCompleteness::Partial,
                });
            }
        }
        delta
    }

    #[must_use]
    fn map_item_records(
        &self,
        value: &Value,
        input: &IntentInput,
        completeness: ResourceCompleteness,
    ) -> MediaGraphDelta {
        let mut delta = MediaGraphDelta {
            sources: vec![self.source_profile()],
            ..MediaGraphDelta::default()
        };
        for record in records(value) {
            let item_id = self.item_id(record, input);
            let mut metadata = metadata_from(record);
            let item_url = item_url(record);
            if let Some(url) = item_url.clone() {
                metadata.insert("url".to_string(), Value::String(url));
            }
            let creators = item_creators(record)
                .map(|author| vec![author])
                .unwrap_or_default();
            let cover_asset_id = item_cover(record).map(|cover| {
                let id = MediaResourceId(format!("asset:{}:cover", item_id.0));
                delta.assets.push(MediaAsset {
                    id: id.clone(),
                    source_id: self.source_id.clone(),
                    unit_id: None,
                    asset_kind: MediaAssetKind::Cover,
                    locator: MediaAssetLocator::Url(cover),
                    metadata: BTreeMap::new(),
                    completeness: ResourceCompleteness::Partial,
                });
                id
            });
            delta.items.push(MediaItem {
                id: item_id.clone(),
                source_id: self.source_id.clone(),
                media_kind: media_kind(record),
                title: item_title(record).unwrap_or_else(|| "未知".to_string()),
                subtitle: text_field(record, &["subtitle", "remarks"]),
                creators,
                description: item_description(record),
                cover_asset_id,
                metadata,
                completeness,
                updated_at: None,
            });
            self.append_play_delta(record, &item_id, &mut delta);
        }
        delta
    }

    #[must_use]
    fn map_unit_records(&self, value: &Value, input: &IntentInput) -> MediaGraphDelta {
        let mut delta = MediaGraphDelta {
            sources: vec![self.source_profile()],
            ..MediaGraphDelta::default()
        };
        let item_id = item_id_from_input(input)
            .or_else(|| item_id_from_payload(input, &self.source_id))
            .unwrap_or_else(|| self.item_id(value, input));
        let item_source_key =
            item_source_key_from_id(&item_id).unwrap_or_else(|| item_id.0.clone());
        let mut emitted_play_units = false;
        for record in records(value) {
            let before = delta.units.len();
            self.append_play_delta(record, &item_id, &mut delta);
            emitted_play_units |= delta.units.len() > before;
        }
        if emitted_play_units {
            delta.assets.clear();
            return delta;
        }
        for (index, record) in records(value).iter().enumerate() {
            let title = unit_title(record).unwrap_or_else(|| "未知".to_string());
            let key = unit_locator(record).unwrap_or_else(|| format!("{index}:{title}"));
            delta.units.push(MediaUnit {
                id: unit_resource_id(&self.source_id, &item_source_key, &key),
                source_id: self.source_id.clone(),
                item_id: item_id.clone(),
                title,
                position: u32::try_from(index + 1).ok(),
                metadata: metadata_from(record),
                completeness: ResourceCompleteness::Partial,
            });
        }
        delta
    }

    #[must_use]
    fn map_asset_records(&self, value: &Value, input: &IntentInput) -> MediaGraphDelta {
        let mut delta = MediaGraphDelta {
            sources: vec![self.source_profile()],
            ..MediaGraphDelta::default()
        };
        let unit_id = unit_id_from_input(input);
        let play_item_id = item_id_from_payload(input, &self.source_id)
            .or_else(|| unit_id.as_ref().and_then(item_id_from_unit_id));
        if let Some(item_id) = play_item_id {
            for record in records(value) {
                self.append_play_delta(record, &item_id, &mut delta);
            }
            if !delta.assets.is_empty() {
                if let Some(selected_unit_id) = unit_id.as_ref() {
                    delta
                        .assets
                        .retain(|asset| asset.unit_id.as_ref() == Some(selected_unit_id));
                }
                delta.units.clear();
                if !delta.assets.is_empty() {
                    return delta;
                }
            }
        }
        for (index, record) in records(value).iter().enumerate() {
            let content = asset_content(record).unwrap_or_default();
            let locator = asset_url(record)
                .map_or_else(|| MediaAssetLocator::Text(content), MediaAssetLocator::Url);
            let id_base = unit_id.as_ref().map_or_else(
                || format!("asset:{}:{index}", self.source_id.0),
                |id| id.0.clone(),
            );
            delta.assets.push(MediaAsset {
                id: MediaResourceId(format!("asset:{}:text", id_component(&id_base))),
                source_id: self.source_id.clone(),
                unit_id: unit_id.clone(),
                asset_kind: MediaAssetKind::Text,
                locator,
                metadata: metadata_from(record),
                completeness: ResourceCompleteness::Complete,
            });
        }
        delta
    }

    fn append_play_delta(
        &self,
        record: &Value,
        item_id: &MediaResourceId,
        delta: &mut MediaGraphDelta,
    ) {
        let Some(play_url) = play_url(record) else {
            return;
        };
        let item_source_key = item_source_key_from_id(item_id).unwrap_or_else(|| item_id.0.clone());
        let play_from = play_from(record).unwrap_or_default();
        let line_names: Vec<&str> = play_from
            .split(',')
            .flat_map(|part| part.split("$$$"))
            .collect();
        for (line_index, line) in play_url
            .split("###")
            .flat_map(|part| part.split("$$$"))
            .filter(|part| !part.is_empty())
            .enumerate()
        {
            let line_name = line_names
                .get(line_index)
                .copied()
                .filter(|name| !name.trim().is_empty())
                .unwrap_or("default")
                .trim()
                .to_string();
            for (episode_index, episode) in
                line.split('#').filter(|part| !part.is_empty()).enumerate()
            {
                let Some((title, url)) = episode.split_once('$') else {
                    continue;
                };
                if url.trim().is_empty() {
                    continue;
                }
                let unit_source_key = format!("play:{line_name}:{}", episode_index + 1);
                let unit_id = unit_resource_id(&self.source_id, &item_source_key, &unit_source_key);
                delta.units.push(MediaUnit {
                    id: unit_id.clone(),
                    source_id: self.source_id.clone(),
                    item_id: item_id.clone(),
                    title: title.trim().to_string(),
                    position: u32::try_from(episode_index + 1).ok(),
                    metadata: BTreeMap::from([(
                        "line".to_string(),
                        Value::String(line_name.clone()),
                    )]),
                    completeness: ResourceCompleteness::Complete,
                });
                delta.assets.push(MediaAsset {
                    id: MediaResourceId(format!("asset:{}:stream", unit_id.0)),
                    source_id: self.source_id.clone(),
                    unit_id: Some(unit_id),
                    asset_kind: MediaAssetKind::VideoStream,
                    locator: MediaAssetLocator::Url(url.trim().to_string()),
                    metadata: BTreeMap::from([(
                        "line".to_string(),
                        Value::String(line_name.clone()),
                    )]),
                    completeness: ResourceCompleteness::Complete,
                });
            }
        }
    }

    #[must_use]
    fn source_profile(&self) -> SourceProfile {
        SourceProfile {
            id: self.source_id.clone(),
            title: if self.source_title.trim().is_empty() {
                self.source_id.0.clone()
            } else {
                self.source_title.clone()
            },
            icon_url: None,
            version: None,
            supported_intents: self.supported_intents.clone(),
            risk_notes: Vec::new(),
        }
    }

    #[must_use]
    fn item_id(&self, record: &Value, input: &IntentInput) -> MediaResourceId {
        if let Some(id) = item_id_from_input(input) {
            return id;
        }
        let key = item_source_key(record).unwrap_or_else(|| "unknown".to_string());
        item_resource_id(&self.source_id, &key)
    }
}

#[must_use]
fn records(value: &Value) -> Vec<&Value> {
    value
        .as_array()
        .map_or_else(|| vec![value], |items| items.iter().collect())
}

#[must_use]
fn metadata_from(value: &Value) -> BTreeMap<String, Value> {
    let Some(object) = value.as_object() else {
        return BTreeMap::new();
    };
    object
        .iter()
        .filter(|(_, value)| !value.is_null())
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

#[must_use]
fn item_id_from_input(input: &IntentInput) -> Option<MediaResourceId> {
    match input {
        IntentInput::ItemId(id) if id.starts_with("item:") => Some(MediaResourceId(id.clone())),
        _ => None,
    }
}

#[must_use]
fn unit_id_from_input(input: &IntentInput) -> Option<MediaResourceId> {
    match input {
        IntentInput::UnitId(id) if id.starts_with("unit:") => Some(MediaResourceId(id.clone())),
        _ => None,
    }
}

#[must_use]
fn item_id_from_payload(
    input: &IntentInput,
    source_id: &MediaResourceId,
) -> Option<MediaResourceId> {
    let IntentInput::Opaque(value) = input else {
        return None;
    };
    text_field(value, &["item_id", "itemId"])
        .filter(|id| id.starts_with("item:"))
        .map(MediaResourceId)
        .or_else(|| payload_item_key(value).map(|key| item_resource_id(source_id, &key)))
}

#[must_use]
fn item_source_key_from_id(item_id: &MediaResourceId) -> Option<String> {
    parse_item_resource_id(&item_id.0).map(|(_, source_key)| source_key)
}

#[must_use]
fn item_id_from_unit_id(unit_id: &MediaResourceId) -> Option<MediaResourceId> {
    parse_unit_resource_id(&unit_id.0)
        .map(|(source_id, item_source_key, _)| item_resource_id(&source_id, &item_source_key))
}

#[must_use]
fn id_component(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "unknown".to_string();
    }
    let mut out = String::with_capacity(trimmed.len().min(96));
    for byte in trimmed.bytes().take(96) {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                out.push(char::from(byte));
            }
            _ => {
                let _ = write!(out, "_{byte:02x}");
            }
        }
    }
    if out.is_empty() {
        "unknown".to_string()
    } else {
        out
    }
}
