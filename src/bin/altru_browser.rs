use std::io::Read;
use std::time::Duration;

use adaptive_web_engine_fabric::companion_observation::{
    BrowserCapabilityObservation, append_local_observation,
};
use adaptive_web_engine_fabric::native_runtime::{NativePage, NativeRuntime, NativeRuntimeError};
use adaptive_web_engine_fabric::native_scene::SceneCommand;
use adaptive_web_engine_fabric::resource_api::{
    ResourceBroker, ResourceError, ResourceRequest, ResourceResponse,
};
use eframe::egui::{self, Align, Color32, FontId, Layout, RichText, Stroke, Vec2};

const START_URL: &str = "altru://start";
const MAX_DOCUMENT_BYTES: u64 = 2 * 1024 * 1024;

const START_DOCUMENT: &str = r#"
<html>
<head>
<style>
body { padding-top: 24px; padding-left: 28px; padding-right: 28px; }
h1 { font-size: 42px; margin-bottom: 8px; }
.lead { font-size: 20px; margin-bottom: 18px; }
.row { display: flex; gap: 16px; margin-top: 16px; }
.card { padding-top: 14px; padding-right: 14px; padding-bottom: 14px; padding-left: 14px; }
.small { font-size: 15px; }
</style>
</head>
<body>
<h1>Altru Browser</h1>
<p class="lead">Code for Humanity.</p>
<p>Developer Preview · owned native engine · explicit authority · evidence-gated.</p>
<div class="row">
  <div class="card"><p>Focus</p><p class="small">The page gets the screen.</p></div>
  <div class="card"><p>Navigate</p><p class="small">Controls appear when needed.</p></div>
  <div class="card"><p>Inspect</p><p class="small">Engine state stays visible on demand.</p></div>
</div>
<p class="small">Press Ctrl+L to enter an HTTPS address. Press Esc to return to Focus.</p>
</body>
</html>
"#;

#[derive(Debug)]
struct HttpsDocumentBroker {
    agent: ureq::Agent,
}

impl Default for HttpsDocumentBroker {
    fn default() -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(6))
            .timeout_read(Duration::from_secs(10))
            .timeout_write(Duration::from_secs(5))
            .redirects(0)
            .user_agent("AltruBrowser/0.1-dev (+https://altru.dev/browser)")
            .build();
        Self { agent }
    }
}

impl ResourceBroker for HttpsDocumentBroker {
    fn fetch(&mut self, request: &ResourceRequest) -> Result<ResourceResponse, ResourceError> {
        if request.target == START_URL {
            return Ok(ResourceResponse {
                request_id: request.request_id,
                status: 200,
                media_type: "text/html".into(),
                body: START_DOCUMENT.as_bytes().to_vec(),
            });
        }

        let url = url::Url::parse(&request.target)
            .map_err(|error| ResourceError::Transport(format!("invalid URL: {error}")))?;
        if url.scheme() != "https" || url.host_str().is_none() {
            return Err(ResourceError::Denied);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(ResourceError::Denied);
        }

        let response = match self.agent.get(url.as_str()).call() {
            Ok(response) => response,
            Err(ureq::Error::Status(status, response)) => {
                return Ok(ResourceResponse {
                    request_id: request.request_id,
                    status,
                    media_type: response
                        .header("Content-Type")
                        .unwrap_or("application/octet-stream")
                        .to_string(),
                    body: Vec::new(),
                });
            }
            Err(error) => {
                return Err(ResourceError::Transport(error.to_string()));
            }
        };

        let status = response.status();
        let media_type = response
            .header("Content-Type")
            .unwrap_or("application/octet-stream")
            .to_string();

        if !(media_type.starts_with("text/html")
            || media_type.starts_with("application/xhtml+xml")
            || media_type.starts_with("text/plain"))
        {
            return Err(ResourceError::Transport(format!(
                "top-level media type is not supported: {media_type}"
            )));
        }

        if let Some(content_length) = response.header("Content-Length")
            && let Ok(length) = content_length.parse::<u64>()
            && length > MAX_DOCUMENT_BYTES
        {
            return Err(ResourceError::Transport(format!(
                "document exceeds {} byte preview limit",
                MAX_DOCUMENT_BYTES
            )));
        }

        let mut body = Vec::new();
        response
            .into_reader()
            .take(MAX_DOCUMENT_BYTES + 1)
            .read_to_end(&mut body)
            .map_err(|error| ResourceError::Transport(error.to_string()))?;
        if body.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err(ResourceError::Transport(format!(
                "document exceeds {} byte preview limit",
                MAX_DOCUMENT_BYTES
            )));
        }

        Ok(ResourceResponse {
            request_id: request.request_id,
            status,
            media_type,
            body,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChromeMode {
    Focus,
    Navigate,
    Inspect,
}

struct AltruBrowserApp {
    runtime: NativeRuntime<HttpsDocumentBroker>,
    page: Option<NativePage>,
    last_error: Option<String>,
    url_input: String,
    mode: ChromeMode,
    request_url_focus: bool,
    dark: bool,
}

impl AltruBrowserApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_style(&cc.egui_ctx, false);

        let mut runtime = NativeRuntime::new(HttpsDocumentBroker::default());
        let (page, last_error) = match runtime.load(START_URL) {
            Ok(page) => (Some(page), None),
            Err(error) => (None, Some(format_runtime_error(&error))),
        };

        Self {
            runtime,
            page,
            last_error,
            url_input: START_URL.into(),
            mode: ChromeMode::Navigate,
            request_url_focus: false,
            dark: false,
        }
    }

    fn navigate(&mut self, target: String) {
        let normalized = normalize_target(&target);
        self.url_input = normalized.clone();
        match self.runtime.load(normalized.clone()) {
            Ok(page) => {
                let _ = append_local_observation(&BrowserCapabilityObservation::success(
                    normalized.clone(),
                ));
                self.page = Some(page);
                self.last_error = None;
                self.mode = ChromeMode::Focus;
            }
            Err(error) => {
                let _ = append_local_observation(&BrowserCapabilityObservation::failure(
                    normalized.clone(),
                    &error,
                ));
                self.page = None;
                self.last_error = Some(format_runtime_error(&error));
                self.mode = ChromeMode::Inspect;
            }
        }
    }

    fn back(&mut self) {
        match self.runtime.back() {
            Ok(Some(page)) => {
                self.url_input = page.target.clone();
                self.page = Some(page);
                self.last_error = None;
            }
            Ok(None) => {}
            Err(error) => self.last_error = Some(format_runtime_error(&error)),
        }
    }

    fn forward(&mut self) {
        match self.runtime.forward() {
            Ok(Some(page)) => {
                self.url_input = page.target.clone();
                self.page = Some(page);
                self.last_error = None;
            }
            Ok(None) => {}
            Err(error) => self.last_error = Some(format_runtime_error(&error)),
        }
    }

    fn reload(&mut self) {
        match self.runtime.reload() {
            Ok(Some(page)) => {
                self.url_input = page.target.clone();
                self.page = Some(page);
                self.last_error = None;
            }
            Ok(None) => {}
            Err(error) => self.last_error = Some(format_runtime_error(&error)),
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input(|input| input.modifiers.ctrl && input.key_pressed(egui::Key::L)) {
            self.mode = ChromeMode::Navigate;
            self.request_url_focus = true;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.mode = ChromeMode::Focus;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::F9)) {
            self.mode = if self.mode == ChromeMode::Inspect {
                ChromeMode::Focus
            } else {
                ChromeMode::Inspect
            };
        }
    }

    fn navigation_bar(&mut self, ctx: &egui::Context) {
        let panel = egui::TopBottomPanel::top("altru_navigation")
            .exact_height(56.0)
            .frame(
                egui::Frame::new()
                    .fill(if self.dark {
                        Color32::from_rgb(11, 25, 39)
                    } else {
                        Color32::from_rgb(249, 251, 253)
                    })
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .stroke(Stroke::new(
                        1.0_f32,
                        if self.dark {
                            Color32::from_rgba_unmultiplied(101, 199, 255, 35)
                        } else {
                            Color32::from_rgba_unmultiplied(23, 63, 91, 22)
                        },
                    )),
            );

        panel.show(ctx, |ui| {
            draw_embossed_motif(ui);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                if ui
                    .button(RichText::new("‹").size(24.0))
                    .on_hover_text("Back")
                    .clicked()
                {
                    self.back();
                }
                if ui
                    .button(RichText::new("›").size(24.0))
                    .on_hover_text("Forward")
                    .clicked()
                {
                    self.forward();
                }
                if ui.button("↻").on_hover_text("Reload").clicked() {
                    self.reload();
                }

                let available = (ui.available_width() - 250.0).max(220.0);
                let response = ui.add_sized(
                    [available, 38.0],
                    egui::TextEdit::singleline(&mut self.url_input)
                        .hint_text("Search or enter an HTTPS address")
                        .desired_width(f32::INFINITY),
                );
                if self.request_url_focus {
                    response.request_focus();
                    select_all_text(ctx, response.id, &self.url_input);
                    self.request_url_focus = false;
                }
                if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                    self.navigate(self.url_input.clone());
                }

                if ui.button("Go").clicked() {
                    self.navigate(self.url_input.clone());
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .selectable_label(self.mode == ChromeMode::Inspect, "Inspect")
                        .clicked()
                    {
                        self.mode = ChromeMode::Inspect;
                    }
                    if ui.button(if self.dark { "☀" } else { "◐" }).clicked() {
                        self.dark = !self.dark;
                        configure_style(ctx, self.dark);
                    }
                    if ui.button("Focus").clicked() {
                        self.mode = ChromeMode::Focus;
                    }
                });
            });
        });
    }

    fn focus_reveal(&mut self, ctx: &egui::Context) {
        egui::Area::new(egui::Id::new("altru_focus_reveal"))
            .anchor(egui::Align2::LEFT_TOP, Vec2::new(12.0, 12.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(if self.dark {
                        Color32::from_rgba_unmultiplied(9, 24, 38, 220)
                    } else {
                        Color32::from_rgba_unmultiplied(255, 255, 255, 230)
                    })
                    .corner_radius(16.0)
                    .stroke(Stroke::new(
                        1.0_f32,
                        Color32::from_rgba_unmultiplied(101, 199, 255, 60),
                    ))
                    .inner_margin(egui::Margin::symmetric(10, 7))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .button(RichText::new("✦").color(Color32::from_rgb(40, 112, 226)))
                                .on_hover_text("Show navigation")
                                .clicked()
                            {
                                self.mode = ChromeMode::Navigate;
                                self.request_url_focus = true;
                            }
                            if ui.small_button("Inspect").on_hover_text("F9").clicked() {
                                self.mode = ChromeMode::Inspect;
                            }
                        });
                    });
            });
    }

    fn render_page(&self, ui: &mut egui::Ui) {
        if let Some(error) = &self.last_error {
            render_error(ui, error);
            return;
        }

        let Some(page) = &self.page else {
            render_error(ui, "No document is loaded.");
            return;
        };

        let scene = &page.execution.scene;
        let viewport_width = ui.available_width().max(320.0);
        let scale = (viewport_width / 840.0).clamp(0.55, 1.45);
        let scene_height = scene
            .commands
            .iter()
            .map(|command| match command {
                SceneCommand::Text { y, size_px, .. } => y + size_px * 1.8,
                SceneCommand::Rect { y, height, .. } => y + height,
            })
            .fold(620.0_f32, f32::max);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let desired = Vec2::new(viewport_width, scene_height * scale + 40.0);
                let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
                let painter = ui.painter_at(rect);
                let origin = rect.min + Vec2::new(18.0, 18.0);

                for command in &scene.commands {
                    match command {
                        SceneCommand::Text {
                            x,
                            y,
                            size_px,
                            text,
                        } => {
                            painter.text(
                                origin + Vec2::new(x * scale, y * scale),
                                egui::Align2::LEFT_TOP,
                                text,
                                FontId::proportional((size_px * scale).clamp(10.0, 64.0)),
                                if self.dark {
                                    Color32::from_rgb(236, 244, 250)
                                } else {
                                    Color32::from_rgb(20, 36, 50)
                                },
                            );
                        }
                        SceneCommand::Rect {
                            x,
                            y,
                            width,
                            height,
                        } => {
                            painter.rect_stroke(
                                egui::Rect::from_min_size(
                                    origin + Vec2::new(x * scale, y * scale),
                                    Vec2::new(width * scale, height * scale),
                                ),
                                6.0,
                                Stroke::new(
                                    1.0_f32,
                                    Color32::from_rgba_unmultiplied(101, 199, 255, 45),
                                ),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                }
            });
    }

    fn inspector(&self, ctx: &egui::Context) {
        let mut open = true;
        egui::Window::new("Page Intelligence")
            .open(&mut open)
            .anchor(egui::Align2::RIGHT_TOP, Vec2::new(-14.0, 68.0))
            .resizable(false)
            .collapsible(false)
            .default_width(330.0)
            .frame(
                egui::Frame::window(&ctx.style())
                    .corner_radius(22.0)
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("ALTRU · NATIVE PREVIEW")
                        .small()
                        .strong()
                        .color(Color32::from_rgb(38, 116, 189)),
                );
                ui.add_space(8.0);

                if let Some(page) = &self.page {
                    metric(ui, "Target", &page.target);
                    metric(ui, "Engine", "Altru native N2.1");
                    metric(
                        ui,
                        "Scene",
                        &format!("{} commands", page.execution.scene.commands.len()),
                    );
                    metric(ui, "Network", "Top-level HTTPS only");
                    metric(ui, "Scripts", "Disabled");
                    metric(ui, "Telemetry", "None");
                    ui.separator();
                    ui.label(RichText::new("Evidence").strong());
                    ui.monospace(short_hash(&page.execution.evidence.execution_sha256));
                    ui.label(
                        RichText::new("Experimental · not production promoted")
                            .small()
                            .color(Color32::from_rgb(160, 110, 20)),
                    );
                } else {
                    ui.label("No successful native document execution.");
                }

                if let Some(error) = &self.last_error {
                    ui.separator();
                    ui.label(RichText::new("Blocked").strong().color(Color32::DARK_RED));
                    ui.label(error);
                }

                ui.separator();
                ui.label(
                    RichText::new("F9 toggles Inspect · Esc returns to Focus")
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            });
    }
}

impl eframe::App for AltruBrowserApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.shortcuts(ctx);

        if self.mode == ChromeMode::Navigate || self.mode == ChromeMode::Inspect {
            self.navigation_bar(ctx);
        } else {
            self.focus_reveal(ctx);
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(if self.dark {
                        Color32::from_rgb(7, 18, 29)
                    } else {
                        Color32::from_rgb(252, 253, 254)
                    })
                    .inner_margin(egui::Margin::same(0)),
            )
            .show(ctx, |ui| {
                self.render_page(ui);
            });

        if self.mode == ChromeMode::Inspect {
            self.inspector(ctx);
        }
    }
}

fn configure_style(ctx: &egui::Context, dark: bool) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(12);
    style.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(12);
    style.visuals.widgets.active.corner_radius = egui::CornerRadius::same(12);
    style.visuals.selection.bg_fill = Color32::from_rgb(65, 139, 232);
    ctx.set_style(style);
}

fn select_all_text(ctx: &egui::Context, widget_id: egui::Id, value: &str) {
    let mut state = egui::TextEdit::load_state(ctx, widget_id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::default(),
            egui::text::CCursor::new(value.chars().count()),
        )));
    state.store(ctx, widget_id);
}

fn normalize_target(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return START_URL.into();
    }
    if trimmed == "altru" || trimmed == "start" {
        return START_URL.into();
    }
    if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

fn format_runtime_error(error: &NativeRuntimeError) -> String {
    format!("{error:?}")
}

fn render_error(ui: &mut egui::Ui, error: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(100.0);
        ui.heading("Altru stopped this page.");
        ui.add_space(8.0);
        ui.label(
            "The current native engine will not silently approximate unsupported web behavior.",
        );
        ui.add_space(12.0);
        ui.label(RichText::new(error).monospace());
        ui.add_space(18.0);
        ui.label("Press Ctrl+L to try another HTTPS page, or enter altru://start.");
    });
}

fn metric(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(label).strong());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(value);
        });
    });
}

fn short_hash(hash: &str) -> String {
    if hash.len() <= 20 {
        hash.into()
    } else {
        format!("{}…{}", &hash[..12], &hash[hash.len() - 8..])
    }
}

fn draw_embossed_motif(ui: &egui::Ui) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    let y = rect.top() + 2.0;
    let base_x = rect.center().x - 42.0;
    let blue = Color32::from_rgba_unmultiplied(55, 132, 214, 22);
    let wheat = Color32::from_rgba_unmultiplied(230, 177, 44, 20);

    for index in 0..5 {
        let x = base_x + index as f32 * 21.0;
        let color = if index % 2 == 0 { blue } else { wheat };
        painter.line_segment(
            [egui::pos2(x, y + 4.0), egui::pos2(x + 8.0, y + 10.0)],
            Stroke::new(1.0_f32, color),
        );
        painter.line_segment(
            [egui::pos2(x + 8.0, y + 10.0), egui::pos2(x + 16.0, y + 4.0)],
            Stroke::new(1.0_f32, color),
        );
    }
}

fn icon_data() -> egui::IconData {
    let image =
        image::load_from_memory(include_bytes!("../../assets/brand/altru-browser-mark.png"))
            .expect("embedded Altru Browser icon must decode")
            .into_rgba8();
    let (width, height) = image.dimensions();
    egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adaptive_web_engine_fabric::resource_api::ResourceKind;

    #[test]
    fn preview_broker_serves_only_explicit_builtin_page_without_network() {
        let mut broker = HttpsDocumentBroker::default();
        let response = broker
            .fetch(&ResourceRequest {
                request_id: 7,
                target: START_URL.into(),
                kind: ResourceKind::Document,
            })
            .unwrap();
        assert_eq!(response.request_id, 7);
        assert_eq!(response.status, 200);
        assert!(
            String::from_utf8(response.body)
                .unwrap()
                .contains("Code for Humanity")
        );
    }

    #[test]
    fn preview_broker_denies_plain_http_and_embedded_credentials() {
        let mut broker = HttpsDocumentBroker::default();
        for target in ["http://example.com", "https://user:pass@example.com"] {
            assert_eq!(
                broker.fetch(&ResourceRequest {
                    request_id: 1,
                    target: target.into(),
                    kind: ResourceKind::Document,
                }),
                Err(ResourceError::Denied)
            );
        }
    }

    #[test]
    fn built_in_start_page_executes_on_native_engine() {
        let mut runtime = NativeRuntime::new(HttpsDocumentBroker::default());
        let page = runtime.load(START_URL).unwrap();
        assert!(!page.execution.scene.is_empty());
        assert!(page.execution.native_semantics);
        assert!(!page.execution.production_promoted);
    }

    #[test]
    fn url_input_defaults_to_https() {
        assert_eq!(normalize_target("example.com"), "https://example.com");
        assert_eq!(normalize_target("start"), START_URL);
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Altru Browser — Developer Preview")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([720.0, 480.0])
            .with_icon(icon_data()),
        ..Default::default()
    };

    eframe::run_native(
        "Altru Browser",
        options,
        Box::new(|cc| Ok(Box::new(AltruBrowserApp::new(cc)))),
    )
}
