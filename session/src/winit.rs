//! Nested backend: Altru Session in a window of an existing desktop (winit).
//!
//! Drawing is demand-driven. The screen is redrawn only when something changed
//! (a window committed new content, input arrived, the layout changed or the
//! window was exposed), checked at most once per display refresh. An idle session
//! uses almost no CPU, so it stays out of the way of the DAW or the image editor.

use std::{cell::RefCell, rc::Rc, time::Duration};

use smithay::{
    backend::{
        renderer::{
            damage::OutputDamageTracker, element::surface::WaylandSurfaceRenderElement,
            gles::GlesRenderer,
        },
        winit::{self, WinitEvent},
    },
    desktop::layer_map_for_output,
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::{
            EventLoop,
            timer::{TimeoutAction, Timer},
        },
        winit::{dpi::LogicalSize, window::WindowAttributes},
    },
    utils::{Rectangle, Transform},
};

use crate::{AltruSession, CalloopData};

/// How often to check for pending changes (one 60 Hz frame).
const FRAME: Duration = Duration::from_micros(16_667);

pub fn init_winit(
    event_loop: &mut EventLoop<CalloopData>,
    data: &mut CalloopData,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = data;

    let (backend, winit) = winit::init_from_attributes(
        WindowAttributes::default()
            .with_title("Altru Session")
            .with_inner_size(LogicalSize::new(1280.0, 800.0)),
    )?;

    let mode = Mode {
        size: backend.window_size(),
        refresh: 60_000,
    };

    let output = Output::new(
        "winit".to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Altru".into(),
            model: "Session (nested)".into(),
        },
    );
    let _global = output.create_global::<AltruSession>(&state.display_handle);
    output.change_current_state(
        Some(mode),
        Some(Transform::Flipped180),
        None,
        Some((0, 0).into()),
    );
    output.set_preferred(mode);

    state.space.map_output(&output, (0, 0));
    state.needs_redraw = true;

    let backend = Rc::new(RefCell::new(backend));
    let mut damage_tracker = OutputDamageTracker::from_output(&output);

    let events_output = output.clone();
    event_loop
        .handle()
        .insert_source(winit, move |event, _, data| {
            let state = data;
            match event {
                WinitEvent::Resized { size, .. } => {
                    events_output.change_current_state(
                        Some(Mode {
                            size,
                            refresh: 60_000,
                        }),
                        None,
                        None,
                        None,
                    );
                    state.retile();
                }
                WinitEvent::Input(event) => {
                    state.process_input_event(event);
                    state.needs_redraw = true;
                }
                WinitEvent::Redraw => state.needs_redraw = true,
                WinitEvent::CloseRequested => state.loop_signal.stop(),
                _ => (),
            };
        })?;

    event_loop
        .handle()
        .insert_source(Timer::immediate(), move |_, _, data| {
            let state = data;
            if !state.needs_redraw {
                return TimeoutAction::ToDuration(FRAME);
            }
            state.needs_redraw = false;

            let mut backend = backend.borrow_mut();
            let size = backend.window_size();
            let damage = Rectangle::from_size(size);
            {
                let (renderer, mut framebuffer) = backend.bind().unwrap();
                smithay::desktop::space::render_output::<
                    _,
                    WaylandSurfaceRenderElement<GlesRenderer>,
                    _,
                    _,
                >(
                    &output,
                    renderer,
                    &mut framebuffer,
                    1.0,
                    0,
                    [&state.space],
                    &[],
                    &mut damage_tracker,
                    [0.1, 0.1, 0.1, 1.0],
                )
                .unwrap();
            }
            backend.submit(Some(&[damage])).unwrap();

            // Let clients draw their next frame; their commit marks the next redraw.
            state.send_frames(&output);
            state.space.refresh();
            layer_map_for_output(&output).cleanup();
            state.popups.cleanup();
            let _ = state.display_handle.flush_clients();
            TimeoutAction::ToDuration(FRAME)
        })?;

    Ok(())
}
