use std::{fs::File, io::Write, rc::Rc, sync::Mutex, thread};

use anyhow::Context;
use app::App;
use clap::Parser;
use cli::AppArgs;
use deimos_core::job::SCHEDULER;
use itertools::Itertools;
use tracing_subscriber::{
    Layer,
    filter::{EnvFilter, FilterExt, LevelFilter, Targets, filter_fn},
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

use crate::{
    cli::{DEIMOS_VERSION, print_banner},
    panic_hook::write_system_info,
};

mod app;
mod cli;
mod config;
mod panic_hook;
mod task;
mod ui;
mod updater;
mod world;

#[macro_use]
extern crate tracing;

// #[cfg(all(feature = "dhat-heap", not(feature = "tracy")))]
// #[global_allocator]
// static ALLOC: dhat::Alloc = dhat::Alloc;

// #[cfg(feature = "tracy")]
// #[global_allocator]
// static GLOBAL: tracy_client::ProfiledAllocator<std::alloc::System> =
//     tracy_client::ProfiledAllocator::new(std::alloc::System, 100);

const TRACING_DISABLED_TARGETS: &[&str] = &["ehttp", "ureq", "rustls"];

fn main() -> anyhow::Result<()> {
    rayon::ThreadPoolBuilder::new()
        .thread_name(|i| format!("rayon-pool-{i}"))
        .num_threads(SCHEDULER.num_workers())
        .build_global()?;

    fix_windows_console();
    print_banner();
    std::panic::set_hook(Box::new(panic_hook::hook));
    let mut log_file = File::create("deimos.log").context("creating log file")?;
    let mut system_info = String::new();
    write_system_info(&mut system_info);
    log_file.write_all(system_info.as_bytes())?;
    writeln!(&mut log_file)?;

    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    let mut file_filter = Targets::new().with_default(LevelFilter::DEBUG);
    for target in TRACING_DISABLED_TARGETS {
        file_filter = file_filter.with_target(*target, LevelFilter::OFF);
    }

    let file_layer = fmt::layer()
        .with_file(false)
        .with_ansi(false)
        .with_writer(Mutex::new(log_file))
        .with_filter(file_filter);

    let stderr_filter = env_filter.and(filter_fn(|meta| {
        !TRACING_DISABLED_TARGETS
            .iter()
            .any(|t| meta.target().starts_with(t))
    }));

    let stderr_layer = fmt::layer()
        .with_file(false)
        .with_writer(std::io::stderr)
        .with_filter(stderr_filter);

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stderr_layer)
        .init();

    let args = AppArgs::parse();

    deimos_core::initialize_package_manager(args.gamedir.as_deref())?;

    // Fixes display scaling on Wayland
    sdl3::hint::set("SDL_VIDEO_WAYLAND_SCALE_TO_DISPLAY", "0");

    let sdl_context = Rc::new(sdl3::init().expect("Failed to initialize SDL"));
    let video_subsystem = sdl_context
        .video()
        .expect("Failed to initialize video subsystem");

    let mut window = {
        let mut builder = video_subsystem.window("Deimos", 1920, 1080);

        let mut builder_ref = builder
            .position_centered()
            .resizable()
            .maximized()
            .high_pixel_density();

        if cfg!(not(target_os = "windows")) {
            builder_ref = builder_ref.vulkan();
        }

        builder_ref.build().expect("Failed to create window")
    };

    if !window.sync() {
        tracing::warn!("Timed out waiting for the window to reach its final state");
    }

    if let Some(display_index) = args.display {
        let displays = video_subsystem.displays()?;
        let Some(display) = displays.get(display_index) else {
            anyhow::bail!(
                "Invalid display index (available displays: {:?})",
                displays.iter().enumerate().map(|(i, _d)| i).collect_vec()
            );
        };
        let display_center = display.get_bounds()?.center();
        let window_size = window.size();
        window.set_position(
            sdl3::video::WindowPos::Positioned(display_center.x - window_size.0 as i32 / 2),
            sdl3::video::WindowPos::Positioned(display_center.y - window_size.1 as i32 / 2),
        );
    }

    let mut app =
        App::new(sdl_context.clone(), Rc::new(window), args).expect("Failed to initialize app");

    let mut event_pump = sdl_context.event_pump().unwrap();
    'app: while app.running {
        for event in event_pump.poll_iter() {
            match event {
                sdl3::event::Event::Quit { .. } => break 'app,
                _ => app.handle_event(event),
            }
        }

        app.render(&event_pump).expect("failed to render frame");
    }

    info!("Application shutting down...");
    app.gpu.wait_idle()?;
    app.renderer.shutdown();

    tiger_pkg::finalize_package_manager();

    Ok(())
}

fn fix_windows_console() {
    #[cfg(target_os = "windows")]
    {
        pub type Handle = *mut std::ffi::c_void;

        unsafe extern "C" {
            fn SetConsoleMode(handle: Handle, mode: u32) -> i32;
            fn GetStdHandle(handle: u32) -> Handle;
        }

        const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
        const ENABLE_PROCESSED_OUTPUT: u32 = 1u32;
        const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 4u32;
        unsafe {
            let stdout = GetStdHandle(STD_OUTPUT_HANDLE);
            if !stdout.is_null() {
                SetConsoleMode(
                    stdout,
                    ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING,
                );
            }
        }
    }
}

// Workaround for subsecond missing this symbol while linking (even though its not used)
#[unsafe(no_mangle)]
#[cfg(not(target_os = "windows"))]
extern "C" fn CoCreateGuid() {}
