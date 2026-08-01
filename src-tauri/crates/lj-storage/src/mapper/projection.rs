//! Projection delta 到 checkpoint snapshot 的纯内存映射。

use crate::repository::event::serialize;
use crate::types::ProjectionDelta;

pub(crate) fn apply_delta_in_memory(graph: &mut lj_media::MediaGraphDelta, delta: ProjectionDelta) {
    *graph = graph.clone().merge(delta.upserts);
    graph
        .sources
        .retain(|value| !delta.tombstones.sources.contains(&value.id));
    graph
        .items
        .retain(|value| !delta.tombstones.items.contains(&value.id));
    graph
        .collections
        .retain(|value| !delta.tombstones.collections.contains(&value.id));
    graph
        .units
        .retain(|value| !delta.tombstones.units.contains(&value.id));
    graph
        .assets
        .retain(|value| !delta.tombstones.assets.contains(&value.id));
    graph
        .actions
        .retain(|value| !delta.tombstones.actions.contains(&value.id));
    graph
        .hints
        .retain(|value| !delta.tombstones.hints.contains(&value.resource_id));
    graph.relations.retain(|value| {
        !delta.tombstones.relations.iter().any(|tombstone| {
            tombstone.source_id == value.source_id
                && tombstone.from_id == value.from_id
                && tombstone.to_id == value.to_id
                && tombstone.relation_kind == serialize(&value.relation_kind).unwrap_or_default()
        })
    });
}
