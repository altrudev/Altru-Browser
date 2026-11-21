//! Altru Metal probe: render one HTML file full-screen with no desktop.
//!
//! On the target machine (text console, no X11/Wayland running):
//!     sudo ./target/release/metal_probe fixtures/native-n1/headings.html
//! Anywhere else (writes a picture instead of touching the screen):
//!     ./target/release/metal_probe fixtures/native-n1/headings.html --ppm out.ppm
//!
//! Keys on the screen: F6 / Ctrl+L = Navigate, F12 = Inspect, Esc = Focus,
//! Ctrl+Q = quit.

#[cfg(target_os = "linux")]
fn main() {
    use adaptive_web_engine_fabric::native_engine::execute_native_document;
    use adaptive_web_engine_fabric::native_scene::SceneRenderer;
    use adaptive_web_engine_fabric::platform::SurfaceSize;
    use adaptive_web_engine_fabric::platform_metal::{
        COLOR_BAR, KeyboardReader, LinuxFramebuffer, METAL_HOST_IDENTITY, MetalHost,
        MetalSceneRenderer, PixelFrame, ShellIntent,
    };

    let args: Vec<String> = std::env::args().collect();
    let Some(input_path) = args.get(1) else {
        eprintln!("usage: metal_probe <file.html> [--ppm out.ppm]");
        std::process::exit(2);
    };
    let ppm_path = args
        .iter()
        .position(|a| a == "--ppm")
        .and_then(|i| args.get(i + 1));

    let input = std::fs::read_to_string(input_path).unwrap_or_else(|error| {
        eprintln!("metal_probe_error=read {input_path}: {error}");
        std::process::exit(1);
    });
    let execution = execute_native_document(&input).unwrap_or_else(|error| {
        eprintln!("metal_probe_error={error:?}");
        std::process::exit(1);
    });

    let renderer = MetalSceneRenderer::default();
    println!("host={METAL_HOST_IDENTITY}");
    println!("renderer={}", renderer.name());
    println!("execution_sha256={}", execution.evidence.execution_sha256);
    println!("scene_commands={}", execution.scene.commands.len());

    if let Some(ppm_path) = ppm_path {
        let frame = renderer.render(&execution.scene, 1600, 900);
        std::fs::write(ppm_path, frame.to_ppm()).expect("write ppm");
        println!("output={ppm_path}");
        return;
    }

    let framebuffer = LinuxFramebuffer::open_default().unwrap_or_else(|error| {
        eprintln!("metal_probe_error=framebuffer {error:?}");
        std::process::exit(1);
    });
    let geometry = framebuffer.geometry();
    println!(
        "framebuffer={}x{} stride={}",
        geometry.width, geometry.height, geometry.stride_bytes
    );
    let host = MetalHost::new(
        SurfaceSize {
            width: geometry.width,
            height: geometry.height,
        },
        "/var/lib/altru",
    );
    println!("host_identity={}", host.identity());

    let draw = |intent: ShellIntent| {
        let mut frame: PixelFrame =
            renderer.render(&execution.scene, geometry.width, geometry.height);
        let w = geometry.width as f32;
        let h = geometry.height as f32;
        match intent {
            // Navigate: thin top bar (address/search goes here next).
            ShellIntent::Navigate => frame.fill_rect(0.0, 0.0, w, 36.0, COLOR_BAR),
            // Inspect: temporary panel on the right (engine + evidence details).
            ShellIntent::Inspect => frame.fill_rect(w * 0.7, 0.0, w * 0.3, h, COLOR_BAR),
            ShellIntent::Focus | ShellIntent::Quit => {}
        }
        framebuffer.present(&frame)
    };

    draw(ShellIntent::Focus).expect("present");
    let mut keyboard = match KeyboardReader::open_first() {
        Ok(keyboard) => keyboard,
        Err(error) => {
            eprintln!("metal_probe_warning=no keyboard ({error:?}); showing for 10s");
            std::thread::sleep(std::time::Duration::from_secs(10));
            return;
        }
    };
    loop {
        match keyboard.next_intent() {
            Ok(ShellIntent::Quit) => break,
            Ok(intent) => {
                println!("intent={intent:?}");
                draw(intent).expect("present");
            }
            Err(error) => {
                eprintln!("metal_probe_error=input {error:?}");
                break;
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("metal_probe is Linux-only (Altru Metal uses Linux as its driver substrate)");
    std::process::exit(2);
}
