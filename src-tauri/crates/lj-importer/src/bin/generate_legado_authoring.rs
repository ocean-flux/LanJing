//! Legado authoring 正式 JSON generator/drift/audit 命令。

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lj_importer::authoring::catalog::{
    audit_upstream, check_generated_artifacts, project_root_from_manifest,
    write_generated_artifacts,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let project_root =
        project_root_from_manifest(manifest_dir).ok_or("无法从 lj-importer manifest 定位项目根")?;
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match arguments.as_slice() {
        [] => write_generated_artifacts(&project_root)?,
        [flag] if flag == "--check" => check_generated_artifacts(&project_root)?,
        [flag, upstream] if flag == "--audit-upstream" => {
            let upstream = resolve_path(&project_root, upstream);
            audit_upstream(&upstream)?;
            write_generated_artifacts(&project_root)?;
        }
        _ => {
            return Err(
                "usage: generate_legado_authoring [--check | --audit-upstream <checkout>]".into(),
            );
        }
    }
    Ok(())
}

fn resolve_path(project_root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        project_root.join(path)
    }
}
