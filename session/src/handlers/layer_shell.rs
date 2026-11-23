//! wlr-layer-shell: panels (Waybar), launchers (fuzzel), notifications (mako),
//! wallpapers. They sit above or below the windows, and space a panel reserves
//! (its exclusive zone) is kept out of the tiling, so windows never hide under it.

use smithay::{
    delegate_layer_shell,
    desktop::{LayerSurface, WindowSurfaceType, layer_map_for_output},
    output::Output,
    reexports::wayland_server::protocol::{wl_output::WlOutput, wl_surface::WlSurface},
    utils::{Logical, Point},
    wayland::{
        compositor::with_states,
        shell::wlr_layer::{
            KeyboardInteractivity, Layer, LayerSurface as WlrLayerSurface, LayerSurfaceData,
            WlrLayerShellHandler, WlrLayerShellState,
        },
    },
};

use crate::AltruSession;

impl WlrLayerShellHandler for AltruSession {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurface,
        wl_output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        let output = wl_output
            .as_ref()
            .and_then(Output::from_resource)
            .or_else(|| self.space.outputs().next().cloned());
        let Some(output) = output else {
            surface.send_close();
            return;
        };
        let mut map = layer_map_for_output(&output);
        if let Err(error) = map.map_layer(&LayerSurface::new(surface, namespace)) {
            eprintln!("layer-shell: could not map surface: {error}");
        }
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        let had_focus = self
            .seat
            .get_keyboard()
            .and_then(|k| k.current_focus())
            .is_some_and(|focus| focus == *surface.wl_surface());
        for output in self.space.outputs().cloned().collect::<Vec<_>>() {
            let mut map = layer_map_for_output(&output);
            let layer = map
                .layers()
                .find(|l| l.layer_surface() == &surface)
                .cloned();
            if let Some(layer) = layer {
                map.unmap_layer(&layer);
            }
        }
        // A closed launcher hands the keyboard back to the windows.
        if had_focus && !self.tiled.is_empty() {
            self.focus_index(0);
        }
        self.retile();
    }
}

delegate_layer_shell!(AltruSession);

impl AltruSession {
    /// Layer-surface part of a commit: arrange, send the first configure, and give
    /// the keyboard to surfaces that ask for it exclusively (launchers, lock screens).
    pub fn layer_commit(&mut self, surface: &WlSurface) {
        let Some(output) = self
            .space
            .outputs()
            .find(|o| {
                layer_map_for_output(o)
                    .layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
                    .is_some()
            })
            .cloned()
        else {
            return;
        };
        let initial_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<LayerSurfaceData>()
                .map(|data| data.lock().unwrap().initial_configure_sent)
                .unwrap_or(true)
        });
        let wants_keyboard = {
            let mut map = layer_map_for_output(&output);
            let zone_before = map.non_exclusive_zone();
            map.arrange();
            let zone_changed = map.non_exclusive_zone() != zone_before;
            let layer = map
                .layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
                .cloned();
            drop(map);
            if zone_changed {
                self.retile();
            }
            let Some(layer) = layer else {
                return;
            };
            if !initial_sent {
                layer.layer_surface().send_configure();
                false
            } else {
                layer.cached_state().keyboard_interactivity == KeyboardInteractivity::Exclusive
            }
        };
        if wants_keyboard {
            let keyboard = self.seat.get_keyboard().unwrap();
            if keyboard.current_focus().as_ref() != Some(surface) {
                keyboard.set_focus(
                    self,
                    Some(surface.clone()),
                    smithay::utils::SERIAL_COUNTER.next_serial(),
                );
            }
        }
    }

    /// The layer surface under `pos` on `layers` (checked in order), if any.
    pub fn layer_surface_under(
        &self,
        pos: Point<f64, Logical>,
        layers: &[Layer],
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        let output = self.space.outputs().find(|o| {
            self.space
                .output_geometry(o)
                .is_some_and(|g| g.to_f64().contains(pos))
        })?;
        let origin = self.space.output_geometry(output)?.loc;
        let map = layer_map_for_output(output);
        for layer_kind in layers {
            let local = pos - origin.to_f64();
            if let Some(layer) = map.layer_under(*layer_kind, local) {
                let layer_loc = map.layer_geometry(layer)?.loc;
                return layer
                    .surface_under(local - layer_loc.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, p)| (s, (p + layer_loc + origin).to_f64()));
            }
        }
        None
    }
}
