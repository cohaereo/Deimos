use std::{collections::BTreeMap, mem::discriminant, rc::Rc, sync::Arc, thread::JoinHandle};

use ash::vk;
use deimos_render::gpu::Gpu;
use egui::{Align2, Color32, FontFamily, FontId, LayerId, Margin, Vec2, pos2, vec2};
use google_material_symbols::GoogleMaterialSymbols;

use crate::{
    app::SharedState,
    task::Task,
    ui::{
        colors::MARATHON_GREEN,
        tabs::{Tab, TabViewer},
        util::UiExt,
    },
    updater::{AvailableUpdate, check_stable_release, execute_update},
};

pub mod colors;
mod scene;
// pub mod sodi;
pub mod icons;
mod style;
pub mod tabs;
pub mod util;

pub struct Gui {
    window: Rc<sdl3::video::Window>,
    sdl: Rc<sdl3::Sdl>,

    pub renderer: GuiRenderer,
    pub egui_sdl3: egui_sdl3_platform::Platform,

    tabs: Vec<Tab>,
    added_tabs: Vec<Tab>,
    active_tab: usize,

    // sodi: SodiVow,
    update_check: Task<Option<AvailableUpdate>>,
    available_update: Option<AvailableUpdate>,

    update_thread: Option<JoinHandle<()>>,

    commonmark_cache: egui_commonmark::CommonMarkCache,
}

impl Gui {
    pub fn new(
        gpu: &Arc<Gpu>,
        sdl: Rc<sdl3::Sdl>,
        window: Rc<sdl3::video::Window>,
    ) -> anyhow::Result<Self> {
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "ppfraktionmono".into(),
            Arc::new(egui::FontData::from_static(include_bytes!(
                "../../assets/fonts/ppfraktionmono-regular.otf"
            ))),
        );
        fonts.font_data.insert(
            "ppfraktionmono-bold".into(),
            Arc::new(egui::FontData::from_static(include_bytes!(
                "../../assets/fonts/ppfraktionmono-bold.otf"
            ))),
        );
        fonts.font_data.insert(
            "marathonshapiro_wide".into(),
            Arc::new(egui::FontData::from_static(include_bytes!(
                "../../assets/fonts/marathonshapiro_wide65.otf"
            ))),
        );
        fonts.font_data.insert(
            "khinterference-regular".into(),
            Arc::new(egui::FontData::from_static(include_bytes!(
                "../../assets/fonts/khinterference-regular.otf"
            ))),
        );
        fonts.font_data.insert(
            "MaterialSymbolsRounded-Medium".into(),
            Arc::new(egui::FontData::from_static(
                GoogleMaterialSymbols::FONT_BYTES,
            )),
        );
        fonts.font_data.insert(
            "GoliathBeta-Encrypted".into(),
            Arc::new(egui::FontData::from_static(include_bytes!(
                "../../assets/fonts/GoliathBeta-Encrypted.otf"
            ))),
        );

        let mut add_with_icons = |family: egui::FontFamily, elements: &[&str]| {
            for (i, &element) in elements.iter().enumerate() {
                fonts
                    .families
                    .entry(family.clone())
                    .or_default()
                    .insert(i, element.to_owned());
            }

            fonts
                .families
                .entry(family)
                .or_default()
                .insert(elements.len(), "MaterialSymbolsRounded-Medium".to_owned());
        };

        add_with_icons(
            egui::FontFamily::Proportional,
            &["ppfraktionmono", "ppfraktionmono-bold"],
        );
        add_with_icons(
            egui::FontFamily::Monospace,
            &["ppfraktionmono", "ppfraktionmono-bold"],
        );
        add_with_icons(
            egui::FontFamily::Name("khinterference".into()),
            &["khinterference-regular"],
        );
        add_with_icons(
            egui::FontFamily::Name("shapiro".into()),
            &["marathonshapiro_wide"],
        );
        fonts
            .families
            .entry(egui::FontFamily::Name("encrypted".into()))
            .or_default()
            .insert(0, "GoliathBeta-Encrypted".into());

        let renderer = egui_ash_renderer::Renderer::with_default_allocator(
            &gpu.instance,
            gpu.physical_device,
            gpu.device.clone(),
            egui_ash_renderer::RenderMode::DynamicRendering(egui_ash_renderer::DynamicRendering {
                color_attachment_format: gpu.surface_format.format,
                depth_attachment_format: None,
                stencil_attachment_format: None,
            }),
            egui_ash_renderer::Options {
                in_flight_frames: Gpu::MAX_FRAMES_IN_FLIGHT,
                enable_depth_test: false,
                enable_depth_write: false,
                srgb_framebuffer: false,
            },
        )
        .unwrap();

        let egui_sdl3 =
            egui_sdl3_platform::Platform::new(&sdl, &window, gpu.swapchain_resolution())?;
        egui_sdl3.context().set_fonts(fonts);
        egui_sdl3.context().global_style_mut(|s| {
            *s = style::gui_style();
        });

        // Redefine text_styles
        let text_styles: BTreeMap<_, _> = [
            (
                egui::TextStyle::Heading,
                FontId::new(42.0, egui::FontFamily::Name("shapiro".into())),
            ),
            (
                egui::TextStyle::Body,
                FontId::new(18.0, egui::FontFamily::Proportional),
            ),
            (
                egui::TextStyle::Monospace,
                FontId::new(14.0, egui::FontFamily::Proportional),
            ),
            (
                egui::TextStyle::Button,
                FontId::new(24.0, egui::FontFamily::Name("khinterference".into())),
            ),
            (
                egui::TextStyle::Small,
                FontId::new(10.0, egui::FontFamily::Proportional),
            ),
        ]
        .into();

        // Mutate global styles with new text styles
        egui_sdl3
            .context()
            .all_styles_mut(move |style| style.text_styles = text_styles.clone());

        egui_extras::install_image_loaders(egui_sdl3.context());

        let tabs = vec![Tab::Settings, Tab::Home];
        let active_tab = tabs
            .iter()
            .position(|t| matches!(t, Tab::Home))
            .unwrap_or(0);

        Ok(Self {
            window,
            sdl,
            renderer: GuiRenderer::new(gpu.clone(), renderer)?,
            egui_sdl3,
            tabs,
            added_tabs: vec![],
            active_tab,
            // sodi: SodiVow::default(),
            update_check: Task::new("updater".to_string(), || match check_stable_release() {
                Ok(Some(update)) => Some(update),
                Ok(None) => None,
                e => {
                    error!("Failed to check for update: {:?}", e);
                    None
                }
            }),
            available_update: None,
            update_thread: None,
            commonmark_cache: egui_commonmark::CommonMarkCache::default(),
        })
    }

    pub fn add_tab(&mut self, tab: Tab) {
        self.added_tabs.push(tab);
    }

    pub fn draw_ui(&mut self, shared_state: &Arc<SharedState>) -> egui::FullOutput {
        let (ctx, raw_input) = self
            .egui_sdl3
            .begin_frame(self.window.size_in_pixels(), self.window.display_scale());
        ctx.global_style_mut(|s| s.visuals.panel_fill = Color32::from_black_alpha(96));

        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len().saturating_sub(1);
        }

        let mut remove_tab = None;
        let output = ctx.run_ui(raw_input, |ui| {
            ui.layer_painter(egui::LayerId::background()).rect_filled(
                ui.content_rect(),
                0,
                ui.style().visuals.window_fill,
            );

            let old_spacing = ui.style_mut().spacing.item_spacing;
            ui.style_mut().spacing.item_spacing = egui::Vec2::ZERO;
            ui.horizontal_wrapped(|ui| {
                const TAB_PADDING: Vec2 = vec2(8.0, 0.0);
                const TAB_FONT: FontId = FontId::proportional(20.0);
                const TAB_LED_HEIGHT: f32 = 3.0;
                const TAB_CLOSE_BUTTON_WIDTH: f32 = 16.0;

                let tab_bar_painter =
                    ui.layer_painter(LayerId::new(egui::Order::Foreground, "tab_bar".into()));

                for (index, tab) in self.tabs.iter().enumerate() {
                    let title = tab.title();
                    let title_layout =
                        ui.painter()
                            .layout(title.clone(), TAB_FONT, Color32::WHITE, f32::INFINITY);

                    let mut width_padded = title_layout.rect.width() + TAB_PADDING.x * 2.0;
                    if !tab.is_fixed() {
                        width_padded += TAB_CLOSE_BUTTON_WIDTH;
                    }
                    let height_padded =
                        title_layout.rect.height() + TAB_PADDING.y * 2.0 + TAB_LED_HEIGHT;

                    let (rect, resp) = ui.allocate_exact_size(
                        vec2(width_padded, height_padded),
                        egui::Sense::click(),
                    );

                    let tab_bg = if index == self.active_tab || resp.hovered() {
                        Color32::from_gray(64)
                    } else {
                        Color32::from_gray(32)
                    };
                    tab_bar_painter.rect_filled(rect, 0, tab_bg);
                    if index == self.active_tab {
                        tab_bar_painter.rect_filled(
                            rect.with_max_y(rect.min.y + TAB_LED_HEIGHT),
                            0,
                            MARATHON_GREEN,
                        );
                    }

                    let rect = rect.with_min_y(rect.min.y + TAB_LED_HEIGHT);
                    tab_bar_painter.text(
                        pos2(rect.min.x + TAB_PADDING.x, rect.min.y + rect.height() / 2.0),
                        egui::Align2::LEFT_CENTER,
                        title,
                        TAB_FONT,
                        if index == self.active_tab {
                            Color32::WHITE
                        } else if resp.hovered() {
                            Color32::from_gray(226)
                        } else {
                            Color32::from_gray(196)
                        },
                    );

                    if !tab.is_fixed() && resp.hovered() {
                        let close_pos = pos2(
                            rect.max.x - TAB_CLOSE_BUTTON_WIDTH / 2.0 - TAB_PADDING.x / 2.0,
                            rect.center().y,
                        );
                        let close_rect = egui::Rect::from_center_size(
                            close_pos,
                            vec2(TAB_CLOSE_BUTTON_WIDTH, TAB_CLOSE_BUTTON_WIDTH),
                        );
                        tab_bar_painter.text(
                            close_pos,
                            egui::Align2::CENTER_CENTER,
                            "×",
                            FontId::proportional(20.0),
                            Color32::from_gray(196),
                        );

                        if let Some(cursor_pos) = ui.pointer_hover_pos()
                            && close_rect.contains(cursor_pos)
                        {
                            let clicked =
                                ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary));
                            tab_bar_painter.rect_filled(
                                close_rect,
                                2,
                                if clicked {
                                    Color32::from_white_alpha(64)
                                } else {
                                    Color32::from_white_alpha(128)
                                },
                            );

                            if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
                            {
                                remove_tab = Some(index);
                            }
                        }
                    }

                    resp.context_menu(|ui| {
                        if ui.button("Close").clicked() {
                            remove_tab = Some(index);
                        }
                    });
                    if resp.clicked_by(egui::PointerButton::Middle) {
                        remove_tab = Some(index);
                    }

                    if resp.clicked() {
                        self.active_tab = index;
                    }
                }
            });

            let tab_end_height = ui.cursor().min.y;

            ui.layer_painter(egui::LayerId::background()).rect_filled(
                ui.content_rect().with_max_y(tab_end_height),
                0,
                Color32::from_gray(16),
            );

            let mut tab_viewer = TabViewer {
                added_nodes: &mut self.added_tabs,
                renderer: &mut self.renderer,
                shared_state,
            };

            let active_tab = self.active_tab;
            if let Some(tab) = self.tabs.get_mut(active_tab) {
                ui.style_mut().spacing.item_spacing = old_spacing;
                tab_viewer.ui(ui, tab);
            }
        });

        if let Some(index) = remove_tab
            && !self.tabs[index].is_fixed()
        {
            self.tabs.remove(index);
        }

        for tab in self.added_tabs.drain(..) {
            // Is the tab unique and does it already exist? Then switch to it instead of adding it again.
            if let Some(tab_index) = self
                .tabs
                .iter()
                .position(|t| discriminant(t) == discriminant(&tab) && t.key() == tab.key())
            {
                self.active_tab = tab_index;
            } else {
                self.tabs.push(tab);
                self.active_tab = self.tabs.len() - 1;
            }
        }

        // {
        //     let painter = ctx.layer_painter(egui::LayerId::new(
        //         egui::Order::Foreground,
        //         egui::Id::new("sodi"),
        //     ));

        //     painter.text(
        //         ctx.content_rect().left_bottom() + vec2(24.0, -16.0),
        //         egui::Align2::LEFT_BOTTOM,
        //         format!(
        //             "{} TEST BUILD, DO NOT DISTRIBUTE",
        //             GoogleMaterialSymbols::Lock
        //         ),
        //         egui::FontId::new(48.0, egui::FontFamily::Name("shapiro".into())),
        //         egui::Color32::from_white_alpha(127),
        //     );
        // }

        // self.sodi.draw(&ctx);

        if let Some(update) = self.update_check.get() {
            match update {
                Ok(update) => {
                    self.available_update = update;
                }
                Err(e) => {
                    error!("Failed to check for update: {:?}", e);
                }
            }
        }

        let mut close_update_window = false;
        if let Some(update) = self.available_update.as_ref() {
            egui::Modal::new("update_available".into())
                .frame(
                    egui::Frame::popup(&ctx.global_style()).inner_margin(Margin::symmetric(64, 48)),
                )
                .show(&ctx, |ui| {
                    ui.heading("Update available!");
                    ui.label(format!(
                        "Release '{}' is available for download",
                        update.version
                    ));
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .max_height(ui.ctx().viewport_rect().height() * 0.6)
                        .show(ui, |ui| {
                            egui_commonmark::CommonMarkViewer::new().show(
                                ui,
                                &mut self.commonmark_cache,
                                update.changelog.as_str(),
                            );
                        });

                    ui.add_space(32.0);
                    ui.horizontal(|ui| {
                        if ui
                            .d_button(format!("{} Close", GoogleMaterialSymbols::Close))
                            .clicked()
                        {
                            close_update_window = true;
                        }
                        if ui
                            .d_button(format!(
                                "{} Open in Browser",
                                GoogleMaterialSymbols::OpenInNew
                            ))
                            .clicked()
                        {
                            ctx.open_url(egui::OpenUrl::new_tab(&update.url));
                        }
                        if ui
                            .d_button(format!("{} Download", GoogleMaterialSymbols::Download))
                            .clicked()
                        {
                            let download_url = update.download_url.clone();
                            self.update_thread = Some(std::thread::spawn(move || {
                                match ehttp::fetch_blocking(&ehttp::Request::get(download_url)) {
                                    Ok(response) => {
                                        info!(
                                            "Successfully downloaded update: {} bytes",
                                            response.bytes.len()
                                        );

                                        execute_update(response.bytes)
                                            .expect("Failed to execute update");
                                    }
                                    Err(e) => {
                                        panic!("Failed to download update: {e:?}");
                                    }
                                }
                            }));
                            close_update_window = true;
                        }
                    });
                });
        }

        if close_update_window {
            self.available_update = None;
        }

        if let Some(_thread) = &self.update_thread {
            egui::Modal::new("update_available".into())
                .frame(egui::Frame::default().inner_margin(64.0))
                .show(&ctx, |ui| {
                    let time = ui.input(|i| i.time);
                    let dots = (time * 3.0) as usize % 3;
                    ui.painter().text(
                        ctx.viewport_rect().center(),
                        Align2::CENTER_CENTER,
                        format!(
                            "{} Updating{}{}",
                            GoogleMaterialSymbols::Downloading,
                            ".".repeat(dots),
                            " ".repeat(3 - dots),
                        ),
                        egui::FontId::new(64.0, FontFamily::Name("shapiro".into())),
                        Color32::WHITE,
                    );
                });
        }

        self.egui_sdl3
            .end_frame(&mut self.sdl.video().unwrap(), output)
            .unwrap()
    }

    pub fn render(&mut self, _gpu: &Arc<Gpu>, cmd: &vk::CommandBuffer, output: egui::FullOutput) {
        self.renderer
            .draw(self.egui_sdl3.context(), output, cmd)
            .unwrap();
    }
}

pub struct GuiRenderer {
    gpu: Arc<Gpu>,
    pub renderer: egui_ash_renderer::Renderer<egui_ash_renderer::allocator::DefaultAllocator>,
    temporary_textures: Vec<egui::TextureId>,

    descriptor_layout: vk::DescriptorSetLayout,
}

impl GuiRenderer {
    pub fn new(
        gpu: Arc<Gpu>,
        renderer: egui_ash_renderer::Renderer<egui_ash_renderer::allocator::DefaultAllocator>,
    ) -> anyhow::Result<Self> {
        let descriptor_layout =
            egui_ash_renderer::vulkan::create_vulkan_descriptor_set_layout(&gpu.device)?;

        Ok(Self {
            gpu,
            renderer,
            temporary_textures: vec![],

            descriptor_layout,
        })
    }

    pub fn draw(
        &mut self,
        ctx: &egui::Context,
        mut output: egui::FullOutput,
        cmd: &vk::CommandBuffer,
    ) -> anyhow::Result<()> {
        let pixels_per_point = ctx.pixels_per_point();
        let primitives = ctx.tessellate(output.shapes, pixels_per_point);

        for (id, deltas) in output.textures_delta.set.drain() {
            for delta in deltas {
                self.renderer
                    .set_texture(self.gpu.queue, self.gpu.command_pool, id, &delta)
                    .unwrap();
            }
        }

        if let Err(e) = self.renderer.cmd_draw(
            *cmd,
            self.gpu.swapchain_extent(),
            pixels_per_point,
            &primitives,
        ) {
            error!("Failed to draw egui primitives: {:?}", e);
        }

        for id in output.textures_delta.free.drain() {
            if let Err(e) = self.renderer.free_texture(id) {
                error!("Failed to free egui texture: {:?}", e);
            }
        }

        for id in self.temporary_textures.drain(..) {
            self.renderer.remove_user_texture(id);
        }

        Ok(())
    }

    pub fn add_user_texture(&mut self, view: vk::ImageView) -> egui::TextureId {
        let descriptor_set = egui_ash_renderer::vulkan::create_vulkan_descriptor_set(
            &self.gpu,
            self.descriptor_layout,
            self.gpu.frame().descriptor_pool,
            view,
            self.gpu.default_sampler,
        )
        .expect("Failed to create descriptor set");

        let id = self.renderer.add_user_texture(descriptor_set);
        self.temporary_textures.push(id);
        id
    }
}
