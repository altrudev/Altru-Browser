mod compositor;
mod xdg_shell;
mod xwayland;

use crate::AltruSession;

//
// Wl Seat
//

use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::output::OutputHandler;
use smithay::wayland::selection::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
    set_data_device_focus,
};
use smithay::wayland::selection::primary_selection::{
    PrimarySelectionHandler, PrimarySelectionState, set_primary_focus,
};
use smithay::wayland::selection::{SelectionHandler, SelectionSource, SelectionTarget};
use smithay::{delegate_data_device, delegate_output, delegate_primary_selection, delegate_seat};
use std::os::fd::OwnedFd;

impl SeatHandler for AltruSession {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<AltruSession> {
        &mut self.seat_state
    }

    fn cursor_image(
        &mut self,
        _seat: &Seat<Self>,
        image: smithay::input::pointer::CursorImageStatus,
    ) {
        self.cursor_status = image;
        self.needs_redraw = true;
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let dh = &self.display_handle;
        let client = focused.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client.clone());
        set_primary_focus(dh, seat, client);
    }
}

delegate_seat!(AltruSession);

//
// Wl Data Device
//

/// Copy and paste between Wayland and X11 apps: a Wayland app's selection is
/// offered to X11 through the XWayland window manager, and X11 asks it for data.
impl SelectionHandler for AltruSession {
    type SelectionUserData = ();

    fn new_selection(
        &mut self,
        target: SelectionTarget,
        source: Option<SelectionSource>,
        _seat: Seat<Self>,
    ) {
        if let Some(xwm) = self.xwm.as_mut()
            && let Err(error) = xwm.new_selection(target, source.map(|s| s.mime_types()))
        {
            eprintln!("clipboard: could not offer the selection to X11 apps: {error}");
        }
    }

    fn send_selection(
        &mut self,
        target: SelectionTarget,
        mime_type: String,
        fd: OwnedFd,
        _seat: Seat<Self>,
        _user_data: &(),
    ) {
        if let Some(xwm) = self.xwm.as_mut()
            && let Err(error) = xwm.send_selection(target, mime_type, fd, self.loop_handle.clone())
        {
            eprintln!("clipboard: could not read the X11 selection: {error}");
        }
    }
}

impl PrimarySelectionHandler for AltruSession {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection_state
    }
}

delegate_primary_selection!(AltruSession);

impl DataDeviceHandler for AltruSession {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for AltruSession {}
impl ServerDndGrabHandler for AltruSession {}

delegate_data_device!(AltruSession);

//
// Wl Output & Xdg Output
//

impl OutputHandler for AltruSession {}
delegate_output!(AltruSession);
