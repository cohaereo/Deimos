use std::str::FromStr;

use anyhow::Context;
use deimos_data::tfx::STechnique;
use deimos_render::tfx::expression_vm::{self, decompiler::DecompilerState};
use itertools::Itertools;
use tiger_parse::PackageManagerExt;
use tiger_pkg::{TagHash, package_manager};

fn main() -> anyhow::Result<()> {
    let Some(hash) = std::env::args().nth(1) else {
        anyhow::bail!("Usage: technique_decompile <package dir> <technique tag>");
    };

    let Ok(hash) = TagHash::from_str(&hash) else {
        anyhow::bail!("Invalid technique tag hash: {}", hash);
    };

    deimos_core::initialize_package_manager(None)?;

    let no_ansi = std::env::args().any(|arg| arg == "--no-ansi");

    let tech: STechnique = package_manager()
        .read_tag_struct(hash)
        .context("Failed to read/parse tag")?;

    for (stage, shader) in tech.all_valid_shaders() {
        if shader.core.constant_buffer.is_none() && shader.core.bytecode.is_empty() {
            continue;
        }

        println!("// Stage: {stage:?}");
        println!("\t// Initial constants:");
        for (i, constant) in shader.core.unk30.iter().enumerate() {
            println!("\t  [{i}]: {}", constant);
        }

        println!("\t// Disassembly:");
        match expression_vm::disassemble(&shader.core.bytecode) {
            Ok(lines) => {
                for line in lines {
                    println!("\t{line}");
                }
            }
            Err(e) => {
                println!("\t// Failed to disassemble expression: {e}");
                continue;
            }
        }

        println!("\t// Decompiled assignments:");
        match DecompilerState::new(&shader.core.bytecode)
            .with_ansi(!no_ansi)
            .evaluate(&shader.core.bytecode_constants)
        {
            Ok(o) => {
                println!("\t{}", o.pretty_print().split("\n").join("\n\t"));
            }
            Err(e) => println!("Failed to decompile expression: {e:?}"),
        };
    }

    Ok(())
}
