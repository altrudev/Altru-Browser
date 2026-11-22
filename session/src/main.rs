//! Altru Session: the display layer of the Altru platform.
//!
//! A Wayland compositor that runs native tools (Ardour, Krita, darktable, VS Code,
//! terminals) at full speed, laid out automatically, and is where Altru Browser
//! apps will join them. Built on Smithay; the compositor core started from
//! Smithay's `smallvil` example (MIT, see LICENSE-SMITHAY).
//!
//!     altru-session --profile studio
//!     altru-session --profile art --run "foot"
//!
//! This first version runs inside an existing desktop (winit backend). Running
//! directly on the hardware (DRM/KMS, libinput) is the next backend.

#![allow(irrefutable_let_patterns)]

mod audio;
mod grabs;
mod handlers;
mod input;
mod layout;
mod profile;
mod shortcuts;
mod state;
mod winit;

use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
pub use state::AltruSession;

/// The event loop's data is the session state itself, so Smithay handlers that
/// need the loop handle (XWayland's window manager) can use it directly.
pub type CalloopData = AltruSession;

fn usage() {
    eprintln!(
        "usage: altru-session [--profile studio|art|dev]... [--run \"command args\"]... [--latency low|default|FRAMES[@RATE]]\n\
         \n  studio  Ardour, qpwgraph, terminal\n  art     Krita, darktable, GIMP\n  dev     VS Code, terminal"
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().init();
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        usage();
        return Ok(());
    }
    let plan = profile::parse_args(&args);
    if !plan.errors.is_empty() {
        for error in &plan.errors {
            eprintln!("altru-session: {error}");
        }
        usage();
        std::process::exit(2);
    }

    let mut event_loop: EventLoop<'static, CalloopData> = EventLoop::try_new()?;
    let display: Display<AltruSession> = Display::new()?;
    let mut data = AltruSession::new(&mut event_loop, display);

    crate::winit::init_winit(&mut event_loop, &mut data)?;
    println!("wayland_display={}", data.socket_name.to_string_lossy());

    let audio_mode = plan.audio();
    if let audio::AudioMode::Fixed(clock) = audio_mode {
        println!("{}", audio::apply(Some(clock)));
        data.audio_latency = Some(clock.env_value());
    }

    data.pending_launch = if plan.apps.is_empty() {
        profile::parse_args(&["--run".into(), "foot".into()]).apps
    } else {
        plan.apps
    };
    // Apps start once XWayland is ready, so X11-only programs find a display.
    data.start_xwayland(&mut event_loop);

    // Send every reply to clients after each loop iteration. Redraws are
    // demand-driven, so replies must never wait for the next frame.
    let result = event_loop.run(None, &mut data, move |data| {
        let _ = data.display_handle.flush_clients();
    });
    if let audio::AudioMode::Fixed(_) = audio_mode {
        println!("{}", audio::apply(None));
    }
    result?;
    Ok(())
}
