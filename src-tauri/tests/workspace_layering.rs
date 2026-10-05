//! workspace crate 分层的守卫。
//!
//! 允许的依赖边就是下面这张 `ALLOWED` 表：它与实际 `[dependencies]` 边完全一致，
//! 并断言整张图无环。新增或删除一条边都必须改这张表 —— 边界变更因此是一次显式动作，
//! 不会悄悄发生。
//!
//! 规则与理由见 `docs/adr/0008-rust-crate-layering.md`。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// 每个 crate 允许的内部 `[dependencies]` 边。只统计生产依赖，不含 dev/build 依赖。
const ALLOWED: &[(&str, &[&str])] = &[
    // 门面层：Tauri 二进制只依赖门面，不直接碰 storage / runtime / compiler / importer。
    ("lanjing", &["lj-rule-system"]),
    // 契约叶子：无内部依赖。
    ("lj-capability", &[]),
    ("lj-storage-entity", &[]),
    ("lj-media", &["lj-capability"]),
    ("lj-rule-model", &["lj-capability"]),
    ("lj-compiler", &["lj-rule-model"]),
    ("lj-importer", &["lj-capability", "lj-rule-model"]),
    (
        "lj-runtime",
        &["lj-capability", "lj-media", "lj-rule-model"],
    ),
    ("lj-node-extract", &["lj-rule-model", "lj-runtime"]),
    (
        "lj-node-http",
        &["lj-capability", "lj-media", "lj-rule-model", "lj-runtime"],
    ),
    (
        "lj-node-js",
        &["lj-capability", "lj-rule-model", "lj-runtime"],
    ),
    (
        "lj-storage-migration",
        &["lj-compiler", "lj-rule-model", "lj-storage-entity"],
    ),
    (
        "lj-storage",
        &[
            "lj-media",
            "lj-rule-model",
            "lj-runtime",
            "lj-storage-entity",
            "lj-storage-migration",
        ],
    ),
    (
        "lj-rule-system",
        &[
            "lj-capability",
            "lj-compiler",
            "lj-importer",
            "lj-media",
            "lj-node-extract",
            "lj-node-http",
            "lj-node-js",
            "lj-rule-model",
            "lj-runtime",
            "lj-storage",
        ],
    ),
];

/// 端到端收尾 crate：允许依赖全部层，它的 `[dependencies]` 不代表产品依赖方向。
const EXEMPT: &[&str] = &["lj-integration-tests"];

#[test]
fn crate_dependencies_match_the_allowed_layer_table() {
    let dir = src_tauri_dir();

    let mut actual: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    actual.insert(
        package_name(&dir.join("Cargo.toml")),
        internal_dependencies(&dir.join("Cargo.toml")),
    );
    for manifest in crate_manifests(&dir) {
        let name = package_name(&manifest);
        if EXEMPT.contains(&name.as_str()) {
            continue;
        }
        actual.insert(name, internal_dependencies(&manifest));
    }

    let expected: BTreeMap<String, BTreeSet<String>> = ALLOWED
        .iter()
        .map(|(name, deps)| {
            let deps = deps.iter().map(|dep| (*dep).to_owned()).collect();
            ((*name).to_owned(), deps)
        })
        .collect();

    assert_eq!(
        actual, expected,
        "crate 依赖边与 ALLOWED 表不一致；边界变更必须同时改表与 \
         docs/adr/0008-rust-crate-layering.md"
    );
}

#[test]
fn crate_dependency_graph_is_acyclic() {
    let mut remaining: BTreeMap<&str, BTreeSet<&str>> = ALLOWED
        .iter()
        .map(|(name, deps)| (*name, deps.iter().copied().collect()))
        .collect();

    // Kahn：反复摘掉「依赖都已摘掉」的节点；还有剩就说明有环。
    loop {
        let leaves: Vec<&str> = remaining
            .iter()
            .filter(|(_, deps)| deps.iter().all(|dep| !remaining.contains_key(dep)))
            .map(|(name, _)| *name)
            .collect();
        if leaves.is_empty() {
            break;
        }
        for leaf in &leaves {
            remaining.remove(*leaf);
        }
    }

    assert!(
        remaining.is_empty(),
        "crate 依赖图里有环：{:?}",
        remaining.keys().collect::<Vec<_>>()
    );
}

fn src_tauri_dir() -> PathBuf {
    // `cargo test` 以包根为 cwd；直接运行测试二进制时退回编译期路径。
    let cwd = std::env::current_dir().expect("读取当前目录");
    if cwd.join("crates").is_dir() && cwd.join("Cargo.toml").is_file() {
        return cwd;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn crate_manifests(dir: &Path) -> Vec<PathBuf> {
    let mut manifests: Vec<PathBuf> = std::fs::read_dir(dir.join("crates"))
        .expect("读取 crates 目录")
        .map(|entry| entry.expect("目录项").path().join("Cargo.toml"))
        .filter(|manifest| manifest.is_file())
        .collect();
    manifests.sort();
    manifests
}

fn package_name(manifest: &Path) -> String {
    let text = read(manifest);
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && key.trim() == "name"
        {
            return value.trim().trim_matches('"').to_owned();
        }
    }
    panic!("{} 里找不到 [package] name", manifest.display());
}

fn internal_dependencies(manifest: &Path) -> BTreeSet<String> {
    let text = read(manifest);
    let mut dependencies = BTreeSet::new();
    let mut in_dependencies = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            // 也接受 `[dependencies.lj-x]` 这种分段写法。
            if let Some(rest) = line.strip_prefix("[dependencies.") {
                let name = rest.trim_end_matches(']').trim();
                if name.starts_with("lj-") {
                    dependencies.insert(name.to_owned());
                }
            }
            in_dependencies = is_production_dependency_section(line);
            continue;
        }
        if !in_dependencies || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, _)) = line.split_once('=') {
            let key = key.trim();
            if key.starts_with("lj-") {
                dependencies.insert(key.to_owned());
            }
        }
    }
    dependencies
}

fn is_production_dependency_section(header: &str) -> bool {
    header.ends_with("dependencies]")
        && !header.starts_with("[workspace")
        && !header.contains("dev-")
        && !header.contains("build-")
}

fn read(manifest: &Path) -> String {
    std::fs::read_to_string(manifest)
        .unwrap_or_else(|error| panic!("读取 {} 失败: {error}", manifest.display()))
}
