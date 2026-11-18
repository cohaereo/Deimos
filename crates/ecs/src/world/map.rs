use std::io::{Cursor, Seek};

use anyhow::Context;
use deimos_data::{
    activity::{SActivity, SUnk80808948},
    map::{ComponentData, SBubbleParent, SMapNodeTable},
    pattern::{SComponent, SPattern},
    tag::TagRef,
};
use glam::Vec4Swizzles;
use tiger_parse::{PackageManagerExt, TigerReadable};
use tiger_pkg::{TagHash, package_manager};
use tracing::{debug, error, info, warn};

use crate::{transform::Transform, world::pattern::spawn_pattern};

#[derive(Debug, PartialEq, Eq)]
pub enum ComponentLoadResult {
    Loaded,
    Skipped,
}

pub fn load_map_into_world<F>(
    taghash: TagHash,
    world: &mut hecs::World,
    callback: F,
) -> anyhow::Result<()>
where
    F: Fn(
        &mut hecs::World,
        hecs::Entity,
        &SPattern,
        &ComponentData,
        &TagRef<SComponent>,
    ) -> anyhow::Result<ComponentLoadResult>,
{
    info!("Loading map {taghash}");
    let start = std::time::Instant::now();
    let parent = package_manager()
        .read_tag_struct::<SBubbleParent>(taghash)
        .context("Failed to read SBubbleParent")?;
    for resources in &parent.definition.containers {
        for datatable_hash in &resources.data_tables {
            load_nodetable_into_world(*datatable_hash, world, &callback)?;
        }
    }
    info!("Loaded map in {:?}", start.elapsed());

    Ok(())
}

pub fn load_activity_phase_into_world<F>(
    phase: &SUnk80808948,
    world: &mut hecs::World,
    callback: F,
) -> anyhow::Result<()>
where
    F: Fn(
        &mut hecs::World,
        hecs::Entity,
        &SPattern,
        &ComponentData,
        &TagRef<SComponent>,
    ) -> anyhow::Result<ComponentLoadResult>,
{
    for res in &phase.unk_entity_reference.unk18.components {
        let component_data = package_manager()
            .read_tag(res.component)
            .context("Failed to read component data")?;
        let mut component_data = Cursor::new(component_data);

        let component =
            SComponent::read_ds(&mut component_data).context("Failed to read SComponent")?;

        match component.definition.resource_type {
            0x8080B1A5 => {
                component_data
                    .seek(std::io::SeekFrom::Start(component.definition.offset + 0x84))?;
                let nodetable_hash = TagHash::read_ds(&mut component_data)?;
                load_nodetable_into_world(nodetable_hash, world, &callback)?;
            }
            // 0x8080B24B
            0x8080B250 => {
                component_data
                    .seek(std::io::SeekFrom::Start(component.definition.offset + 0x68))?;
                let nodetable_hash = TagHash::read_ds(&mut component_data)?;
                load_nodetable_into_world(nodetable_hash, world, &callback)?;
            }
            // cohae: This loads a LOT of stuff, to the point where the application becomes unusable
            // // 0x80805be0
            0x80805B83 => {
                //     // TODO(cohae): Do this in a way that isn't brute forcing
                //     component_data.seek(std::io::SeekFrom::Start(component.unk18.offset))?;
                //     let mut loaded_tables = HashSet::default();
                //     while let Ok(taghash) = WideHash::read_ds(&mut component_data) {
                //         component_data.seek(std::io::SeekFrom::Current(-0xC))?;
                //         let Some(entry) = package_manager().get_entry(taghash) else {
                //             continue;
                //         };
                //         if Some(entry.reference) == SMapNodeTable::ID
                //             && loaded_tables.insert(taghash.hash32())
                //         {
                //             load_nodetable_into_world(renderer, taghash.hash32(), world)?;
                //         }
                //     }
                //     info!("Loaded {} node tables from component", loaded_tables.len());
            }
            0x80805B63 => {
                // contains node tables
            }
            0x80805924 => {
                // contains pattern
            }
            0x80805951 => {
                // contains ??? (empty?)
            }
            0x808059B6 => {
                // contains music/sound events
            }
            0x80805A18 => {
                // contains pattern
            }
            0x80805A25 => {
                // contains ??? (empty?)
            }
            0x80805A8C => {
                // contains ???
            }
            0x80805B38 => {
                // contains ??? (empty)
            }
            0x80805B39 => {
                // contains ??? (empty)
            }
            0x80805B65 => {
                // contains ??? (objective_, spawn_rule)
            }
            // 0x80805c9a
            0x80805C19 => {
                // contains ??? (accouncement_filter)
            }
            0x8080995E => {
                // contains ??? (pm_fodder, pm_elite_b)
            }
            // 0x8080B0D1
            0x8080B0D5 => {
                // contains ??? (fa_firing_area, ai_firing_area)
            }
            // 0x8080B0DF
            0x8080B0E0 => {
                // contains ??? (region_*_loot)
            }
            // 0x8080B198
            0x8080B199 => {
                // contains ??? (tick_spawner_nest_*)
            }
            // 0x8080B1B5
            0x8080B1B6 => {
                // contains ??? (eflg_compiler_fight_started, eflg_spectacle_complete)
            }
            // // 0x8080B24B
            // 0x8080B250 => {
            //     // contains ??? (ppc_*, population_loot_budget)
            // }
            // 0x8080B507
            0x8080B508 => {
                // contains ??? (activity_variables)
            }
            // 0x8080B831
            0x8080B832 => {
                // contains ??? (p_point, pg_spline, spline)
            }
            // 0x8080B83C
            0x8080B83E => {
                // contains ??? (p_point_*)
            }
            // 0x8080B89F
            0x8080B8A0 => {
                // contains ??? (a whole load of different things)
            }
            0x80B38378 | 0x80B38AEE | 0x80B38AF1 | 0x80B3F9A2 | 0x80B3E7CE | 0x80B41417
            | 0x80B3BF42 | 0x80B392D3 | 0x80B3CFB0 => {
                // contains scripts
            }
            0x80B3DB80 => {
                // contains scripts and a lot of strings
            }
            u => {
                warn!(
                    "Unknown activity phase resource type in component {}: 0x{u:08X}",
                    res.component
                );
            }
        }
    }

    Ok(())
}

pub fn load_activity_for_map_into_world<F>(
    activity_hash: impl Into<TagHash>,
    bubble_hash: u32,
    world: &mut hecs::World,
    callback: F,
) -> anyhow::Result<()>
where
    F: Fn(
        &mut hecs::World,
        hecs::Entity,
        &SPattern,
        &ComponentData,
        &TagRef<SComponent>,
    ) -> anyhow::Result<ComponentLoadResult>,
{
    let activity: SActivity = package_manager().read_tag_struct(activity_hash.into())?;
    let activity_map = &activity
        .unk50
        .iter()
        .find(|b| b.bubble_name == bubble_hash)
        .context("Map index out of range")?;

    for unk in &activity_map.unk18 {
        if let Err(e) = load_activity_phase_into_world(unk, world, &callback) {
            error!(
                "Activity phase load for {} failed: {e}",
                unk.unk_entity_reference.taghash()
            );
        }
    }

    Ok(())
}

pub fn load_nodetable_into_world<F>(
    table_hash: TagHash,
    world: &mut hecs::World,
    callback: &F,
) -> anyhow::Result<()>
where
    F: Fn(
        &mut hecs::World,
        hecs::Entity,
        &SPattern,
        &ComponentData,
        &TagRef<SComponent>,
    ) -> anyhow::Result<ComponentLoadResult>,
{
    let table: SMapNodeTable = package_manager().read_tag_struct(table_hash)?;
    for node in table.nodes {
        let transform = Transform::new(
            node.translation.xyz(),
            node.rotation,
            node.translation.www(),
        );

        for (i, data) in node.component_data.iter().enumerate() {
            if let ComponentData::Unknown { class, offset, .. } = data {
                debug!(
                    "Unknown dynamic component data class: {:08X} (#{}) in {table_hash} at \
                     offset: {:#X}",
                    class, i, offset
                );
            }
        }

        if node.entity.is_none() {
            anyhow::bail!(
                "Map data table node with world id {} has no entity. This shouldn't be possible!",
                node.world_id
            );
        }

        if let Err(e) = spawn_pattern(
            world,
            node.entity,
            Some(&node.component_data),
            Some(transform),
            callback,
        ) {
            error!("Failed to load entity {}: {:?}", node.entity, e);
        }
    }

    Ok(())
}
