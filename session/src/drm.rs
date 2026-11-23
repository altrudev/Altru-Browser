//! Hardware backend: Altru Session directly on the machine, with no desktop under it.
//!
//! - **Session:** libseat gives access to the GPU and input devices without running
//!   as root, and handles switching to other virtual terminals (Ctrl+Alt+F1…F12).
//! - **Display:** the primary GPU through DRM/KMS. Every connected screen becomes an
//!   output, placed left to right, at its preferred mode. Frames are composited
//!   with OpenGL ES on GBM buffers and flipped on vblank; windows that can be
//!   scanned out directly skip composition (DrmCompositor planes).
//! - **Input:** libinput (keyboards, mice, touchpads, tablets).
//! - **GPU apps:** linux-dmabuf, so Krita, darktable, browsers and games render on
//!   the GPU instead of falling back to software.
//!
//! Single GPU for now. Hybrid-graphics machines use the primary (boot) GPU.

use std::{collections::HashMap, path::PathBuf, time::Duration};

use smithay::{
    backend::{
        allocator::{
            Fourcc,
            dmabuf::Dmabuf,
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        },
        drm::{
            DrmDevice, DrmDeviceFd, DrmEvent, DrmNode,
            compositor::FrameFlags,
            exporter::gbm::GbmFramebufferExporter,
            output::{DrmOutput, DrmOutputManager, DrmOutputRenderElements},
        },
        egl::{EGLContext, EGLDevice, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            ImportDma, ImportMemWl,
            element::{
                Kind,
                solid::{SolidColorBuffer, SolidColorRenderElement},
                surface::{WaylandSurfaceRenderElement, render_elements_from_surface_tree},
            },
            gles::GlesRenderer,
        },
        session::{Event as SessionEvent, Session, libseat::LibSeatSession},
        udev::{all_gpus, primary_gpu},
    },
    delegate_dmabuf,
    desktop::{
        layer_map_for_output,
        space::{SpaceRenderElements, space_render_elements},
    },
    input::pointer::{CursorImageStatus, CursorImageSurfaceData},
    output::{Mode as WlMode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::{
            EventLoop,
            timer::{TimeoutAction, Timer},
        },
        drm::control::{Device as ControlDevice, ModeTypeFlags, connector, crtc},
        input::Libinput,
        rustix::fs::OFlags,
    },
    utils::{DeviceFd, Physical, Point, Scale},
    wayland::{
        compositor::with_states,
        dmabuf::{DmabufFeedbackBuilder, DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier},
    },
};

use crate::{AltruSession, CalloopData};

type Allocator = GbmAllocator<DrmDeviceFd>;
type Exporter = GbmFramebufferExporter<DrmDeviceFd>;
type OutputManager = DrmOutputManager<Allocator, Exporter, (), DrmDeviceFd>;
type HwOutput = DrmOutput<Allocator, Exporter, (), DrmDeviceFd>;

const CLEAR: [f32; 4] = [0.1, 0.1, 0.1, 1.0];
const FRAME: Duration = Duration::from_micros(16_667);

smithay::backend::renderer::element::render_elements! {
    pub SessionElement<=GlesRenderer>;
    Space=SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>,
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    Solid=SolidColorRenderElement,
}

pub struct Screen {
    pub output: Output,
    hw: HwOutput,
    /// A frame is queued and waiting for vblank.
    pending: bool,
}

pub struct DrmBackend {
    pub session: LibSeatSession,
    pub renderer: GlesRenderer,
    manager: OutputManager,
    screens: HashMap<crtc::Handle, Screen>,
    libinput: Libinput,
    cursor_outline: SolidColorBuffer,
    cursor_fill: SolidColorBuffer,
}

fn err(context: &str, error: impl std::fmt::Display) -> Box<dyn std::error::Error> {
    format!("{context}: {error}").into()
}

/// Start the hardware backend. Fails with a readable reason when there is no
/// seat (for example when run inside another desktop) or no usable GPU.
pub fn init_drm(
    event_loop: &mut EventLoop<'static, CalloopData>,
    state: &mut AltruSession,
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut session, notifier) = LibSeatSession::new()
        .map_err(|e| err("no seat (run from a text console, not inside a desktop)", e))?;
    let seat = session.seat();

    let gpu_path: PathBuf = primary_gpu(&seat)
        .ok()
        .flatten()
        .or_else(|| all_gpus(&seat).ok()?.into_iter().next())
        .ok_or("no GPU found")?;
    println!("drm: GPU {}", gpu_path.display());

    let fd = session
        .open(
            &gpu_path,
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
        )
        .map_err(|e| {
            err(
                "open GPU (start Altru Session from a text console on this machine, e.g. Ctrl+Alt+F3, not over SSH)",
                e,
            )
        })?;
    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (drm, drm_events) = DrmDevice::new(fd.clone(), true).map_err(|e| err("DRM device", e))?;
    let gbm = GbmDevice::new(fd.clone()).map_err(|e| err("GBM device", e))?;

    // SAFETY: the GBM device outlives the display: both live in this backend.
    let egl_display = unsafe { EGLDisplay::new(gbm.clone()) }.map_err(|e| err("EGL display", e))?;
    let render_node = EGLDevice::device_for_display(&egl_display)
        .ok()
        .and_then(|device| device.try_get_render_node().ok().flatten());
    let node = DrmNode::from_path(&gpu_path).map_err(|e| err("DRM node", e))?;
    let context = EGLContext::new(&egl_display).map_err(|e| err("EGL context", e))?;
    // SAFETY: the context is current only on this thread.
    let mut renderer =
        unsafe { GlesRenderer::new(context) }.map_err(|e| err("OpenGL ES renderer", e))?;

    state.shm_state.update_formats(renderer.shm_formats());

    // GPU buffers from apps (linux-dmabuf).
    let feedback = DmabufFeedbackBuilder::new(
        render_node.unwrap_or(node).dev_id(),
        renderer.dmabuf_formats(),
    )
    .build()
    .map_err(|e| err("dmabuf feedback", e))?;
    let global = state
        .dmabuf_state
        .create_global_with_default_feedback::<AltruSession>(&state.display_handle, &feedback);
    state.dmabuf_global = Some(global);

    let render_formats = renderer.egl_context().dmabuf_render_formats().clone();
    let mut manager = OutputManager::new(
        drm,
        GbmAllocator::new(
            gbm.clone(),
            GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
        ),
        GbmFramebufferExporter::new(gbm.clone(), render_node),
        Some(gbm),
        [Fourcc::Argb8888, Fourcc::Xrgb8888, Fourcc::Abgr8888],
        render_formats.iter().copied(),
    );

    let screens = connect_screens(state, &mut manager, &mut renderer)?;
    if screens.is_empty() {
        return Err("no connected screen".into());
    }

    // Input.
    let mut libinput =
        Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(session.clone().into());
    libinput
        .udev_assign_seat(&seat)
        .map_err(|()| err("libinput", "could not assign seat"))?;
    let input = LibinputInputBackend::new(libinput.clone());
    event_loop
        .handle()
        .insert_source(input, |event, _, state| {
            state.process_input_event(event);
            state.needs_redraw = true;
        })
        .map_err(|e| err("input source", e))?;

    // Page flips.
    event_loop
        .handle()
        .insert_source(drm_events, |event, _, state| match event {
            DrmEvent::VBlank(crtc) => {
                if let Some(screen) = state
                    .drm
                    .as_mut()
                    .and_then(|drm| drm.screens.get_mut(&crtc))
                {
                    let _ = screen.hw.frame_submitted();
                    screen.pending = false;
                }
            }
            DrmEvent::Error(error) => eprintln!("drm: {error}"),
        })
        .map_err(|e| err("DRM events", e))?;

    // Virtual terminal switches.
    event_loop
        .handle()
        .insert_source(notifier, |event, _, state| {
            let Some(drm) = state.drm.as_mut() else {
                return;
            };
            match event {
                SessionEvent::PauseSession => {
                    drm.libinput.suspend();
                    drm.manager.pause();
                }
                SessionEvent::ActivateSession => {
                    if drm.libinput.resume().is_err() {
                        eprintln!("drm: could not resume input");
                    }
                    if let Err(error) = drm.manager.activate(false) {
                        eprintln!("drm: could not resume display: {error}");
                    }
                    for screen in drm.screens.values_mut() {
                        screen.pending = false;
                    }
                    state.needs_redraw = true;
                }
            }
        })
        .map_err(|e| err("session events", e))?;

    // Demand-driven drawing, as in the nested backend.
    event_loop
        .handle()
        .insert_source(Timer::immediate(), |_, _, state| {
            if state.needs_redraw
                && state
                    .drm
                    .as_ref()
                    .is_some_and(|drm| drm.session.is_active())
            {
                state.needs_redraw = !render_all(state);
            }
            TimeoutAction::ToDuration(FRAME)
        })
        .map_err(|e| err("render timer", e))?;

    state.drm = Some(DrmBackend {
        session,
        renderer,
        manager,
        screens,
        libinput,
        cursor_outline: SolidColorBuffer::new((12, 12), [0.0, 0.0, 0.0, 1.0]),
        cursor_fill: SolidColorBuffer::new((8, 8), [1.0, 1.0, 1.0, 1.0]),
    });
    state.needs_redraw = true;
    state.retile();
    Ok(())
}

/// Set up every connected screen, left to right, at its preferred mode.
fn connect_screens(
    state: &mut AltruSession,
    manager: &mut OutputManager,
    renderer: &mut GlesRenderer,
) -> Result<HashMap<crtc::Handle, Screen>, Box<dyn std::error::Error>> {
    let device = manager.device();
    let resources = device
        .resource_handles()
        .map_err(|e| err("DRM resources", e))?;
    let mut used_crtcs = Vec::new();
    let mut plans = Vec::new();
    for handle in resources.connectors() {
        let Ok(info) = device.get_connector(*handle, true) else {
            continue;
        };
        if info.state() != connector::State::Connected || info.modes().is_empty() {
            continue;
        }
        let crtc = info
            .encoders()
            .iter()
            .filter_map(|encoder| device.get_encoder(*encoder).ok())
            .flat_map(|encoder| resources.filter_crtcs(encoder.possible_crtcs()))
            .find(|crtc| !used_crtcs.contains(crtc));
        let Some(crtc) = crtc else {
            eprintln!("drm: no free CRTC for {:?}", info.interface());
            continue;
        };
        used_crtcs.push(crtc);
        plans.push((info, crtc));
    }

    let mut screens = HashMap::new();
    let mut x = 0;
    for (info, crtc) in plans {
        let mode = info
            .modes()
            .iter()
            .find(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
            .copied()
            .unwrap_or(info.modes()[0]);
        let name = format!("{}-{}", info.interface().as_str(), info.interface_id());
        let (width_mm, height_mm) = info.size().unwrap_or((0, 0));
        let output = Output::new(
            name.clone(),
            PhysicalProperties {
                size: (width_mm as i32, height_mm as i32).into(),
                subpixel: Subpixel::Unknown,
                make: "Altru".into(),
                model: name.clone(),
            },
        );
        let _global = output.create_global::<AltruSession>(&state.display_handle);
        let wl_mode = WlMode::from(mode);
        output.set_preferred(wl_mode);
        output.change_current_state(Some(wl_mode), None, None, Some((x, 0).into()));
        state.space.map_output(&output, (x, 0));
        x += wl_mode.size.w;

        let hw = manager
            .initialize_output::<_, SessionElement>(
                crtc,
                mode,
                &[info.handle()],
                &output,
                None,
                renderer,
                &DrmOutputRenderElements::default(),
            )
            .map_err(|e| err(&format!("screen {name}"), e))?;
        println!(
            "drm: screen {name} {}x{}@{}",
            wl_mode.size.w,
            wl_mode.size.h,
            wl_mode.refresh / 1000
        );
        screens.insert(
            crtc,
            Screen {
                output,
                hw,
                pending: false,
            },
        );
    }
    Ok(screens)
}

/// Draw every screen that isn't waiting for a flip. Returns true when all were drawn.
fn render_all(state: &mut AltruSession) -> bool {
    let pointer_location = state
        .seat
        .get_pointer()
        .map(|pointer| pointer.current_location())
        .unwrap_or_default();
    let cursor = state.cursor_status.clone();
    let Some(drm) = state.drm.as_mut() else {
        return true;
    };
    let mut all_drawn = true;
    let mut drawn_outputs = Vec::new();
    for screen in drm.screens.values_mut() {
        if screen.pending {
            all_drawn = false;
            continue;
        }
        let Some(geometry) = state.space.output_geometry(&screen.output) else {
            continue;
        };
        let mut elements: Vec<SessionElement> = Vec::new();

        // Cursor first: elements are front to back.
        if geometry.to_f64().contains(pointer_location) {
            let at = (pointer_location - geometry.loc.to_f64()).to_physical(1.0);
            let at: Point<i32, Physical> = (at.x.round() as i32, at.y.round() as i32).into();
            match &cursor {
                CursorImageStatus::Hidden => {}
                CursorImageStatus::Surface(surface) => {
                    let hotspot = with_states(surface, |states| {
                        states
                            .data_map
                            .get::<CursorImageSurfaceData>()
                            .map(|data| data.lock().unwrap().hotspot)
                            .unwrap_or_default()
                    });
                    let origin = at - hotspot.to_physical(1);
                    elements.extend(
                        render_elements_from_surface_tree(
                            &mut drm.renderer,
                            surface,
                            origin,
                            Scale::from(1.0),
                            1.0,
                            Kind::Cursor,
                        )
                        .into_iter()
                        .map(SessionElement::Surface),
                    );
                }
                CursorImageStatus::Named(_) => {
                    elements.push(SessionElement::Solid(SolidColorRenderElement::from_buffer(
                        &drm.cursor_fill,
                        at + Point::from((2, 2)),
                        1.0,
                        1.0,
                        Kind::Cursor,
                    )));
                    elements.push(SessionElement::Solid(SolidColorRenderElement::from_buffer(
                        &drm.cursor_outline,
                        at,
                        1.0,
                        1.0,
                        Kind::Cursor,
                    )));
                }
            }
        }

        match space_render_elements(&mut drm.renderer, [&state.space], &screen.output, 1.0) {
            Ok(space) => elements.extend(space.into_iter().map(SessionElement::Space)),
            Err(error) => eprintln!("drm: {error:?}"),
        }

        let drawn = screen
            .hw
            .render_frame(&mut drm.renderer, &elements, CLEAR, FrameFlags::DEFAULT)
            .map(|result| !result.is_empty);
        match drawn {
            Ok(true) => match screen.hw.queue_frame(()) {
                Ok(()) => screen.pending = true,
                Err(error) => eprintln!("drm: queue frame: {error}"),
            },
            Ok(false) => {}
            Err(error) => eprintln!("drm: render: {error}"),
        }

        drawn_outputs.push(screen.output.clone());
    }
    for output in &drawn_outputs {
        state.send_frames(output);
        layer_map_for_output(output).cleanup();
    }
    state.space.refresh();
    state.popups.cleanup();
    all_drawn
}

impl DmabufHandler for AltruSession {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        let imported = self
            .drm
            .as_mut()
            .is_some_and(|drm| drm.renderer.import_dmabuf(&dmabuf, None).is_ok());
        if imported {
            let _ = notifier.successful::<AltruSession>();
        } else {
            notifier.failed();
        }
    }
}

delegate_dmabuf!(AltruSession);

impl AltruSession {
    /// Switch to virtual terminal `vt` (hardware backend only).
    pub fn switch_vt(&mut self, vt: i32) {
        if let Some(drm) = self.drm.as_mut()
            && let Err(error) = drm.session.change_vt(vt)
        {
            eprintln!("drm: could not switch to VT {vt}: {error}");
        }
    }
}
