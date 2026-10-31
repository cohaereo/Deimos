use std::fmt::Write;
use std::{backtrace::Backtrace, panic::PanicHookInfo};

use deimos_render::gpu::device_iterator::PhysicalDeviceIterator;
use deimos_render::gpu::vk_api_version_string;
use nu_ansi_term::{Color, Style};
use sysinfo::System;

use crate::ui::util::format_bytes;

pub fn hook(panic: &PanicHookInfo<'_>) {
    let message = if let Some(s) = panic.payload().downcast_ref::<&str>() {
        Some((*s).to_string())
    } else {
        panic.payload().downcast_ref::<String>().cloned()
    };
    let location = panic.location().unwrap();
    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("Unknown thread");

    let style = Style::new().fg(Color::LightMagenta).bold();
    let mut msg = String::new();
    writeln!(
        &mut msg,
        "Thread '{}' panicked at {}:\n{}",
        thread_name,
        location,
        message.unwrap_or_else(|| "Unknown panic".to_owned())
    )
    .ok();

    let bt = Backtrace::capture();
    match bt.status() {
        std::backtrace::BacktraceStatus::Unsupported => {
            writeln!(&mut msg, "Backtrace is not supported").ok();
        }
        std::backtrace::BacktraceStatus::Disabled => {
            writeln!(
                &mut msg,
                "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace"
            )
            .ok();
        }
        std::backtrace::BacktraceStatus::Captured => {
            writeln!(&mut msg, "Backtrace:\n{bt}").ok();
        }
        u => {
            writeln!(&mut msg, "Unknown backtrace status: {:?}", u).ok();
        }
    }

    eprint!("{}", style.paint(&msg));

    // Dont show dialog on debug builds
    if !cfg!(debug_assertions) {
        // Finally, show a dialog
        let panic_message_stripped = strip_ansi_codes(&msg);
        if let Err(e) = native_dialog::MessageDialog::new()
            .set_type(native_dialog::MessageType::Error)
            .set_title("Deimos crashed!")
            .set_text(&format!(
                "{}\n\nA full crash log has been written to panic.log. Please attach panic.log and deimos.log when reporting this issue to the developer.",
                panic_message_stripped
            ))
            .show_alert()
        {
            eprintln!("Failed to show error dialog: {e}");
        }
    }

    writeln!(&mut msg).ok();
    write_system_info(&mut msg);

    std::fs::write("panic.log", msg.clone()).ok();
}

pub fn write_system_info<R: std::fmt::Write>(out: &mut R) {
    let s = System::new_with_specifics(
        sysinfo::RefreshKind::nothing()
            .with_memory(sysinfo::MemoryRefreshKind::everything())
            .with_cpu(sysinfo::CpuRefreshKind::everything()),
    );

    let is_wine = std::fs::read_to_string("/proc/version")
        .unwrap_or_default()
        .contains("Linux");

    writeln!(out, "System info:").ok();
    writeln!(
        out,
        "  Operating System: {} ({})",
        System::long_os_version().unwrap_or_else(|| "unknown".to_string()),
        if is_wine { "Wine/Proton" } else { "Microsoft" }
    )
    .ok();
    if let Some(cpu) = s.cpus().first() {
        writeln!(
            out,
            "  CPU: {} ({}), {} MHz, {} cores, {} threads",
            cpu.brand(),
            cpu.vendor_id(),
            cpu.frequency(),
            System::physical_core_count().unwrap_or(0),
            s.cpus().len()
        )
        .ok();
    }
    writeln!(
        out,
        "  Memory: {} free, {} total",
        format_bytes(s.free_memory() as usize),
        format_bytes(s.total_memory() as usize)
    )
    .ok();

    if let Ok(device_iter) = PhysicalDeviceIterator::new() {
        for (i, (_dev, desc)) in device_iter.enumerate() {
            writeln!(
                out,
                "  GPU {}: {} (vendor: 0x{:04X}, device: 0x{:04X}), Vulkan {}, {} VRAM",
                i,
                desc.description,
                desc.vendor_id,
                desc.device_id,
                vk_api_version_string(desc.api_version),
                format_bytes(desc.device_local_memory as usize)
            )
            .ok();
        }
    }
}

pub fn strip_ansi_codes(input: &str) -> String {
    let ansi_escape_pattern = regex::Regex::new(r"\x1B\[[0-9;]*[mK]").unwrap();
    ansi_escape_pattern.replace_all(input, "").to_string()
}
