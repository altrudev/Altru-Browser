use std::{ffi::OsString, sync::Arc};

use smithay::{
    desktop::{PopupManager, Space, Window, WindowSurfaceType, layer_map_for_output},
    input::{Seat, SeatState, pointer::CursorImageStatus},
    output::Output,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    reexports::{
        calloop::{
            EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction, generic::Generic,
        },
        wayland_server::{
            Display, DisplayHandle, Resource,
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::WlSurface,
        },
    },
    utils::{IsAlive, Logical, Point, Rectangle, SERIAL_COUNTER},
    wayland::{
        compositor::{CompositorClientState, CompositorState, with_states},
        dmabuf::{DmabufGlobal, DmabufState},
        output::OutputManagerState,
        seat::WaylandFocus,
        selection::{data_device::DataDeviceState, primary_selection::PrimarySelectionState},
        shell::wlr_layer::{Layer, WlrLayerShellState},
        shell::xdg::{XdgShellState, XdgToplevelSurfaceData},
        shm::ShmState,
        socket::ListeningSocketSource,
        xwayland_shell::XWaylandShellState,
    },
    xwayland::X11Wm,
};

use crate::CalloopData;
use crate::profile::{App, is_installed};
use crate::shortcuts::{Action, cycle};

/// App launchers tried by Super+D, first installed wins. All use wlr-layer-shell.
const LAUNCHERS: &[&[&str]] = &[&["fuzzel"], &["wofi", "--show", "drun"], &["bemenu-run"]];

pub struct AltruSession {
    pub start_time: std::time::Instant,
    pub socket_name: OsString,
    pub display_handle: DisplayHandle,

    pub space: Space<Window>,
    /// Windows in layout order: the first is the main (left) window.
    pub tiled: Vec<Window>,
    /// Launch position of each window in `tiled` (same order).
    tiled_rank: Vec<usize>,
    /// Process ids of the apps this session started, in launch order.
    pub launch_order: Vec<u32>,
    /// `PIPEWIRE_LATENCY` for apps this session starts, when a clock is forced.
    pub audio_latency: Option<String>,
    /// Apps to start once XWayland is ready (or has failed).
    pub pending_launch: Vec<App>,
    pub loop_handle: LoopHandle<'static, CalloopData>,
    pub xwayland_shell_state: XWaylandShellState,
    pub xwm: Option<X11Wm>,
    /// X11 display number served by XWayland, once it is ready.
    pub xdisplay: Option<u32>,
    /// Something on screen changed since the last frame.
    pub needs_redraw: bool,
    pub loop_signal: LoopSignal,

    // Smithay State
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<AltruSession>,
    pub data_device_state: DataDeviceState,
    pub primary_selection_state: PrimarySelectionState,
    pub layer_shell_state: WlrLayerShellState,
    pub popups: PopupManager,

    pub seat: Seat<Self>,
    /// What the pointer should look like, as set by the app under it.
    pub cursor_status: CursorImageStatus,
    /// Hardware backend, when running directly on the machine.
    pub drm: Option<crate::drm::DrmBackend>,
    pub dmabuf_state: DmabufState,
    pub dmabuf_global: Option<DmabufGlobal>,
}

impl AltruSession {
    pub fn new(event_loop: &mut EventLoop<'static, CalloopData>, display: Display<Self>) -> Self {
        let start_time = std::time::Instant::now();

        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let primary_selection_state = PrimarySelectionState::new::<Self>(&dh);
        let layer_shell_state = WlrLayerShellState::new::<Self>(&dh);
        let xwayland_shell_state = XWaylandShellState::new::<Self>(&dh);
        let popups = PopupManager::default();

        // A seat is a group of keyboards, pointer and touch devices.
        // A seat typically has a pointer and maintains a keyboard focus and a pointer focus.
        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "altru");

        // Notify clients that we have a keyboard, for the sake of the example we assume that keyboard is always present.
        // You may want to track keyboard hot-plug in real compositor.
        seat.add_keyboard(Default::default(), 200, 25).unwrap();

        // Notify clients that we have a pointer (mouse)
        // Here we assume that there is always pointer plugged in
        seat.add_pointer();

        // A space represents a two-dimensional plane. Windows and Outputs can be mapped onto it.
        //
        // Windows get a position and stacking order through mapping.
        // Outputs become views of a part of the Space and can be rendered via Space::render_output.
        let space = Space::default();

        let socket_name = Self::init_wayland_listener(display, event_loop);

        // Get the loop signal, used to stop the event loop
        let loop_signal = event_loop.get_signal();

        Self {
            start_time,
            display_handle: dh,

            space,
            tiled: Vec::new(),
            tiled_rank: Vec::new(),
            launch_order: Vec::new(),
            audio_latency: None,
            pending_launch: Vec::new(),
            loop_handle: event_loop.handle(),
            xwayland_shell_state,
            xwm: None,
            xdisplay: None,
            needs_redraw: true,
            loop_signal,
            socket_name,

            compositor_state,
            xdg_shell_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            primary_selection_state,
            layer_shell_state,
            popups,
            seat,
            cursor_status: CursorImageStatus::default_named(),
            drm: None,
            dmabuf_state: DmabufState::new(),
            dmabuf_global: None,
        }
    }

    fn init_wayland_listener(
        display: Display<AltruSession>,
        event_loop: &mut EventLoop<CalloopData>,
    ) -> OsString {
        // Creates a new listening socket, automatically choosing the next available `wayland` socket name.
        let listening_socket = ListeningSocketSource::new_auto().unwrap();

        // Get the name of the listening socket.
        // Clients will connect to this socket.
        let socket_name = listening_socket.socket_name().to_os_string();

        let loop_handle = event_loop.handle();

        loop_handle
            .insert_source(listening_socket, move |client_stream, _, state| {
                // Inside the callback, you should insert the client into the display.
                //
                // You may also associate some data with the client when inserting the client.
                state
                    .display_handle
                    .insert_client(client_stream, Arc::new(ClientState::default()))
                    .unwrap();
            })
            .expect("Failed to init the wayland event source.");

        // You also need to add the display itself to the event loop, so that client events will be processed by wayland-server.
        loop_handle
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    // Safety: we don't drop the display
                    unsafe {
                        display.get_mut().dispatch_clients(state).unwrap();
                    }
                    Ok(PostAction::Continue)
                },
            )
            .unwrap();

        socket_name
    }

    /// Add a new window to the layout, keeping launch order: the first app
    /// listed stays the main window however fast each app starts. Windows from
    /// processes this session didn't start (or that re-spawned themselves) go last.
    pub fn add_tiled(&mut self, window: Window) {
        let pid = window
            .wl_surface()
            .and_then(|surface| surface.client())
            .and_then(|client| client.get_credentials(&self.display_handle).ok())
            .map(|credentials| credentials.pid as u32);
        let rank = pid
            .and_then(|pid| {
                self.launch_order
                    .iter()
                    .position(|launched| *launched == pid)
            })
            .unwrap_or(usize::MAX);
        let at = self
            .tiled_rank
            .partition_point(|existing| *existing <= rank);
        self.tiled.insert(at, window);
        self.tiled_rank.insert(at, rank);
    }

    /// Start the apps waiting in `pending_launch`, reporting missing ones with the
    /// package to install.
    pub fn launch_pending(&mut self) {
        let path_var = std::env::var("PATH").ok();
        for app in std::mem::take(&mut self.pending_launch) {
            let program = &app.argv[0];
            if !is_installed(program, path_var.as_deref()) {
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
            match self.launch(&app.argv) {
                Ok(pid) => println!("started: {} (pid {pid})", app.argv.join(" ")),
                Err(error) => eprintln!("failed: {}: {error}", app.argv.join(" ")),
            }
        }
    }

    /// Start a program on this session. Wayland toolkits are selected; DISPLAY
    /// points at this session's XWayland (or is removed while there is none), so
    /// nothing silently opens on another desktop.
    pub fn launch(&mut self, argv: &[String]) -> std::io::Result<u32> {
        let (program, args) = argv.split_first().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "empty command")
        })?;
        let mut command = std::process::Command::new(program);
        if let Some(latency) = &self.audio_latency {
            command.env("PIPEWIRE_LATENCY", latency);
        }
        command
            .args(args)
            .env("WAYLAND_DISPLAY", &self.socket_name)
            .env("XDG_SESSION_TYPE", "wayland")
            .env("GDK_BACKEND", "wayland")
            .env("QT_QPA_PLATFORM", "wayland")
            .env("SDL_VIDEODRIVER", "wayland")
            .env("MOZ_ENABLE_WAYLAND", "1")
            .env("ELECTRON_OZONE_PLATFORM_HINT", "wayland")
            .env_remove("DISPLAY");
        // X11-only apps reach this session's XWayland, never another desktop.
        if let Some(display) = self.xdisplay {
            command.env("DISPLAY", format!(":{display}"));
        }
        let child = command.spawn()?;
        self.launch_order.push(child.id());
        Ok(child.id())
    }

    fn focused_index(&self) -> Option<usize> {
        let focus = self.seat.get_keyboard()?.current_focus()?;
        self.tiled
            .iter()
            .position(|w| w.wl_surface().is_some_and(|s| *s == focus))
    }

    /// Give keyboard focus to the window at layout position `index`.
    pub fn focus_index(&mut self, index: usize) {
        let Some(window) = self.tiled.get(index).cloned() else {
            return;
        };
        let Some(surface) = window.wl_surface().map(|s| s.into_owned()) else {
            return;
        };
        self.space.raise_element(&window, true);
        // X11 windows also need X11 focus and stacking.
        for other in &self.tiled {
            if let Some(x11) = other.x11_surface() {
                let _ = x11.set_activated(other == &window);
            }
        }
        if let (Some(x11), Some(wm)) = (window.x11_surface(), self.xwm.as_mut()) {
            let _ = wm.raise_window(x11);
        }
        let keyboard = self.seat.get_keyboard().unwrap();
        keyboard.set_focus(self, Some(surface), SERIAL_COUNTER.next_serial());
        self.needs_redraw = true;
    }

    pub fn run_action(&mut self, action: Action) {
        match action {
            Action::FocusNext | Action::FocusPrevious => {
                let forward = action == Action::FocusNext;
                if let Some(index) = cycle(self.focused_index(), self.tiled.len(), forward) {
                    self.focus_index(index);
                }
            }
            Action::FocusIndex(index) => self.focus_index(index),
            Action::MakeMain => {
                if let Some(index) = self.focused_index().filter(|i| *i > 0) {
                    let window = self.tiled.remove(index);
                    self.tiled_rank.remove(index);
                    let first_rank = self.tiled_rank.first().copied().unwrap_or(usize::MAX);
                    self.tiled.insert(0, window);
                    self.tiled_rank.insert(0, first_rank);
                    self.retile();
                    self.focus_index(0);
                }
            }
            Action::OpenTerminal => {
                if let Err(error) = self.launch(&["foot".to_string()]) {
                    eprintln!("altru-session: could not open a terminal: {error}");
                }
            }
            Action::OpenLauncher => {
                let path_var = std::env::var("PATH").ok();
                let launcher = LAUNCHERS
                    .iter()
                    .find(|argv| is_installed(argv[0], path_var.as_deref()));
                match launcher {
                    Some(argv) => {
                        let argv: Vec<String> = argv.iter().map(|a| a.to_string()).collect();
                        if let Err(error) = self.launch(&argv) {
                            eprintln!("altru-session: could not open the launcher: {error}");
                        }
                    }
                    None => eprintln!(
                        "altru-session: no app launcher installed (install: sudo apt install fuzzel)"
                    ),
                }
            }
            Action::Close => {
                if let Some(index) = self.focused_index() {
                    let window = &self.tiled[index];
                    if let Some(toplevel) = window.toplevel() {
                        toplevel.send_close();
                    } else if let Some(x11) = window.x11_surface() {
                        let _ = x11.close();
                    }
                }
            }
            Action::Quit => self.loop_signal.stop(),
            Action::SwitchVt(vt) => self.switch_vt(vt),
            Action::None => {}
        }
        if action != Action::None {
            println!("shortcut: {action:?}");
        }
    }

    /// Remove `window` from the layout.
    pub fn remove_tiled_window(&mut self, window: &Window) {
        if let Some(index) = self.tiled.iter().position(|w| w == window) {
            self.tiled.remove(index);
            self.tiled_rank.remove(index);
        }
    }

    /// Remove the window showing `surface` from the layout.
    pub fn remove_tiled(&mut self, surface: &WlSurface) {
        if let Some(index) = self
            .tiled
            .iter()
            .position(|w| w.toplevel().is_some_and(|t| t.wl_surface() == surface))
        {
            self.tiled.remove(index);
            self.tiled_rank.remove(index);
        }
    }

    /// Lay every window out again with `crate::layout::tiles`.
    /// Called when a window opens or closes and when the screen size changes.
    pub fn retile(&mut self) {
        self.needs_redraw = true;
        let mut ranks = std::mem::take(&mut self.tiled_rank).into_iter();
        let mut kept_ranks = Vec::new();
        self.tiled.retain(|window| {
            let rank = ranks.next().unwrap_or(usize::MAX);
            let alive = window.alive();
            if alive {
                kept_ranks.push(rank);
            }
            alive
        });
        self.tiled_rank = kept_ranks;
        let Some(output) = self.space.outputs().next().cloned() else {
            return;
        };
        let Some(output_geometry) = self.space.output_geometry(&output) else {
            return;
        };
        // Keep panels' reserved space out of the tiling.
        let mut geometry = layer_map_for_output(&output).non_exclusive_zone();
        geometry.loc += output_geometry.loc;
        let tiles = crate::layout::tiles(self.tiled.len(), geometry.size.w, geometry.size.h);
        let many = self.tiled.len() > 1;
        for (window, tile) in self.tiled.clone().into_iter().zip(tiles) {
            if let Some(toplevel) = window.toplevel() {
                toplevel.with_pending_state(|state| {
                    state.size = Some((tile.width, tile.height).into());
                    // Tell apps they are tiled, so they drop shadows and rounded corners.
                    for edge in [
                        xdg_toplevel::State::TiledLeft,
                        xdg_toplevel::State::TiledRight,
                        xdg_toplevel::State::TiledTop,
                        xdg_toplevel::State::TiledBottom,
                    ] {
                        if many {
                            state.states.set(edge);
                        } else {
                            state.states.unset(edge);
                        }
                    }
                    if many {
                        state.states.unset(xdg_toplevel::State::Maximized);
                    } else {
                        state.states.set(xdg_toplevel::State::Maximized);
                    }
                });
                let initial_sent = with_states(toplevel.wl_surface(), |states| {
                    states
                        .data_map
                        .get::<XdgToplevelSurfaceData>()
                        .map(|data| data.lock().unwrap().initial_configure_sent)
                        .unwrap_or(false)
                });
                if initial_sent {
                    toplevel.send_pending_configure();
                }
            }
            let location = (geometry.loc.x + tile.x, geometry.loc.y + tile.y);
            if let Some(x11) = window.x11_surface() {
                let _ = x11.configure(Rectangle::new(
                    location.into(),
                    (tile.width, tile.height).into(),
                ));
            }
            self.space.map_element(window, location, false);
        }
    }

    pub fn surface_under(
        &self,
        pos: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        // Panels and launchers above the windows, then windows, then wallpapers.
        self.layer_surface_under(pos, &[Layer::Overlay, Layer::Top])
            .or_else(|| {
                self.space
                    .element_under(pos)
                    .and_then(|(window, location)| {
                        window
                            .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                            .map(|(s, p)| (s, (p + location).to_f64()))
                    })
            })
            .or_else(|| self.layer_surface_under(pos, &[Layer::Bottom, Layer::Background]))
    }

    /// Ask every window and layer surface on `output` to draw its next frame.
    pub fn send_frames(&self, output: &Output) {
        let elapsed = self.start_time.elapsed();
        let primary = |_: &WlSurface, _: &_| Some(output.clone());
        self.space.elements().for_each(|window| {
            window.send_frame(output, elapsed, Some(std::time::Duration::ZERO), primary)
        });
        for layer in layer_map_for_output(output).layers() {
            layer.send_frame(output, elapsed, Some(std::time::Duration::ZERO), primary);
        }
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}
