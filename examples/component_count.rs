use std::{
    hash::{DefaultHasher, Hash, Hasher},
    io::{Cursor, Seek, SeekFrom},
    str::FromStr,
};

use ahash::HashMap;
use anyhow::Context;
use chroma_dbg::ChromaDebug;
use deimos_data::{
    hash::fnv1, pattern::SComponent, tfx::features::dynamic::SDynamicModelComponent,
};
use tiger_parse::{PackageManagerExt, TigerReadable};
use tiger_pkg::{TagHash, package_manager};

fn main() -> anyhow::Result<()> {
    deimos_core::initialize_package_manager(None)?;

    let mut types = HashMap::<u32, usize>::default();
    for (hash, _) in package_manager().get_all_by_reference(SComponent::ID.unwrap()) {
        let component: SComponent = package_manager()
            .read_tag_struct(hash)
            .context("Failed to read/parse tag")?;

        let entry = types
            .entry(component.default_instance.resource_type)
            .or_default();
        *entry += 1;
    }

    for (resource_type, count) in types {
        println!("{:08X}: {}", resource_type, count);
    }

    Ok(())
}
