use std::str::FromStr;

use anyhow::Context;
use deimos_data::tfx::{RenderStage, features::dynamic::SDynamicModel, scope::SScope};
use deimos_render::tfx::expression_vm::{self, decompiler::DecompilerState};
use itertools::Itertools;
use tiger_parse::PackageManagerExt;
use tiger_pkg::{TagHash, package_manager};

fn main() -> anyhow::Result<()> {
    let Some(hash) = std::env::args().nth(1) else {
        anyhow::bail!("Usage: scope_decompile <package dir> <scope tag>");
    };

    let Ok(hash) = TagHash::from_str(&hash) else {
        anyhow::bail!("Invalid scope tag hash: {}", hash);
    };

    deimos_core::initialize_package_manager(None)?;

    let model: SDynamicModel = package_manager()
        .read_tag_struct(hash)
        .context("Failed to read/parse tag")?;

    for (mesh_index, mesh) in model.meshes.iter().enumerate() {
        println!("mesh {mesh_index}: {} parts", mesh.parts.len());
        for stage in RenderStage::iter() {
            let range = mesh.get_range_for_stage(stage);
            if range.is_empty() {
                continue;
            }

            println!("  stage {stage:?}: parts {}-{}", range.start, range.end);
            for part_index in range {
                let part = &mesh.parts[part_index];
                println!(
                    "    part {part_index}: indices={}-{} technique={} ({:?})",
                    part.index_start,
                    part.index_start + part.index_count,
                    part.technique,
                    part.lod_category
                );
            }
        }
    }

    Ok(())
}
