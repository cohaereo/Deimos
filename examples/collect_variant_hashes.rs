use std::{
    collections::HashSet,
    io::{Cursor, Seek, SeekFrom},
};

use anyhow::Context;
use deimos_data::{
    hash::fnv1, pattern::SComponent, tfx::features::dynamic::SDynamicModelComponent,
};
use tiger_parse::{PackageManagerExt, TigerReadable};
use tiger_pkg::package_manager;

fn main() -> anyhow::Result<()> {
    deimos_core::initialize_package_manager(None)?;

    let mut hashes: HashSet<u32> = HashSet::new();
    for (hash, _) in package_manager().get_all_by_reference(SComponent::ID.unwrap()) {
        let component: SComponent = package_manager()
            .read_tag_struct(hash)
            .context("Failed to read/parse tag")?;

        if component.default_instance.resource_type != 0x80808673 {
            continue;
        }

        let mut cur = Cursor::new(package_manager().read_tag(hash)?);
        cur.seek(SeekFrom::Start(component.definition.offset))?;
        let model: SDynamicModelComponent = TigerReadable::read_ds(&mut cur)?;

        for keys1 in &model.unk38 {
            for keys in &keys1.unk8 {
                hashes.insert(keys.switch_key);
                hashes.insert(keys.value);
            }
        }
    }

    let mut hashes: Vec<u32> = hashes.into_iter().collect();
    hashes.sort_unstable();
    for hash in hashes {
        println!("0x{:08X} => {}", hash, find_hash(hash));
    }

    Ok(())
}

const HASH_STRS: &[&str] = &[
    "accent",
    "black",
    "blue",
    "brightness",
    "cyan",
    "device_lock",
    "device_position",
    "device_power",
    "dim",
    "flicker",
    "flux",
    "gray",
    "grey",
    "grayscale",
    "green",
    "grime",
    "high",
    "imminencies",
    "interference",
    "inverted",
    "main",
    "max",
    "mid",
    "min",
    "off",
    "off",
    "on",
    "orange",
    "permutation",
    "pink",
    "pulse",
    "red",
    "region",
    "smoothness",
    "struggle",
    "uv",
    "waterline",
    "white",
    "yellow",
    "frost",
];

const GUESSES: &[(u32, &str)] = &[(0xB01748DA, "color"), (0x871AC0EA, "<unused>")];

fn find_hash(hash: u32) -> String {
    if let Some(s) = HASH_STRS.iter().find(|s| fnv1(s) == hash) {
        s.to_string()
    } else {
        if let Some((_, s)) = GUESSES.iter().find(|(h, _)| *h == hash) {
            s.to_string()
        } else {
            format!("{:08X}", hash)
        }
    }
}
