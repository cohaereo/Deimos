use std::collections::HashSet;

use deimos_data::tfx::{ExternIndex, SDynamicCore, STechnique, scope::SScope};
use deimos_render::tfx::expression_vm::opcodes::{Opcode, OpcodeIterator, pascal_to_snake};
use itertools::Itertools;
use tiger_parse::{PackageManagerExt, TigerReadable};
use tiger_pkg::package_manager;

fn main() -> anyhow::Result<()> {
    deimos_core::initialize_package_manager(std::env::args().nth(1).as_deref())?;

    #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
    pub enum ExternFieldType {
        Float,
        Vec4,
        Mat4,
        U32,
        Texture,
        Uav,
    }

    let mut fields: HashSet<(ExternIndex, ExternFieldType, usize)> = Default::default();
    let mut global_channels = [0u8; 256];
    let mut unk5e = [0u8; 256];

    let mut process_constants = |dc: &SDynamicCore| {
        for op in OpcodeIterator::new(&dc.bytecode) {
            let (op, args) = match op {
                Ok(o) => o,
                Err(e) => {
                    eprintln!(
                        "Failed to parse opcode in dynamic constants, fields may be missing: {e} ({:02X?})",
                        dc.bytecode
                    );
                    break;
                }
            };

            if !matches!(
                op,
                Opcode::PushExternInputFloat
                    | Opcode::PushExternInputVec4
                    | Opcode::PushExternInputMat4
                    | Opcode::PushExternInputTextureView
                    | Opcode::PushExternInputU32
                    | Opcode::PushExternInputUav
            ) {
                match op {
                    Opcode::PushGlobalChannelVector => {
                        let channel = args[0] as usize;
                        global_channels[channel] += 1;
                    }
                    Opcode::Push0x5eChannelVector => {
                        let channel = args[0] as usize;
                        unk5e[channel] += 1;
                    }
                    _ => {}
                }

                continue;
            }

            let extern_ = ExternIndex::try_from(args[0]).unwrap();
            let offset_raw = args[1];

            match op {
                Opcode::PushExternInputFloat => {
                    fields.insert((extern_, ExternFieldType::Float, offset_raw as usize * 4));
                }
                Opcode::PushExternInputVec4 => {
                    fields.insert((extern_, ExternFieldType::Vec4, offset_raw as usize * 16));
                }
                Opcode::PushExternInputMat4 => {
                    fields.insert((extern_, ExternFieldType::Mat4, offset_raw as usize * 16));
                }
                Opcode::PushExternInputTextureView => {
                    fields.insert((extern_, ExternFieldType::Texture, offset_raw as usize * 8));
                }
                Opcode::PushExternInputU32 => {
                    fields.insert((extern_, ExternFieldType::U32, offset_raw as usize * 4));
                }
                Opcode::PushExternInputUav => {
                    fields.insert((extern_, ExternFieldType::Uav, offset_raw as usize * 8));
                }
                _ => {}
            }
        }
    };

    for (t, _) in package_manager().get_all_by_reference(SScope::ID.unwrap()) {
        let Ok(scope): tiger_parse::Result<SScope> = package_manager().read_tag_struct(t) else {
            continue;
        };
        for (s, _) in scope.iter_stages() {
            process_constants(&s.core);
        }
    }

    for (t, _) in package_manager().get_all_by_reference(STechnique::ID.unwrap()) {
        let Ok(technique): tiger_parse::Result<STechnique> = package_manager().read_tag_struct(t)
        else {
            continue;
        };
        for (_, s) in technique.all_shaders() {
            process_constants(&s.core);
        }
    }

    for ext in (0..ExternIndex::COUNT).map(|v| ExternIndex::try_from(v as u8).unwrap()) {
        let mut sfields = fields
            .iter()
            .filter(|(e, _, _)| *e == ext)
            .map(|(_, a, b)| (*a, *b))
            .collect_vec();

        sfields.sort_by_key(|(_, offset)| *offset);

        if sfields.is_empty() {
            continue;
        }

        println!("extern_struct! {{");
        println!(
            "\tstruct {ext:?}(\"{}\") {{",
            pascal_to_snake(&format!("{ext:?}"))
        );

        for (ty, offset) in sfields {
            let ty_str = match ty {
                ExternFieldType::Float => "f32",
                ExternFieldType::Vec4 => "Vec4",
                ExternFieldType::Mat4 => "Mat4",
                ExternFieldType::U32 => "u32",
                ExternFieldType::Texture => "TextureView",
                ExternFieldType::Uav => "UnorderedAccessView",
            };

            println!("\t\t0x{offset:02X} => unk{offset:02x}: {ty_str},");
        }

        println!("\t}}");
        println!("}}\n");
    }

    for (channel_index, &count) in global_channels.iter().enumerate() {
        if count > 0 {
            println!("global_channels[{channel_index}] used {count} times");
        }
    }

    for (channel_index, &count) in unk5e.iter().enumerate() {
        if count > 0 {
            println!("0x5e[{channel_index}] used {count} times");
        }
    }

    Ok(())
}
