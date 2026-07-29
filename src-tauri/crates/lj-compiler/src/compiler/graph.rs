//! validation 共用的纯 graph 查询。

use super::{BTreeMap, BTreeSet, FlowEdge, FlowPortRef, HashSet, Uuid, VecDeque};

pub(in crate::compiler) fn edges_by_input<'a>(
    edges: &'a [&'a FlowEdge],
) -> BTreeMap<FlowPortRef, Vec<&'a FlowEdge>> {
    let mut result = BTreeMap::<FlowPortRef, Vec<&FlowEdge>>::new();
    for edge in edges {
        result.entry(edge.to.clone()).or_default().push(edge);
    }
    result
}

pub(in crate::compiler) fn edges_by_output<'a>(
    edges: &'a [&'a FlowEdge],
) -> BTreeMap<FlowPortRef, Vec<&'a FlowEdge>> {
    let mut result = BTreeMap::<FlowPortRef, Vec<&FlowEdge>>::new();
    for edge in edges {
        result.entry(edge.from.clone()).or_default().push(edge);
    }
    result
}

pub(in crate::compiler) fn adjacency(
    edges: &[&FlowEdge],
    excluded: &BTreeSet<FlowEdge>,
) -> BTreeMap<Uuid, Vec<Uuid>> {
    let mut result = BTreeMap::<Uuid, Vec<Uuid>>::new();
    for edge in edges {
        if excluded.contains(*edge) {
            continue;
        }
        result
            .entry(edge.from.node_id)
            .or_default()
            .push(edge.to.node_id);
    }
    for neighbors in result.values_mut() {
        neighbors.sort_unstable();
        neighbors.dedup();
    }
    result
}

pub(in crate::compiler) fn is_reachable(
    adjacency: &BTreeMap<Uuid, Vec<Uuid>>,
    from: Uuid,
    to: Uuid,
) -> bool {
    if from == to {
        return true;
    }
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([from]);
    while let Some(current) = queue.pop_front() {
        if !seen.insert(current) {
            continue;
        }
        if let Some(neighbors) = adjacency.get(&current) {
            for neighbor in neighbors {
                if *neighbor == to {
                    return true;
                }
                queue.push_back(*neighbor);
            }
        }
    }
    false
}

pub(in crate::compiler) fn has_cycle(
    nodes: impl IntoIterator<Item = Uuid>,
    adjacency: &BTreeMap<Uuid, Vec<Uuid>>,
) -> bool {
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    nodes
        .into_iter()
        .any(|node| has_cycle_from(node, adjacency, &mut visiting, &mut visited))
}

pub(in crate::compiler) fn has_cycle_from(
    node: Uuid,
    adjacency: &BTreeMap<Uuid, Vec<Uuid>>,
    visiting: &mut HashSet<Uuid>,
    visited: &mut HashSet<Uuid>,
) -> bool {
    if visited.contains(&node) {
        return false;
    }
    if !visiting.insert(node) {
        return true;
    }
    let cycle = adjacency.get(&node).is_some_and(|neighbors| {
        neighbors
            .iter()
            .any(|next| has_cycle_from(*next, adjacency, visiting, visited))
    });
    visiting.remove(&node);
    visited.insert(node);
    cycle
}

pub(in crate::compiler) fn is_valid_json_pointer(pointer: &str) -> bool {
    if pointer.is_empty() {
        return true;
    }
    if !pointer.starts_with('/') {
        return false;
    }
    for token in pointer.split('/').skip(1) {
        let bytes = token.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] != b'~' {
                index += 1;
                continue;
            }
            if index + 1 >= bytes.len() || !matches!(bytes[index + 1], b'0' | b'1') {
                return false;
            }
            index += 2;
        }
    }
    true
}
