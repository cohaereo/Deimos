use std::{collections::BTreeMap, mem::discriminant, rc::Rc, sync::Arc, thread::JoinHandle};

use ash::vk;
use deimos_render::gpu::Gpu;
use egui::{Align2, Color32, FontFamily, FontId, Margin};
use egui_dock::{DockArea, DockState, TabInteractionStyle};
use google_material_symbols::GoogleMaterialSymbols;

use crate::{
    app::SharedState,
    task::Task,
    ui::{
        tabs::{DockStateExt, Tab, TabViewer},
        util::UiExt,
    },
    updater::{AvailableUpdate, check_stable_release, execute_update},
};

pub mod colors;
mod scene;
// pub mod sodi;
mod style;
pub mod tabs;
pub mod util;

pub struct Gui {
    window: Rc<sdl3::video::Window>,
    sdl: Rc<sdl3::Sdl>,

    pub renderer: GuiRenderer,
    pub egui_sdl3: egui_sdl3_platform::Platform,
    tree: DockState<Tab>,

    added_nodes: Vec<Tab>,

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
            egui_ash_renderer::DynamicRendering {
                color_attachment_format: gpu.surface_format.format,
                depth_attachment_format: None,
            },
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
        egui_sdl3.context().style_mut(|s| {
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

        let mut tree = DockState::new(vec![Tab::Settings, Tab::Home]);
        if let Some(tab_ref) = tree.find_tab(|t| matches!(t, Tab::Home)) {
            tree.set_active_tab(tab_ref);
        }

        Ok(Self {
            window,
            sdl,
            renderer: GuiRenderer::new(gpu.clone(), renderer)?,
            egui_sdl3,
            tree,
            added_nodes: Vec::new(),
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
        self.added_nodes.push(tab);
    }

    pub fn draw_ui(&mut self, shared_state: &Arc<SharedState>) {
        let ctx = self
            .egui_sdl3
            .begin_frame(self.window.size(), self.window.display_scale());
        ctx.style_mut(|s| s.visuals.panel_fill = Color32::from_black_alpha(96));

        DockArea::new(&mut self.tree)
            .show_add_buttons(false)
            .style({
                let mut style = egui_dock::Style::from_egui(ctx.style().as_ref());
                // style.tab_bar.fill_tab_bar = true;
                style.tab_bar.height = 32.0;
                style.tab_bar.bg_fill = Color32::from_gray(4);

                let inactive = TabInteractionStyle {
                    outline_color: Color32::TRANSPARENT,
                    corner_radius: egui::CornerRadius::ZERO,
                    bg_fill: Color32::BLACK,
                    text_color: Color32::WHITE,
                };

                let hovered = TabInteractionStyle {
                    outline_color: Color32::from_gray(127),
                    bg_fill: ctx.style().visuals.window_fill().gamma_multiply(0.5),
                    ..inactive
                };

                let active = TabInteractionStyle {
                    bg_fill: ctx.style().visuals.window_fill(),
                    ..hovered
                };

                let focused = TabInteractionStyle {
                    outline_color: Color32::WHITE,
                    bg_fill: ctx.style().visuals.window_fill(),
                    ..inactive
                };

                style.tab = egui_dock::TabStyle {
                    active: active.clone(),
                    inactive: inactive.clone(),
                    focused: focused.clone(),
                    hovered,
                    inactive_with_kb_focus: inactive,
                    active_with_kb_focus: active,
                    focused_with_kb_focus: focused,
                    tab_body: egui_dock::TabBodyStyle {
                        inner_margin: ctx.style().spacing.window_margin,
                        stroke: ctx.style().visuals.widgets.noninteractive.bg_stroke,
                        corner_radius: ctx.style().visuals.widgets.active.corner_radius,
                        bg_fill: ctx.style().visuals.window_fill(),
                        // bg_fill: Color32::from_black_alpha(128),
                    },
                    hline_below_active_tab_name: false,
                    ..Default::default()
                };
                style
            })
            .show_leaf_collapse_buttons(false)
            .show_leaf_close_all_buttons(false)
            .draggable_tabs(false)
            .allowed_splits(egui_dock::AllowedSplits::None)
            .show(
                &ctx,
                &mut TabViewer {
                    added_nodes: &mut self.added_nodes,
                    renderer: &mut self.renderer,
                    shared_state,
                },
            );

        for tab in self.added_nodes.drain(..) {
            // Is the tab unique and does it already exist? Then switch to it instead of adding it again.
            if let Some(tab_ref) = self
                .tree
                .find_tab(|t| discriminant(t) == discriminant(&tab) && t.key() == tab.key())
            {
                self.tree.set_active_tab(tab_ref);
            } else {
                self.tree.push_to_focused_leaf(tab);
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
                .frame(egui::Frame::popup(&ctx.style()).inner_margin(Margin::symmetric(64, 48)))
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
    }

    pub fn render(&mut self, gpu: &Arc<Gpu>, cmd: &vk::CommandBuffer) {
        let output = self
            .egui_sdl3
            .end_frame(&mut self.sdl.video().unwrap())
            .unwrap();

        self.renderer
            .draw(self.egui_sdl3.context(), output, cmd)
            .unwrap();
    }
}

pub struct GuiRenderer {
    gpu: Arc<Gpu>,
    pub renderer: egui_ash_renderer::Renderer,
    temporary_textures: Vec<egui::TextureId>,

    descriptor_layout: vk::DescriptorSetLayout,
}

impl GuiRenderer {
    pub fn new(gpu: Arc<Gpu>, renderer: egui_ash_renderer::Renderer) -> anyhow::Result<Self> {
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
        output: egui::FullOutput,
        cmd: &vk::CommandBuffer,
    ) -> anyhow::Result<()> {
        let pixels_per_point = ctx.pixels_per_point();
        let primitives = ctx.tessellate(output.shapes, pixels_per_point);

        self.renderer
            .set_textures(
                self.gpu.queue,
                self.gpu.command_pool,
                output.textures_delta.set.as_slice(),
            )
            .unwrap();

        if let Err(e) = self.renderer.cmd_draw(
            *cmd,
            self.gpu.swapchain_extent,
            pixels_per_point,
            &primitives,
        ) {
            error!("Failed to draw egui primitives: {:?}", e);
        }

        self.renderer
            .free_textures(output.textures_delta.free.as_slice())
            .unwrap();

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
