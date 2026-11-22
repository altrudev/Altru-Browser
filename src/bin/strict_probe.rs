//! PWX Strict probe: replay a recorded PWX Strict app through Altru's own engine.
//!
//!     cargo run --features taffy-layout --bin strict_probe -- \
//!         fixtures/pwx-strict/notes-seeded.strict fixtures/pwx-strict/ui.css --ppm notes.ppm
//!
//! Options:
//!   --dark            resolve ui.css for a dark host
//!   --ppm <file>      draw with the Altru Metal renderer into a picture
//!   --screen          draw full-screen on the Linux framebuffer (Altru Metal), Ctrl+Q quits
//!   --html <file>     write the same tree as HTML, for comparison with Chromium/Firefox/WebKit
//!   --width <px>      layout width (default 1600)
//!   --font <file.ttf> measure text with this font face, so buttons and labels get
//!                     their real width (without it, auto-sized items share rows)

use adaptive_web_engine_fabric::native_layout::FontFaceMeasurer;
use adaptive_web_engine_fabric::pwx_strict::{
    ColorScheme, StrictBridge, native_ui_stylesheet, parse_stream,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(stream_path), Some(css_path)) = (args.get(1), args.get(2)) else {
        eprintln!(
            "usage: strict_probe <app.strict> <ui.css> [--dark] [--ppm f] [--screen] [--html f] [--width px]"
        );
        std::process::exit(2);
    };
    let option = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
    };
    let scheme = if args.iter().any(|a| a == "--dark") {
        ColorScheme::Dark
    } else {
        ColorScheme::Light
    };
    let width: u32 = option("--width")
        .and_then(|w| w.parse().ok())
        .unwrap_or(1600);
    let fail = |what: &str, error: &dyn std::fmt::Debug| -> ! {
        eprintln!("strict_probe_error={what}: {error:?}");
        std::process::exit(1);
    };

    let stream = std::fs::read_to_string(stream_path).unwrap_or_else(|e| fail("read stream", &e));
    let ui_css = std::fs::read_to_string(css_path).unwrap_or_else(|e| fail("read css", &e));
    let lines = parse_stream(&stream).unwrap_or_else(|e| fail("parse stream", &e));

    let mut bridge = StrictBridge::new();
    let events = bridge.replay(&lines);
    println!("lines={}", lines.len());
    println!("app_nodes={}", bridge.node_count());
    println!("events_delivered={}", events.len());
    println!("rejected={}", bridge.rejected().len());
    for rejection in bridge.rejected() {
        println!("rejected_op={}", rejection.0);
    }

    if let Some(path) = option("--html") {
        std::fs::write(path, bridge.to_html(&ui_css)).unwrap_or_else(|e| fail("write html", &e));
        println!("html={path}");
    }

    let wants_pixels = option("--ppm").is_some() || args.iter().any(|a| a == "--screen");
    if !wants_pixels {
        return;
    }
    let (sheet, adapted) =
        native_ui_stylesheet(&ui_css, scheme).unwrap_or_else(|e| fail("ui.css", &e));
    for note in &adapted {
        println!("css_adapted={note}");
    }
    let font_bytes =
        option("--font").map(|path| std::fs::read(path).unwrap_or_else(|e| fail("read font", &e)));
    let measurer = font_bytes
        .as_deref()
        .map(|font_bytes| FontFaceMeasurer { font_bytes });
    println!(
        "text_measurement={}",
        if measurer.is_some() { "font" } else { "none" }
    );
    let snapshot = bridge.snapshot();
    let scene = snapshot
        .scene_measured(
            &sheet,
            width as f32,
            measurer
                .as_ref()
                .map(|m| m as &dyn adaptive_web_engine_fabric::native_layout::TextMeasurer),
        )
        .unwrap_or_else(|e| {
            eprintln!("hint: ui.css uses flexbox; build with --features taffy-layout");
            fail("layout", &e)
        });
    println!("scene_commands={}", scene.commands.len());
    draw(&scene, width, &args, option("--ppm"));
}

#[cfg(target_os = "linux")]
fn draw(
    scene: &adaptive_web_engine_fabric::native_scene::Scene,
    width: u32,
    args: &[String],
    ppm: Option<&String>,
) {
    use adaptive_web_engine_fabric::native_scene::SceneRenderer;
    use adaptive_web_engine_fabric::platform_metal::{
        KeyboardReader, LinuxFramebuffer, MetalSceneRenderer, ShellIntent,
    };
    let renderer = MetalSceneRenderer::default();
    if let Some(path) = ppm {
        let frame = renderer.render(scene, width, width * 9 / 16);
        std::fs::write(path, frame.to_ppm()).expect("write ppm");
        println!("ppm={path}");
    }
    if args.iter().any(|a| a == "--screen") {
        let fb = LinuxFramebuffer::open_default().expect("open /dev/fb0");
        let g = fb.geometry();
        fb.present(&renderer.render(scene, g.width, g.height))
            .expect("present");
        if let Ok(mut keyboard) = KeyboardReader::open_first() {
            while !matches!(keyboard.next_intent(), Ok(ShellIntent::Quit) | Err(_)) {}
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn draw(
    _scene: &adaptive_web_engine_fabric::native_scene::Scene,
    _width: u32,
    _args: &[String],
    _ppm: Option<&String>,
) {
    eprintln!("drawing uses Altru Metal, which is Linux-only");
}
