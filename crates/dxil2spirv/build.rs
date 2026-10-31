use std::{env, path::PathBuf};

fn main() {
    let mut cfg = cmake::Config::new("./dxil-spirv");

    let profile = match &std::env::var("PROFILE").unwrap()[..] {
        "debug" => "Debug",
        "release" | "bench" => "Release",
        unknown => {
            eprintln!(
                "Warning: unknown Rust profile={}; defaulting to a release build.",
                unknown
            );
            "Release"
        }
    };

    let dst = cfg
        .profile(profile)
        .build_target("dxil-spirv-c-static")
        .build();

    println!(
        "cargo:rustc-link-search=native={}",
        dst.join("build").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        dst.join("build").join(profile).display()
    );
    for subdir in ["bc", "third_party"] {
        println!(
            "cargo:rustc-link-search=native={}",
            dst.join("build").join(subdir).display()
        );
        println!(
            "cargo:rustc-link-search=native={}",
            dst.join("build").join(subdir).join(profile).display()
        );
    }

    println!("cargo:rustc-link-lib=dxil-spirv-c-static");
    println!("cargo:rustc-link-lib=dxil-converter");
    println!("cargo:rustc-link-lib=spirv-module");
    println!("cargo:rustc-link-lib=glslang-spirv-builder");
    println!("cargo:rustc-link-lib=llvm-bc");
    println!("cargo:rustc-link-lib=bc-decoder");
    println!("cargo:rustc-link-lib=dxbc-spirv");
    println!("cargo:rustc-link-lib=dxil-debug");
    println!("cargo:rustc-link-lib=dxil-utils");

    let bindings = bindgen::Builder::default()
        .headers(["dxil-spirv/dxil_spirv_c.h"])
        .clang_arg("-Idxil-spirv")
        // .default_enum_style(bindgen::EnumVariation::Consts)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    println!("cargo:rerun-if-changed=dxil-spirv/CMakeLists.txt");
}
