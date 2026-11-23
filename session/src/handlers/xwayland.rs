//! XWayland: X11-only programs (some DAWs, many audio plugin windows, older tools)
//! run inside Altru Session and are tiled like any other window. Menus, tooltips
//! and other override-redirect windows float where the app puts them.

use std::{os::fd::OwnedFd, process::Stdio};

use smithay::{
    delegate_xwayland_shell,
    desktop::Window,
    reexports::calloop::EventLoop,
    utils::{Logical, Rectangle},
    wayland::{
        seat::WaylandFocus,
        selection::{
            SelectionTarget,
            data_device::{
                clear_data_device_selection, current_data_device_selection_userdata,
                request_data_device_client_selection, set_data_device_selection,
            },
            primary_selection::{
                clear_primary_selection, current_primary_selection_userdata,
                request_primary_client_selection, set_primary_selection,
            },
        },
        xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    },
    xwayland::{
        X11Surface, X11Wm, XWayland, XWaylandEvent, XwmHandler,
        xwm::{Reorder, ResizeEdge, XwmId},
    },
};

use crate::{AltruSession, CalloopData};

impl AltruSession {
    /// Start XWayland. Apps waiting in `pending_launch` start once it is ready, or
    /// right away (Wayland-only) if XWayland can't start.
    pub fn start_xwayland(&mut self, event_loop: &mut EventLoop<CalloopData>) {
        let spawned = XWayland::spawn(
            &self.display_handle,
            None,
            std::iter::empty::<(String, String)>(),
            true,
            Stdio::null(),
            Stdio::null(),
            |_| (),
        );
        let (xwayland, client) = match spawned {
            Ok(pair) => pair,
            Err(error) => {
                eprintln!(
                    "xwayland: not available ({error}); X11-only apps won't open. Install: sudo apt install xwayland"
                );
                self.launch_pending();
                return;
            }
        };
        let inserted =
            event_loop
                .handle()
                .insert_source(xwayland, move |event, _, data| match event {
                    XWaylandEvent::Ready {
                        x11_socket,
                        display_number,
                    } => {
                        let handle = data.loop_handle.clone();
                        match X11Wm::start_wm(handle, x11_socket, client.clone()) {
                            Ok(wm) => {
                                data.xwm = Some(wm);
                                data.xdisplay = Some(display_number);
                                println!("xwayland: ready on :{display_number}");
                            }
                            Err(error) => eprintln!("xwayland: window manager failed: {error}"),
                        }
                        data.launch_pending();
                    }
                    XWaylandEvent::Error => {
                        eprintln!("xwayland: crashed on startup; X11-only apps won't open");
                        data.launch_pending();
                    }
                });
        if let Err(error) = inserted {
            eprintln!("xwayland: could not watch the server: {error}");
            self.launch_pending();
        }
    }

    fn x11_window(&self, surface: &X11Surface) -> Option<Window> {
        self.space
            .elements()
            .find(|w| w.x11_surface() == Some(surface))
            .cloned()
    }
}

impl XWaylandShellHandler for AltruSession {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.xwayland_shell_state
    }
}

delegate_xwayland_shell!(AltruSession);

impl XwmHandler for AltruSession {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.xwm
            .as_mut()
            .expect("X11 events only arrive once the WM is running")
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn map_window_request(&mut self, _xwm: XwmId, surface: X11Surface) {
        if surface.set_mapped(true).is_err() {
            return;
        }
        let window = Window::new_x11_window(surface);
        self.space.map_element(window.clone(), (0, 0), false);
        self.add_tiled(window.clone());
        self.retile();
        if let Some(index) = self.tiled.iter().position(|w| *w == window) {
            self.focus_index(index);
        }
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, surface: X11Surface) {
        let location = surface.geometry().loc;
        self.space
            .map_element(Window::new_x11_window(surface), location, true);
        self.needs_redraw = true;
    }

    fn unmapped_window(&mut self, _xwm: XwmId, surface: X11Surface) {
        if let Some(window) = self.x11_window(&surface) {
            self.remove_tiled_window(&window);
            self.space.unmap_elem(&window);
        }
        if !surface.is_override_redirect() {
            let _ = surface.set_mapped(false);
        }
        self.retile();
    }

    fn destroyed_window(&mut self, _xwm: XwmId, surface: X11Surface) {
        if let Some(window) = self.x11_window(&surface) {
            self.remove_tiled_window(&window);
            self.space.unmap_elem(&window);
            self.retile();
        }
    }

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        surface: X11Surface,
        x: Option<i32>,
        y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        // Tiled windows keep their tile: answer with the geometry they already have.
        if let Some(window) = self.x11_window(&surface)
            && self.tiled.contains(&window)
        {
            let _ = surface.configure(None);
            return;
        }
        let mut geometry = surface.geometry();
        if let Some(x) = x {
            geometry.loc.x = x;
        }
        if let Some(y) = y {
            geometry.loc.y = y;
        }
        if let Some(w) = w {
            geometry.size.w = w as i32;
        }
        if let Some(h) = h {
            geometry.size.h = h as i32;
        }
        let _ = surface.configure(geometry);
    }

    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        surface: X11Surface,
        geometry: Rectangle<i32, Logical>,
        _above: Option<u32>,
    ) {
        if let Some(window) = self.x11_window(&surface)
            && !self.tiled.contains(&window)
        {
            self.space.map_element(window, geometry.loc, false);
            self.needs_redraw = true;
        }
    }

    /// X11 apps may read the clipboard only while one of them has the keyboard,
    /// so a background X11 program can't watch what you copy.
    fn allow_selection_access(&mut self, xwm: XwmId, _selection: SelectionTarget) -> bool {
        let Some(focus) = self.seat.get_keyboard().and_then(|k| k.current_focus()) else {
            return false;
        };
        self.space.elements().any(|window| {
            window
                .x11_surface()
                .is_some_and(|x11| x11.xwm_id() == Some(xwm))
                && window.wl_surface().is_some_and(|s| *s == focus)
        })
    }

    fn send_selection(
        &mut self,
        _xwm: XwmId,
        selection: SelectionTarget,
        mime_type: String,
        fd: OwnedFd,
    ) {
        let result = match selection {
            SelectionTarget::Clipboard => {
                request_data_device_client_selection(&self.seat, mime_type, fd)
                    .map_err(|e| e.to_string())
            }
            SelectionTarget::Primary => request_primary_client_selection(&self.seat, mime_type, fd)
                .map_err(|e| e.to_string()),
        };
        if let Err(error) = result {
            eprintln!("clipboard: X11 app could not read the Wayland selection: {error}");
        }
    }

    fn new_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_types: Vec<String>) {
        match selection {
            SelectionTarget::Clipboard => {
                set_data_device_selection(&self.display_handle, &self.seat, mime_types, ())
            }
            SelectionTarget::Primary => {
                set_primary_selection(&self.display_handle, &self.seat, mime_types, ())
            }
        }
    }

    fn cleared_selection(&mut self, _xwm: XwmId, selection: SelectionTarget) {
        match selection {
            SelectionTarget::Clipboard => {
                if current_data_device_selection_userdata(&self.seat).is_some() {
                    clear_data_device_selection(&self.display_handle, &self.seat)
                }
            }
            SelectionTarget::Primary => {
                if current_primary_selection_userdata(&self.seat).is_some() {
                    clear_primary_selection(&self.display_handle, &self.seat)
                }
            }
        }
    }

    // Windows are tiled, so app-driven moves and resizes are ignored.
    fn resize_request(
        &mut self,
        _xwm: XwmId,
        _window: X11Surface,
        _button: u32,
        _edge: ResizeEdge,
    ) {
    }

    fn move_request(&mut self, _xwm: XwmId, _window: X11Surface, _button: u32) {}
}
