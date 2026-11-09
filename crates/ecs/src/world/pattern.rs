use std::{
    collections::HashSet,
    io::{Cursor, Seek, SeekFrom},
    sync::{Arc, atomic::AtomicUsize},
};

use anyhow::Context;
use deimos_data::{
    map::{ComponentData, SComponentDataListPtr},
    pattern::{S8080A313, SComponent, SObjectChannelComponent, SPattern},
    strings::StringContainer,
    tag::{TagRef, WideHash},
};

use glam::{Vec4, vec4};
use itertools::Itertools;
use tiger_parse::{FnvHash, PackageManagerExt, TigerReadable};
use tiger_pkg::{TagHash, package_manager};
use tracing::{debug, error, info};

use crate::{
    PatternSource, TempAudioMarker, UnimplementedTigerComponent, UnimplementedTigerComponents,
    interactibles::Pingable,
    object::{ObjectChannel, ObjectChannels},
    transform::Transform,
    world::map::ComponentLoadResult,
};

#[macro_export]
macro_rules! once {
    () => {{
        static RAN_ONCE: AtomicBool = AtomicBool::new(false);
        !RAN_ONCE.swap(true, Ordering::SeqCst)
    }};
}

pub fn spawn_pattern<F>(
    world: &mut hecs::World,
    pattern_tag: TagHash,
    map_data_list: Option<&SComponentDataListPtr>,
    transform: Option<Transform>,
    callback: F,
) -> anyhow::Result<hecs::Entity>
where
    F: Fn(
        &mut hecs::World,
        hecs::Entity,
        &SPattern,
        &ComponentData,
        &TagRef<SComponent>,
    ) -> anyhow::Result<ComponentLoadResult>,
{
    spawn_pattern_internal(world, pattern_tag, map_data_list, transform, &callback)
}

fn spawn_pattern_internal<F>(
    world: &mut hecs::World,
    pattern_tag: TagHash,
    map_data_list: Option<&SComponentDataListPtr>,
    transform: Option<Transform>,
    callback: &F,
) -> anyhow::Result<hecs::Entity>
where
    F: Fn(
        &mut hecs::World,
        hecs::Entity,
        &SPattern,
        &ComponentData,
        &TagRef<SComponent>,
    ) -> anyhow::Result<ComponentLoadResult>,
{
    let header = package_manager()
        .read_tag_struct::<SPattern>(pattern_tag)
        .context("Failed to read SPattern")?;
    let e = spawn_pattern_from_header(world, &header, map_data_list, transform, callback)?;

    world.insert_one(e, PatternSource { tag: pattern_tag });

    Ok(e)
}

pub fn spawn_pattern_from_header<F>(
    world: &mut hecs::World,
    header: &SPattern,
    map_data_list: Option<&SComponentDataListPtr>,
    transform: Option<Transform>,
    callback: &F,
) -> anyhow::Result<hecs::Entity>
where
    F: Fn(
        &mut hecs::World,
        hecs::Entity,
        &SPattern,
        &ComponentData,
        &TagRef<SComponent>,
    ) -> anyhow::Result<ComponentLoadResult>,
{
    let entity = world.spawn(());
    if let Some(transform) = transform {
        world.insert_one(entity, transform)?;
    }

    for e in &header.components {
        let mut cur = Cursor::new(package_manager().read_tag(e.component)?);
        let component = TagRef::new(SComponent::read_ds(&mut cur)?, e.component);
        cur.seek(SeekFrom::Start(component.definition.offset))?;

        macro_rules! add_unknown_component {
            ($name:expr) => {
                let component = UnimplementedTigerComponent {
                    class_id: component.default_instance.resource_type,
                    hash: component.taghash(),
                    name: None,
                };
                if let Ok(mut components) = world.get::<&mut UnimplementedTigerComponents>(entity) {
                    components.0.push(component);
                } else {
                    world.insert_one(entity, UnimplementedTigerComponents(vec![component]))?;
                }
            };
        }

        let Some(dynamic_data) = component.dynamic_data.first() else {
            continue;
        };

        let data = if let Some(data) =
            map_data_list.and_then(|l| l.get_by_class(dynamic_data.data().class_id()))
        {
            data
        } else {
            dynamic_data.data()
        };

        // macro_rules! get_component_data {
        //     ($type:ident) => {
        //         if let ComponentData::$type(c) = data {
        //             c
        //         } else {
        //             error!(
        //                 "Expected component data type {} for component type 0x{:08X}, found \
        //                  {}/0x{:08X}",
        //                 stringify!($type),
        //                 component.unk10.resource_type,
        //                 data.class_name(),
        //                 data.class_id()
        //             );
        //             continue;
        //         }
        //     };
        // }

        match callback(world, entity, header, data, &component)
            .with_context(|| format!("while loading component {}", component.taghash()))?
        {
            ComponentLoadResult::Loaded => {
                continue;
            }
            ComponentLoadResult::Skipped => {}
        }

        match component.default_instance.resource_type {
            0x80802976 => {
                cur.seek(SeekFrom::Start(component.definition.offset + 0x58))?;
                let hash = FnvHash::read_ds(&mut cur)?;
                cur.seek(SeekFrom::Start(component.definition.offset + 0x68))?;
                let string_table = WideHash::read_ds(&mut cur)?;

                match StringContainer::load(string_table) {
                    Ok(container) => {
                        let name = container.0.get(&hash).cloned();
                        world.insert_one(entity, Pingable { name })?;
                    }
                    Err(e) => {
                        error!("Failed to load string container for pingable component: {e:?}");
                    }
                }
            }
            0x8080AF8E => {
                let data = SObjectChannelComponent::read_ds(&mut cur)?;
                let mut channels = ObjectChannels(
                    data.m_channels
                        .iter()
                        .map(|c| ObjectChannel {
                            name: c.name,
                            value: c
                                .expression
                                .bytecode_constants
                                .first()
                                .cloned()
                                .unwrap_or(Vec4::ONE),
                            expression: c.expression.clone(),
                            usage: Arc::new(AtomicUsize::new(0)),
                        })
                        .collect(),
                );

                channels.set_by_name("cool_down", Vec4::ZERO);
                channels.set_by_name("charge_progress", Vec4::ZERO);
                channels.set_by_name("unique_id", Vec4::splat(fastrand::f32_inclusive()));

                // Fixes darkened clearance code icons on terminals
                channels.set_by_name("deposit_1", Vec4::ZERO);
                channels.set_by_name("deposit_2", Vec4::ZERO);
                channels.set_by_name("deposit_3", Vec4::ZERO);
                channels.set_by_id(0xDEF47AFA, Vec4::splat(3.5));
                channels.set_by_id(0xDEF47AFB, Vec4::splat(1.0));
                channels.set_by_id(0xDEF47AF8, Vec4::splat(0.5));

                channels.set_by_name("hack_progress", Vec4::splat(0.5));
                channels.set_by_name("device_power", Vec4::splat(0.1));
                channels.set_by_name("spawn_in", Vec4::ZERO);
                channels.set_by_id(0x0FBEEF72, Vec4::ZERO);
                // channels.set_by_id(0x262F908A, Vec4::ZERO);

                channels.set_by_id(0xEE1D8D50, Vec4::ZERO); // Makes flight control roomba happy :D

                channels.set_by_id(0x2EC4BC4E, Vec4::ZERO); // Makes compiler/sptsh bullets more recognisable
                channels.set_by_id(0x6057A3B9, Vec4::ZERO); // Makes compiler core green
                channels.set_by_id(0x0EDC1DFF, Vec4::ZERO); // Makes compiler cape visible
                channels.set_by_id(0xEE29282E, Vec4::ZERO);
                channels.set_by_id(0x5B7CD2A2, Vec4::ZERO);
                channels.set_by_id(0x8694B692, Vec4::ZERO); // Makes big compiler cape visible

                channels.set_by_id(0x3969B148, Vec4::ZERO); // Hides exfil bubble/flash
                channels.set_by_id(0xDEB2E0B2, Vec4::splat(0.1)); // Shows exfil circle a bit better
                channels.set_by_id(0x25784EE5, vec4(0.0, 0.0, 2.0, 1.0)); // Positions exfil bubble/flash correctly

                channels.set_by_id(0xF7202BBC, Vec4::splat(0.0)); // If not zero then rook's material varieties are static
                channels.set_by_id(0x7EC60CCB, Vec4::splat(0.0)); // same as above, but for the decals

                channels.set_by_id(0x2B9D1F06, Vec4::splat(0.0)); // Fixes invisible weapon mods

                world.insert_one(entity, channels)?;
            }
            0x8080A317 => {
                let data = S8080A313::read_ds(&mut cur)?;
                for u1 in data.unk98 {
                    for u2 in u1.unk8 {
                        if u2.pattern.is_none() {
                            continue;
                        }

                        spawn_pattern_internal(
                            world,
                            u2.pattern.hash32(),
                            None,
                            transform,
                            callback,
                        )
                        .with_context(|| {
                            format!("failed to spawn attached pattern for bone {:08X}", u2.bone)
                        })?;
                    }
                }
            }
            0x808077C0 => {
                // TODO(cohae): Do this in a way that isn't brute forcing
                cur.seek(std::io::SeekFrom::Start(component.definition.offset))?;
                let mut loaded_patterns: HashSet<TagHash> = HashSet::default();
                while let Ok(taghash) = WideHash::read_ds(&mut cur) {
                    cur.seek(std::io::SeekFrom::Current(-0xC))?;
                    let Some(entry) = package_manager().get_entry(taghash) else {
                        continue;
                    };
                    if Some(entry.reference) == SPattern::ID
                        && loaded_patterns.insert(taghash.hash32())
                    {
                        spawn_pattern_internal(world, taghash.hash32(), None, None, callback)?;
                    }
                }
                info!(
                    "Loaded {} node tables from component",
                    loaded_patterns.len()
                );
            }
            0x8080B69A => {
                world.insert_one(entity, TempAudioMarker);
            }
            u => {
                debug!(
                    "\t- Unknown entity component type {:08X}, tag {:08X}, data type {:08X}/{} \
                     (table {})",
                    u,
                    component.default_instance.resource_type,
                    data.class_id(),
                    data.class_name(),
                    component.taghash()
                );
                if let Some(map_data) = map_data_list {
                    debug!(
                        "\t\t- Has map data ({})",
                        map_data
                            .iter()
                            .enumerate()
                            .map(|(i, c)| format!("[{i}]={}({:08X})", c.class_name(), c.class_id()))
                            .join(", ")
                    );
                }

                if let ComponentData::Unknown { .. } = data {
                } else {
                    error!(
                        "Defined component data type 0x{:X} ({}) was not used while instancing \
                         components! (component class 0x{u:X})",
                        data.class_id(),
                        data.class_name()
                    );
                }

                add_unknown_component!(None);
            }
        }
    }

    Ok(entity)
}
