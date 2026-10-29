use crate::auth::GameAccount;
use crate::log_error;
use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect,
    RichText, Stroke, StrokeKind, TextureHandle, TextureOptions, Vec2,
};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const YELLOW: Color32 = Color32::from_rgb(255, 255, 0);
const BTN_BG: Color32 = Color32::from_rgb(64, 64, 64);
const BTN_HOVER: Color32 = Color32::from_rgb(89, 89, 89);

fn resolve_asset_path(asset_name: &str) -> PathBuf {
    let p1 = PathBuf::from("../data").join(asset_name);
    if p1.exists() {
        return p1;
    }
    let p2 = PathBuf::from("data").join(asset_name);
    if p2.exists() {
        return p2;
    }

    let xdg_data_home = env::var("XDG_DATA_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".local/share"))
        });

    if let Some(xdg) = xdg_data_home {
        let user_path = xdg.join("rlshim/data").join(asset_name);
        if user_path.exists() {
            return user_path;
        }
    }

    let global_paths = [
        PathBuf::from("/usr/share/rlshim/data").join(asset_name),
        PathBuf::from("/usr/local/share/rlshim/data").join(asset_name),
        PathBuf::from("/opt/rlshim/data").join(asset_name),
        PathBuf::from("/app/share/rlshim/data").join(asset_name),
    ];

    for path in global_paths {
        if path.exists() {
            return path;
        }
    }

    PathBuf::from(asset_name)
}

pub(crate) fn configure_fonts_and_style(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    if let Ok(rs_bytes) = fs::read(resolve_asset_path("runescape.ttf")) {
        fonts.font_data.insert(
            "runescape".to_owned(),
            Arc::new(FontData::from_owned(rs_bytes)),
        );
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "runescape".to_owned());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "runescape".to_owned());
    } else {
        log_error!("Failed to load custom font!");
    }

    if let Ok(quill_bytes) = fs::read(resolve_asset_path("runescape_quill.ttf")) {
        fonts.font_data.insert(
            "quill".to_owned(),
            Arc::new(FontData::from_owned(quill_bytes)),
        );
        fonts
            .families
            .insert(FontFamily::Name("quill".into()), vec!["quill".to_owned()]);
    }

    ctx.set_fonts(fonts);

    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = None;
    visuals.widgets.noninteractive.fg_stroke =
        Stroke::new(1.0f32, Color32::from_rgb(128, 128, 128));
    visuals.widgets.noninteractive.corner_radius = CornerRadius::ZERO;

    visuals.widgets.inactive.bg_fill = BTN_BG;
    visuals.widgets.inactive.weak_bg_fill = BTN_BG;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0f32, YELLOW);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0f32, Color32::WHITE);
    visuals.widgets.inactive.corner_radius = CornerRadius::ZERO;

    visuals.widgets.hovered.bg_fill = BTN_HOVER;
    visuals.widgets.hovered.weak_bg_fill = BTN_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0f32, YELLOW);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0f32, Color32::WHITE);
    visuals.widgets.hovered.corner_radius = CornerRadius::ZERO;

    visuals.widgets.active.bg_fill = BTN_HOVER;
    visuals.widgets.active.weak_bg_fill = BTN_HOVER;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0f32, YELLOW);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0f32, Color32::WHITE);
    visuals.widgets.active.corner_radius = CornerRadius::ZERO;

    visuals.widgets.open.bg_fill = BTN_BG;
    visuals.widgets.open.weak_bg_fill = BTN_BG;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0f32, YELLOW);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0f32, Color32::WHITE);
    visuals.widgets.open.corner_radius = CornerRadius::ZERO;

    visuals.window_corner_radius = CornerRadius::ZERO;
    visuals.menu_corner_radius = CornerRadius::ZERO;
    ctx.set_visuals(visuals);

    ctx.style_mut(|style| {
        style.spacing.item_spacing = Vec2::new(8.0, 4.0);
        style.text_styles.insert(
            egui::TextStyle::Button,
            FontId::new(16.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Body,
            FontId::new(16.0, FontFamily::Proportional),
        );
    });
}

pub(crate) fn load_background_texture(ctx: &egui::Context) -> Option<(TextureHandle, Vec2)> {
    let path = resolve_asset_path("background.png");
    let Ok(img) = image::open(&path) else {
        log_error!("Failed to load background.png");
        return None;
    };
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    let tex = ctx.load_texture("background", color_image, TextureOptions::LINEAR);
    Some((tex, Vec2::new(size[0] as f32, size[1] as f32)))
}

fn draw_background_and_border(ctx: &egui::Context, bg: &Option<(TextureHandle, Vec2)>) {
    let screen_rect = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());

    if let Some((tex, bg_size)) = bg
        && bg_size.x > 0.0
        && bg_size.y > 0.0
    {
        let scale_x = screen_rect.width() / bg_size.x;
        let scale_y = screen_rect.height() / bg_size.y;
        let scale = scale_x.max(scale_y);
        let scaled_w = bg_size.x * scale;
        let scaled_h = bg_size.y * scale;
        let min_x = (screen_rect.width() - scaled_w) * 0.5;
        let min_y = (screen_rect.height() - scaled_h) * 0.5;
        let img_rect = Rect::from_min_size(Pos2::new(min_x, min_y), Vec2::new(scaled_w, scaled_h));
        painter.image(
            tex.id(),
            img_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    painter.rect_stroke(
        screen_rect,
        CornerRadius::ZERO,
        Stroke::new(2.0f32, YELLOW),
        StrokeKind::Inside,
    );
}

fn quill_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("quill".into()))
}

fn draw_centered_text(ui: &mut egui::Ui, text: RichText) {
    let widget_text = egui::WidgetText::from(text);
    let galley = widget_text.into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Body,
    );
    let avail_w = ui.available_width();
    let offset_x = ((avail_w - galley.size().x) * 0.5).max(0.0);
    ui.horizontal(|ui| {
        ui.add_space(offset_x);
        ui.label(galley);
    });
}

fn open_url_in_browser(url: &str) {
    let _ = Command::new("xdg-open").arg(url).spawn();
}

pub(crate) struct UrlPromptApp {
    pub(crate) bg: Option<(TextureHandle, Vec2)>,
    pub(crate) instructions: String,
    pub(crate) instructions_pt_2: String,
    pub(crate) url_to_open: String,
    pub(crate) url_placeholder: String,
    pub(crate) input_buf: String,
    pub(crate) opened_at: Option<Instant>,
    pub(crate) copied_at: Option<Instant>,
    pub(crate) result_slot: Arc<Mutex<Option<String>>>,
}

impl UrlPromptApp {
    pub(crate) fn draw(&mut self, ctx: &egui::Context) {
        draw_background_and_border(ctx, &self.bg);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.inner_margin(8.0))
            .show(ctx, |ui| {
                draw_centered_text(
                    ui,
                    RichText::new("rlshim").font(quill_font(64.0)).color(YELLOW),
                );
                ui.add_space(42.0);

                ui.label(RichText::new(&self.instructions).color(YELLOW));
                ui.add_space(8.0);

                let mut readonly_url = self.url_to_open.as_str();
                egui::Frame::NONE
                    .fill(Color32::BLACK)
                    .stroke(Stroke::new(1.0f32, YELLOW))
                    .inner_margin(egui::Margin::symmetric(4, 3))
                    .show(ui, |ui| {
                        ui.add_sized(
                            [ui.available_width(), 120.0],
                            egui::TextEdit::multiline(&mut readonly_url)
                                .text_color(Color32::WHITE)
                                .frame(false),
                        );
                    });

                ui.horizontal(|ui| {
                    let is_opened = self
                        .opened_at
                        .is_some_and(|t| t.elapsed().as_secs_f32() <= 5.0);
                    if is_opened {
                        ui.add_sized([150.0, 40.0], egui::Button::new("opened!"));
                        ctx.request_repaint();
                    } else {
                        self.opened_at = None;
                        if ui
                            .add_sized([150.0, 40.0], egui::Button::new("open in browser"))
                            .clicked()
                        {
                            open_url_in_browser(&self.url_to_open);
                            self.opened_at = Some(Instant::now());
                        }
                    }

                    let is_copied = self
                        .copied_at
                        .is_some_and(|t| t.elapsed().as_secs_f32() <= 5.0);
                    if is_copied {
                        ui.add_sized([150.0, 40.0], egui::Button::new("copied!"));
                        ctx.request_repaint();
                    } else {
                        self.copied_at = None;
                        if ui
                            .add_sized([150.0, 40.0], egui::Button::new("copy url to clipboard"))
                            .clicked()
                        {
                            ctx.copy_text(self.url_to_open.clone());
                            self.copied_at = Some(Instant::now());
                        }
                    }
                });

                ui.add_space(20.0);
                ui.label(RichText::new(&self.instructions_pt_2).color(YELLOW));

                egui::Frame::NONE
                    .fill(Color32::BLACK)
                    .stroke(Stroke::new(1.0f32, YELLOW))
                    .inner_margin(egui::Margin::symmetric(4, 0))
                    .show(ui, |ui| {
                        ui.add_sized(
                            [ui.available_width(), 20.0],
                            egui::TextEdit::singleline(&mut self.input_buf)
                                .hint_text(
                                    RichText::new(&self.url_placeholder)
                                        .color(Color32::from_rgb(128, 128, 128)),
                                )
                                .text_color(Color32::WHITE)
                                .frame(false),
                        );
                    });

                ui.add_space(4.0);
                if ui
                    .add_sized([120.0, 40.0], egui::Button::new("okay here you go!"))
                    .clicked()
                {
                    if !self.input_buf.is_empty() {
                        *self.result_slot.lock().unwrap() = Some(self.input_buf.clone());
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
    }
}

impl eframe::App for UrlPromptApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }
}

pub(crate) struct CharSelectApp {
    pub(crate) bg: Option<(TextureHandle, Vec2)>,
    pub(crate) accounts: Vec<GameAccount>,
    pub(crate) result_slot: Arc<Mutex<Option<GameAccount>>>,
}

impl CharSelectApp {
    pub(crate) fn draw(&mut self, ctx: &egui::Context) {
        draw_background_and_border(ctx, &self.bg);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.inner_margin(8.0))
            .show(ctx, |ui| {
                draw_centered_text(
                    ui,
                    RichText::new("rlshim").font(quill_font(64.0)).color(YELLOW),
                );
                ui.add_space(20.0);
                draw_centered_text(
                    ui,
                    RichText::new("choose a character").size(32.0).color(YELLOW),
                );
                ui.add_space(42.0);

                ui.vertical_centered(|ui| {
                    for acc in &self.accounts {
                        let display_name = if acc.display_name.is_empty() {
                            "[no name set]"
                        } else {
                            &acc.display_name
                        };

                        if ui
                            .add_sized([300.0, 40.0], egui::Button::new(display_name))
                            .clicked()
                        {
                            *self.result_slot.lock().unwrap() = Some(acc.clone());
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        ui.add_space(4.0);
                    }
                });
            });
    }
}

impl eframe::App for CharSelectApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }
}

pub(crate) struct WarningApp {
    pub(crate) bg: Option<(TextureHandle, Vec2)>,
    pub(crate) warning_msg: String,
}

impl WarningApp {
    pub(crate) fn draw(&mut self, ctx: &egui::Context) {
        draw_background_and_border(ctx, &self.bg);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.inner_margin(8.0))
            .show(ctx, |ui| {
                draw_centered_text(ui, RichText::new("rlshim").font(quill_font(64.0)).color(YELLOW));
                ui.add_space(16.0);
                draw_centered_text(ui, RichText::new(&self.warning_msg).color(YELLOW));
                ui.add_space(16.0);
                draw_centered_text(
                    ui,
                    RichText::new(
                        "run rlshim in a terminal for additional information.\nplease fix this issue and restart the application.",
                    )
                    .color(YELLOW),
                );
                ui.add_space(16.0);

                if ui
                    .add_sized([120.0, 40.0], egui::Button::new("okay, i understand"))
                    .clicked()
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
    }
}

impl eframe::App for WarningApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }
}

pub fn prompt_for_url(
    step_name: &str,
    instructions: &str,
    instructions_pt_2: &str,
    url_to_open: &str,
    url_placeholder: &str,
) -> Option<String> {
    let result_slot: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let slot_clone = Arc::clone(&result_slot);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(step_name)
            .with_inner_size([600.0, 600.0])
            .with_resizable(false)
            .with_decorations(false),
        ..Default::default()
    };

    let instructions = instructions.to_string();
    let instructions_pt_2 = instructions_pt_2.to_string();
    let url_to_open = url_to_open.to_string();
    let url_placeholder = url_placeholder.to_string();

    if let Err(e) = eframe::run_native(
        step_name,
        options,
        Box::new(move |cc| {
            configure_fonts_and_style(&cc.egui_ctx);
            let bg = load_background_texture(&cc.egui_ctx);
            Ok(Box::new(UrlPromptApp {
                bg,
                instructions,
                instructions_pt_2,
                url_to_open,
                url_placeholder,
                input_buf: String::new(),
                opened_at: None,
                copied_at: None,
                result_slot: slot_clone,
            }))
        }),
    ) {
        log_error!("Failed to initialize GUI context: {}", e);
        return None;
    }

    let res = result_slot.lock().unwrap().take();
    res.filter(|s| !s.is_empty())
}

pub fn prompt_for_character(accounts: &[GameAccount]) -> Option<GameAccount> {
    if accounts.is_empty() {
        return None;
    }
    if accounts.len() == 1 {
        return Some(accounts[0].clone());
    }

    let result_slot: Arc<Mutex<Option<GameAccount>>> = Arc::new(Mutex::new(None));
    let slot_clone = Arc::clone(&result_slot);
    let accounts_vec = accounts.to_vec();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("rlshim - choose your character")
            .with_inner_size([600.0, 600.0])
            .with_resizable(false)
            .with_decorations(false),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "rlshim - choose your character",
        options,
        Box::new(move |cc| {
            configure_fonts_and_style(&cc.egui_ctx);
            let bg = load_background_texture(&cc.egui_ctx);
            Ok(Box::new(CharSelectApp {
                bg,
                accounts: accounts_vec,
                result_slot: slot_clone,
            }))
        }),
    ) {
        log_error!("failed to initialize GUI context: {}", e);
        return None;
    }

    result_slot.lock().unwrap().take()
}

pub fn warning_prompt(short_title: &str, warning: &str, height: i32) {
    let title = format!("rlshim - {short_title}");
    let warning_msg = format!("warning: {warning}");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(&title)
            .with_inner_size([600.0, height as f32])
            .with_resizable(false)
            .with_decorations(false),
        ..Default::default()
    };

    let _ = eframe::run_native(
        &title,
        options,
        Box::new(move |cc| {
            configure_fonts_and_style(&cc.egui_ctx);
            let bg = load_background_texture(&cc.egui_ctx);
            Ok(Box::new(WarningApp { bg, warning_msg }))
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_visuals_match_cpp_style() {
        let ctx = egui::Context::default();
        configure_fonts_and_style(&ctx);

        let style = ctx.style();
        assert_eq!(
            style.visuals.widgets.inactive.fg_stroke.color,
            Color32::WHITE
        );
        assert_eq!(
            style.visuals.widgets.hovered.fg_stroke.color,
            Color32::WHITE
        );
        assert_eq!(style.visuals.widgets.active.fg_stroke.color, Color32::WHITE);

        assert_eq!(
            style.visuals.widgets.inactive.corner_radius,
            CornerRadius::ZERO
        );
        assert_eq!(
            style.visuals.widgets.hovered.corner_radius,
            CornerRadius::ZERO
        );
        assert_eq!(
            style.visuals.widgets.active.corner_radius,
            CornerRadius::ZERO
        );

        assert_eq!(style.visuals.widgets.inactive.bg_stroke.color, YELLOW);
        assert_eq!(style.visuals.widgets.inactive.bg_fill, BTN_BG);

        let btn_font = style.text_styles.get(&egui::TextStyle::Button).unwrap();
        assert_eq!(btn_font.size, 16.0);
    }
}
