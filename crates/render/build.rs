use std::env;
use std::path::Path;
use std::process::Command;

const SHADER_DIR: &str = "builtin/shaders";

const RASTER_SHADERS: &[&str] = &[
    "apply_hdri_as_background",
    "apply_hdri_as_light",
    "generate_shadow_mask",
    "immediate",
    "ssr",
    "cortao_apply",
    "overdraw",
];

const COMPUTE_SHADERS: &[&str] = &["cortao_prepass", "cortao_main", "cortao_filter"];

fn main() {
    println!("cargo:rerun-if-changed={}", SHADER_DIR);
    println!("cargo:rerun-if-changed=build.rs");

    if Command::new("dxc").arg("--version").output().is_err() {
        println!("cargo:warning=dxc not found on PATH; using existing .dxil files as fallback");
        return;
    }

    let dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join(SHADER_DIR);

    for name in RASTER_SHADERS {
        compile(&dir, name, "ps", "mainPS");
        compile(&dir, name, "vs", "mainVS");
    }
    for name in COMPUTE_SHADERS {
        compile(&dir, name, "cs", "main");
    }
}

fn compile(dir: &Path, name: &str, stage: &str, entry: &str) {
    let source = dir.join(format!("{name}.hlsl"));
    println!("cargo:rerun-if-changed={}", source.display());

    let output = dir.join(format!("{name}.{stage}.dxil"));
    let result = Command::new("dxc")
        .arg("-Fo")
        .arg(&output)
        .arg(&source)
        .arg("-T")
        .arg(format!("{stage}_6_7"))
        .arg("-E")
        .arg(entry)
        .output();

    match result {
        Ok(out) if out.status.success() => {}
        Ok(out) => panic!(
            "dxc failed for {}:\n{}",
            output.display(),
            String::from_utf8_lossy(&out.stderr)
        ),
        Err(e) => {
            panic!("failed to execute dxc: {}", e);
        }
    }
}
