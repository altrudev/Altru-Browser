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

mod grabs;
mod handlers;
mod input;
mod layout;
mod profile;
mod shortcuts;
mod state;
mod winit;

use smithay::reexports::{
    calloop::EventLoop,
    wayland_server::{Display, DisplayHandle},
};
pub use state::AltruSession;

pub struct CalloopData {
    state: AltruSession,
    display_handle: DisplayHandle,
}

fn usage() {
    eprintln!(
        "usage: altru-session [--profile studio|art|dev]... [--run \"command args\"]...\n\
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

    let mut event_loop: EventLoop<CalloopData> = EventLoop::try_new()?;
    let display: Display<AltruSession> = Display::new()?;
    let display_handle = display.handle();
    let state = AltruSession::new(&mut event_loop, display);
    let mut data = CalloopData {
        state,
        display_handle,
    };

    crate::winit::init_winit(&mut event_loop, &mut data)?;
    println!(
        "wayland_display={}",
        data.state.socket_name.to_string_lossy()
    );

    let path_var = std::env::var("PATH").ok();
    let apps = if plan.apps.is_empty() {
        profile::parse_args(&["--run".into(), "foot".into()]).apps
    } else {
        plan.apps
    };
    for app in apps {
        let program = &app.argv[0];
        if !profile::is_installed(program, path_var.as_deref()) {
            if app.package.is_empty() {
                eprintln!("missing: {program}");
            } else {
                eprintln!(
                    "missing: {} (install: sudo apt install {})",
                    app.label, app.package
                );
            }
            continue;
        }
        match data.state.launch(&app.argv) {
            Ok(pid) => println!("started: {} (pid {pid})", app.argv.join(" ")),
            Err(error) => eprintln!("failed: {}: {error}", app.argv.join(" ")),
        }
    }

    // Send every reply to clients after each loop iteration. Redraws are
    // demand-driven, so replies must never wait for the next frame.
    event_loop.run(None, &mut data, move |data| {
        let _ = data.display_handle.flush_clients();
    })?;
    Ok(())
}
