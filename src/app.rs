#![warn(rust_2018_idioms)]
#![deny(clippy::correctness, clippy::suspicious, clippy::complexity)]
#![allow(clippy::collapsible_else_if, clippy::missing_transmute_annotations)]

use std::{
    io::{Cursor, Seek},
    rc::Rc,
    str::FromStr,
    sync::Arc,
    time::Instant,
};

use ahash::HashMap;
use anyhow::Context;
use ash::vk;
use deimos_data::{
    strings::{StringContainer, StringContainerShared},
    tag::WideHash,
};
use deimos_render::{
    gpu::{Gpu, PresentStatus},
    renderer::Renderer,
    util::fps_histogram::FrametimeHistogram,
};
use parking_lot::RwLock;
use sdl3::video::Window;
use tiger_parse::TigerReadable;
use tiger_pkg::{TagHash, package_manager};

use crate::{
    cli::AppArgs,
    config::AppConfig,
    ui::{
        Gui,
        tabs::{Tab, map::MapTab},
    },
};

pub struct App {
    pub sdl: Rc<sdl3::Sdl>,
    pub window: Rc<Window>,
    pub gpu: Arc<Gpu>,
    pub renderer: Arc<Renderer>,
    pub gui: Gui,
    pub running: bool,

    shared_state: Arc<SharedState>,

    // _spinner: FullscreenSpinner,
    last_frame_time: Instant,
    frametime_histogram: FrametimeHistogram,
}

impl App {
    pub fn new(sdl: Rc<sdl3::Sdl>, window: Rc<Window>, args: AppArgs) -> anyhow::Result<Self> {
        let gpu =
            Gpu::new(window.as_ref(), window.size_in_pixels()).context("Failed to create GPU")?;
        let renderer = Renderer::new(gpu.clone());
        *gpu.extern_source.write() =
            deimos_render::tfx::externs::BaseExternSource::Renderer(renderer.clone());

        let shared_state = SharedState::new(renderer.clone())
            .context("Failed to create shared state")?
            .into();
        let mut gui = Gui::new(&gpu, sdl.clone(), window.clone())?;
        if let Some(map_hash) = args.open_map.as_ref() {
            match TagHash::from_str(map_hash) {
                Ok(tag) => match MapTab::new(&renderer, &shared_state, tag, String::new()) {
                    Ok(tab) => gui.add_tab(Tab::Map(tab)),
                    Err(e) => error!("Failed to open map tab for {}: {:?}", map_hash, e),
                },
                Err(e) => {
                    error!("Failed to parse map hash {}: {:?}", map_hash, e);
                }
            };
        }
        // if let Some(pattern_hash) = args.open_pattern.as_ref() {
        //     match TagHash::from_str(pattern_hash) {
        //         Ok(tag) => match ModelViewTab::new_pattern(&renderer, &shared_state, tag) {
        //             Ok(tab) => gui.add_tab(Tab::ModelView(Box::new(tab))),
        //             Err(e) => error!(
        //                 "Failed to open model view tab for {}: {:?}",
        //                 pattern_hash, e
        //             ),
        //         },
        //         Err(e) => {
        //             error!("Failed to parse pattern hash {}: {:?}", pattern_hash, e);
        //         }
        //     };
        // }

        Ok(Self {
            // _spinner: FullscreenSpinner::create(&renderer.gpu)?,
            shared_state,
            renderer,
            gui,
            sdl,
            window,
            gpu,
            running: true,

            last_frame_time: Instant::now(),
            frametime_histogram: FrametimeHistogram::new(10),
        })
    }

    pub fn handle_event(&mut self, event: sdl3::event::Event) {
        #[allow(clippy::single_match, clippy::collapsible_match)]
        match &event {
            sdl3::event::Event::Quit { .. } => {
                self.running = false;
            }
            sdl3::event::Event::Window { win_event, .. } => match win_event {
                &sdl3::event::WindowEvent::Resized(..) => {
                    let (width, height) = self.window.size_in_pixels();
                    if let Err(e) = self.gpu.resize_swapchain(width, height) {
                        error!("Failed to resize swapchain: {e:?}");
                    }
                }
                &sdl3::event::WindowEvent::PixelSizeChanged(width, height) => {
                    if let Err(e) = self
                        .gpu
                        .resize_swapchain(width.max(0) as u32, height.max(0) as u32)
                    {
                        error!("Failed to resize swapchain: {e:?}");
                    }
                }
                sdl3::event::WindowEvent::CloseRequested => {
                    self.running = false;
                }
                _ => {}
            },
            _ => {}
        };

        self.gui
            .egui_sdl3
            .handle_event(&event, &self.sdl, &self.sdl.video().unwrap());
    }

    #[profiling::function]
    pub fn render(&mut self, _event_pump: &sdl3::EventPump) -> anyhow::Result<()> {
        let delta_time = self.last_frame_time.elapsed().as_secs_f32();
        self.last_frame_time = std::time::Instant::now();

        self.renderer.asset_manager.remove_unreferenced();

        self.frametime_histogram.push(delta_time);

        let gpu = &self.gpu;
        let _frame = self.gpu.begin_frame();
        let Some(image_index) = gpu.acquire_next_image()? else {
            // The swapchain went out of date (typically during a live resize).
            let (width, height) = self.window.size_in_pixels();
            if let Err(e) = self.gpu.recreate_swapchain(width, height) {
                error!("Failed to recreate out-of-date swapchain: {e:?}");
            }
            return Ok(());
        };
        let cmd = gpu.current_command_buffer();

        unsafe {
            gpu.reset_command_buffer(*cmd, vk::CommandBufferResetFlags::empty())?;
            gpu.begin_command_buffer(*cmd, &vk::CommandBufferBeginInfo::default())?;
        }

        let (backbuffer_image, backbuffer_view) = gpu.swapchain_image(image_index);
        let swapchain_extent = gpu.swapchain_extent();
        gpu.cmd_image_layout_barrier(
            *cmd,
            backbuffer_image,
            vk::ImageAspectFlags::COLOR,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags::empty(),
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        );

        {
            self.gui.draw_ui(&self.shared_state);

            {
                // let _event_scope = cmd.event_scope("App::render::ui", (255, 0, 255));
                // let _profiler_scope = self.gpu.profiler_scope(&frame.stream, "App::render::ui");
                // let (back_buffer_handle, back_buffer) = self.gpu.swapchain.lock().get_back_buffer();

                let attachment_info = vk::RenderingAttachmentInfo::default()
                    .image_view(backbuffer_view)
                    .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .clear_value(vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: [0.0, 0.0, 0.0, 1.0],
                        },
                    });

                cmd.begin_rendering(
                    (swapchain_extent.width, swapchain_extent.height),
                    &[attachment_info],
                    None,
                    vk::RenderingFlags::empty(),
                )?;

                self.gui.render(&self.gpu, &cmd);

                cmd.end_rendering();
            }
        }

        gpu.cmd_image_layout_barrier(
            *cmd,
            backbuffer_image,
            vk::ImageAspectFlags::COLOR,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            vk::AccessFlags::empty(),
        );

        unsafe {
            gpu.end_command_buffer(*cmd)?;
        }
        self.gpu.submit(image_index)?;
        let present_status = match self.gpu.present(image_index) {
            Ok(status) => status,
            Err(e) => {
                error!("Failed to present image: {e:?}");
                PresentStatus::OutOfDate
            }
        };

        if present_status != PresentStatus::Ok {
            let (width, height) = self.window.size_in_pixels();
            let result = if present_status == PresentStatus::OutOfDate {
                self.gpu.recreate_swapchain(width, height)
            } else {
                self.gpu.resize_swapchain(width, height)
            };
            if let Err(e) = result {
                error!("Failed to recreate swapchain after present: {e:?}");
            }
        }

        let config = self.shared_state.config.read();
        if config.framelimiter_enabled {
            let target_frame_delta = 1.0 / config.framerate_limit as f32;
            while self.last_frame_time.elapsed().as_secs_f32() < target_frame_delta {
                std::hint::spin_loop();
            }
        }

        profiling::finish_frame!();

        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.shared_state.save_config().ok();
    }
}

pub struct SharedState {
    pub strings: StringContainerShared,
    pub strings_by_package: HashMap<String, StringContainer>,
    pub config: RwLock<AppConfig>,
    pub renderer: Arc<Renderer>,
}

impl SharedState {
    pub fn new(renderer: Arc<Renderer>) -> anyhow::Result<Self> {
        let mut strings_by_package = HashMap::default();
        for (name, tag) in package_manager().get_named_tags_by_class(0x80808E8B) {
            let Ok(data) = package_manager().read_tag(tag) else {
                continue;
            };
            let mut cur = Cursor::new(data);
            cur.seek(std::io::SeekFrom::Start(0x10))?;
            let hash = WideHash::read_ds(&mut cur)?;
            if hash.is_none() {
                continue;
            }
            strings_by_package.insert(name, StringContainer::load(hash)?);
        }

        let s = Self {
            strings: StringContainer::load_all_global().into(),
            strings_by_package,
            config: RwLock::new(AppConfig::default()),
            renderer,
        };
        if let Err(e) = s.load_config() {
            warn!("Failed to load config: {:?}", e);
        }

        Ok(s)
    }

    pub fn load_config(&self) -> anyhow::Result<()> {
        let exe_path = std::env::current_exe()?.parent().unwrap().to_path_buf();
        let config_path = exe_path.join("config.toml");
        if config_path.exists() {
            let config_str = std::fs::read_to_string(&config_path)?;
            let config: AppConfig = toml::from_str(&config_str)?;
            *self.config.write() = config;
        }

        Ok(())
    }

    pub fn save_config(&self) -> anyhow::Result<()> {
        let exe_path = std::env::current_exe()?.parent().unwrap().to_path_buf();
        let config_path = exe_path.join("config.toml");
        let config_str = toml::to_string_pretty(&*self.config.read())?;
        std::fs::write(&config_path, config_str)?;

        Ok(())
    }

    pub fn get_string(&self, hash: u32) -> String {
        self.strings.get(hash)
    }

    pub fn get_string_by_package(&self, package: &str, hash: u32) -> String {
        self.strings_by_package
            .get(package)
            .and_then(|s| s.try_get(hash))
            .unwrap_or_else(|| self.get_string(hash))
    }
}
