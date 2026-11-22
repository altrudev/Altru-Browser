//! Altru Metal: a PlatformHost where Altru Browser *is* the screen.
//!
//! Instead of opening a window inside someone else's desktop, Altru Metal
//! draws straight to the Linux framebuffer and reads the keyboard straight
//! from the kernel. Linux is used only as the driver substrate (see DDC-OS).
//!
//! Status: experimental first sketch, std-only (no new dependencies).
//!
//! - graphics: Linux fbdev (`/dev/fb0`), 32 bits per pixel only, fail closed
//!   on anything else. A DRM/KMS backend is the planned replacement.
//! - text: rendered as placeholder glyph cells until a rasterizer is wired
//!   behind `FontProvider`. Layout positions and sizes are real.
//! - input: raw `/dev/input/event*` keyboard events.
//! - storage: one private vault directory per PWX app id.
//! - network: denied by default, allowed per app and per origin.
//! - secrets: the PWX signer slot is never exposed to page content.
//!
//! Nothing here is promoted. Runtime evidence must come from real hardware.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::{DirBuilderExt, FileExt};
use std::path::{Path, PathBuf};

use crate::native_scene::{Scene, SceneCommand, SceneRenderer};
use crate::platform::{
    AccessibilityProvider, ClipboardProvider, FontProvider, GraphicsProvider, MediaProvider,
    NetworkProvider, PlatformFamily, PlatformHost, SandboxProvider, SecretProvider,
    StorageProvider, SurfaceSize, WindowProvider,
};

/// Identity reported in runtime evidence.
pub const METAL_HOST_IDENTITY: &str = "altru-metal-fbdev-v0";

// ---------------------------------------------------------------------------
// Colours (0x00RRGGBB). Neutral defaults; brand tokens can replace these.
// ---------------------------------------------------------------------------

pub const COLOR_BACKGROUND: u32 = 0x00FF_FFFF;
pub const COLOR_BOX: u32 = 0x00D8_DEE9;
pub const COLOR_TEXT: u32 = 0x0020_2833;
pub const COLOR_BAR: u32 = 0x001F_3A5F;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum MetalError {
    Io(io::Error),
    /// The framebuffer exists but is in a format this sketch refuses to guess.
    UnsupportedFramebuffer(String),
    InvalidAppId(String),
}

impl From<io::Error> for MetalError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

// ---------------------------------------------------------------------------
// Pixel frame (pure, testable without hardware)
// ---------------------------------------------------------------------------

/// An owned 32-bit pixel buffer, independent of any device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PixelFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u32>,
}

impl PixelFrame {
    pub fn new(width: u32, height: u32, color: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![color; width as usize * height as usize],
        }
    }

    /// Fill a rectangle, clipped to the frame. Negative or NaN input is clipped.
    pub fn fill_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: u32) {
        let clamp = |v: f32, max: u32| -> u32 {
            if v.is_nan() || v <= 0.0 {
                0
            } else if v >= max as f32 {
                max
            } else {
                v as u32
            }
        };
        let x0 = clamp(x, self.width);
        let y0 = clamp(y, self.height);
        let x1 = clamp(x + width, self.width);
        let y1 = clamp(y + height, self.height);
        for row in y0..y1 {
            let start = row as usize * self.width as usize;
            self.pixels[start + x0 as usize..start + x1 as usize].fill(color);
        }
    }

    pub fn stroke_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: u32) {
        self.fill_rect(x, y, width, 1.0, color);
        self.fill_rect(x, y + height - 1.0, width, 1.0, color);
        self.fill_rect(x, y, 1.0, height, color);
        self.fill_rect(x + width - 1.0, y, 1.0, height, color);
    }

    /// Binary PPM (P6), for testing in QEMU or on a machine with a desktop.
    pub fn to_ppm(&self) -> Vec<u8> {
        let mut out = format!("P6\n{} {}\n255\n", self.width, self.height).into_bytes();
        out.reserve(self.pixels.len() * 3);
        for pixel in &self.pixels {
            out.push((pixel >> 16) as u8);
            out.push((pixel >> 8) as u8);
            out.push(*pixel as u8);
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Scene -> pixels
// ---------------------------------------------------------------------------

/// Draws the engine-owned retained scene into a `PixelFrame`.
#[derive(Debug, Clone, Copy)]
pub struct MetalSceneRenderer {
    /// Offset applied to every scene coordinate (screen margin).
    pub origin_x: f32,
    pub origin_y: f32,
}

impl Default for MetalSceneRenderer {
    fn default() -> Self {
        Self {
            origin_x: 24.0,
            origin_y: 24.0,
        }
    }
}

impl SceneRenderer for MetalSceneRenderer {
    type Artifact = PixelFrame;

    fn name(&self) -> &'static str {
        "altru-metal-cpu-v0"
    }

    fn render(&self, scene: &Scene, width: u32, height: u32) -> PixelFrame {
        let mut frame = PixelFrame::new(width, height, COLOR_BACKGROUND);
        for command in &scene.commands {
            match command {
                SceneCommand::Rect {
                    x,
                    y,
                    width,
                    height,
                } => frame.stroke_rect(
                    self.origin_x + x,
                    self.origin_y + y,
                    *width,
                    *height,
                    COLOR_BOX,
                ),
                SceneCommand::Text {
                    x,
                    y,
                    size_px,
                    text,
                } => draw_placeholder_text(
                    &mut frame,
                    self.origin_x + x,
                    self.origin_y + y,
                    *size_px,
                    text,
                ),
            }
        }
        frame
    }
}

/// One solid cell per visible character, gaps for spaces.
/// Replaced once a glyph rasterizer sits behind `FontProvider`.
fn draw_placeholder_text(frame: &mut PixelFrame, x: f32, y: f32, size_px: f32, text: &str) {
    let size = if size_px.is_finite() && size_px > 0.0 {
        size_px
    } else {
        16.0
    };
    let advance = size * 0.55;
    let cell_width = (advance - 2.0).max(1.0);
    let cell_height = size * 0.7;
    let top = y + (size - cell_height) / 2.0;
    for (index, character) in text.chars().enumerate() {
        if !character.is_whitespace() {
            frame.fill_rect(
                x + index as f32 * advance,
                top,
                cell_width,
                cell_height,
                COLOR_TEXT,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Linux framebuffer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramebufferGeometry {
    pub width: u32,
    pub height: u32,
    pub stride_bytes: u32,
    pub bits_per_pixel: u32,
}

impl FramebufferGeometry {
    /// Read geometry from sysfs, e.g. `/sys/class/graphics/fb0`.
    pub fn from_sysfs(dir: &Path) -> Result<Self, MetalError> {
        let read = |name: &str| -> Result<String, MetalError> {
            Ok(fs::read_to_string(dir.join(name))?.trim().to_string())
        };
        let size = read("virtual_size")?;
        let (w, h) = size
            .split_once(',')
            .ok_or_else(|| MetalError::UnsupportedFramebuffer(format!("virtual_size={size}")))?;
        let parse = |label: &str, value: &str| -> Result<u32, MetalError> {
            value
                .trim()
                .parse::<u32>()
                .map_err(|_| MetalError::UnsupportedFramebuffer(format!("{label}={value}")))
        };
        let geometry = Self {
            width: parse("width", w)?,
            height: parse("height", h)?,
            stride_bytes: parse("stride", &read("stride")?)?,
            bits_per_pixel: parse("bits_per_pixel", &read("bits_per_pixel")?)?,
        };
        geometry.validate()?;
        Ok(geometry)
    }

    /// Fail closed: only 32bpp with a stride wide enough for the width.
    pub fn validate(&self) -> Result<(), MetalError> {
        if self.bits_per_pixel != 32 {
            return Err(MetalError::UnsupportedFramebuffer(format!(
                "bits_per_pixel={} (only 32 supported)",
                self.bits_per_pixel
            )));
        }
        if self.width == 0 || self.height == 0 || self.stride_bytes < self.width * 4 {
            return Err(MetalError::UnsupportedFramebuffer(format!(
                "width={} height={} stride={}",
                self.width, self.height, self.stride_bytes
            )));
        }
        Ok(())
    }
}

/// A Linux fbdev device. Assumes XRGB8888 little-endian, which is what the
/// Intel i915 / simpledrm framebuffer exposes on machines like the Lenovo G700.
pub struct LinuxFramebuffer {
    device: File,
    geometry: FramebufferGeometry,
}

impl LinuxFramebuffer {
    pub fn open_default() -> Result<Self, MetalError> {
        Self::open(Path::new("/dev/fb0"), Path::new("/sys/class/graphics/fb0"))
    }

    pub fn open(device: &Path, sysfs_dir: &Path) -> Result<Self, MetalError> {
        let geometry = FramebufferGeometry::from_sysfs(sysfs_dir)?;
        let device = OpenOptions::new().read(true).write(true).open(device)?;
        Ok(Self { device, geometry })
    }

    pub fn geometry(&self) -> FramebufferGeometry {
        self.geometry
    }

    /// Copy a frame to the screen, row by row, honouring the stride.
    pub fn present(&self, frame: &PixelFrame) -> Result<(), MetalError> {
        let width = frame.width.min(self.geometry.width) as usize;
        let height = frame.height.min(self.geometry.height) as usize;
        let mut row_bytes = vec![0u8; width * 4];
        for row in 0..height {
            let source = &frame.pixels[row * frame.width as usize..][..width];
            for (index, pixel) in source.iter().enumerate() {
                row_bytes[index * 4..index * 4 + 4].copy_from_slice(&pixel.to_le_bytes());
            }
            let offset = row as u64 * self.geometry.stride_bytes as u64;
            self.device.write_all_at(&row_bytes, offset)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Keyboard input (raw evdev)
// ---------------------------------------------------------------------------

const EV_KEY: u16 = 0x01;
const KEY_ESC: u16 = 1;
const KEY_L: u16 = 38;
const KEY_LEFTCTRL: u16 = 29;
const KEY_F6: u16 = 64;
const KEY_F12: u16 = 88;
const KEY_Q: u16 = 16;
/// `struct input_event` on 64-bit Linux: timeval (16) + type + code + value.
const INPUT_EVENT_SIZE: usize = 24;

/// The three approved shell states (Focus / Navigate / Inspect) plus exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellIntent {
    Focus,
    Navigate,
    Inspect,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: u16,
    /// 0 = release, 1 = press, 2 = auto-repeat.
    pub value: i32,
}

/// Decode one raw `input_event`. Returns `None` for non-key events.
pub fn decode_input_event(bytes: &[u8; INPUT_EVENT_SIZE]) -> Option<KeyEvent> {
    let kind = u16::from_le_bytes([bytes[16], bytes[17]]);
    if kind != EV_KEY {
        return None;
    }
    Some(KeyEvent {
        code: u16::from_le_bytes([bytes[18], bytes[19]]),
        value: i32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]),
    })
}

/// Maps key presses to shell intents. Tracks Ctrl for Ctrl+L.
#[derive(Debug, Default)]
pub struct ShellKeymap {
    ctrl_down: bool,
}

impl ShellKeymap {
    pub fn feed(&mut self, event: KeyEvent) -> Option<ShellIntent> {
        if event.code == KEY_LEFTCTRL {
            self.ctrl_down = event.value != 0;
            return None;
        }
        if event.value != 1 {
            return None;
        }
        match event.code {
            KEY_ESC => Some(ShellIntent::Focus),
            KEY_F6 => Some(ShellIntent::Navigate),
            KEY_L if self.ctrl_down => Some(ShellIntent::Navigate),
            KEY_F12 => Some(ShellIntent::Inspect),
            KEY_Q if self.ctrl_down => Some(ShellIntent::Quit),
            _ => None,
        }
    }
}

/// Find keyboard event devices from `/proc/bus/input/devices` text.
pub fn keyboard_event_nodes(proc_devices: &str) -> Vec<PathBuf> {
    let mut nodes = Vec::new();
    for block in proc_devices.split("\n\n") {
        let handlers = block
            .lines()
            .find_map(|line| line.strip_prefix("H: Handlers="))
            .unwrap_or("");
        let is_keyboard = handlers.split_whitespace().any(|h| h == "kbd")
            && block.lines().any(|line| line.starts_with("B: EV=120013"));
        if is_keyboard {
            for handler in handlers.split_whitespace() {
                if handler.starts_with("event") {
                    nodes.push(PathBuf::from("/dev/input").join(handler));
                }
            }
        }
    }
    nodes
}

pub struct KeyboardReader {
    device: File,
    keymap: ShellKeymap,
}

impl KeyboardReader {
    pub fn open_first() -> Result<Self, MetalError> {
        let text = fs::read_to_string("/proc/bus/input/devices")?;
        let node = keyboard_event_nodes(&text)
            .into_iter()
            .next()
            .ok_or_else(|| {
                MetalError::Io(io::Error::new(io::ErrorKind::NotFound, "no keyboard"))
            })?;
        Ok(Self {
            device: File::open(node)?,
            keymap: ShellKeymap::default(),
        })
    }

    /// Blocks until the next shell intent.
    pub fn next_intent(&mut self) -> Result<ShellIntent, MetalError> {
        let mut buffer = [0u8; INPUT_EVENT_SIZE];
        loop {
            self.device.read_exact(&mut buffer)?;
            if let Some(intent) = decode_input_event(&buffer).and_then(|e| self.keymap.feed(e)) {
                return Ok(intent);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Storage: one private vault per PWX app
// ---------------------------------------------------------------------------

/// App ids are lowercase letters, digits, `-` and `.`, with no `..`.
/// This is the only thing standing between an app id and a path, so it is strict.
pub fn validate_app_id(app_id: &str) -> Result<(), MetalError> {
    let ok = !app_id.is_empty()
        && app_id.len() <= 128
        && !app_id.starts_with('.')
        && !app_id.contains("..")
        && app_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.');
    if ok {
        Ok(())
    } else {
        Err(MetalError::InvalidAppId(app_id.to_string()))
    }
}

pub struct AppVaultStorage {
    root: PathBuf,
}

impl AppVaultStorage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn vault_path(&self, app_id: &str) -> Result<PathBuf, MetalError> {
        validate_app_id(app_id)?;
        Ok(self.root.join("apps").join(app_id).join("data"))
    }

    /// Create the vault with owner-only permissions (0700).
    pub fn open_vault(&self, app_id: &str) -> Result<PathBuf, MetalError> {
        let path = self.vault_path(app_id)?;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&path)?;
        Ok(path)
    }
}

impl StorageProvider for AppVaultStorage {}

// ---------------------------------------------------------------------------
// Network: denied by default
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct AppNetworkPolicy {
    allowed: BTreeMap<String, BTreeSet<String>>,
}

impl AppNetworkPolicy {
    /// Grant one app access to one origin (e.g. `https://altru.dev`).
    pub fn allow(&mut self, app_id: &str, origin: &str) -> Result<(), MetalError> {
        validate_app_id(app_id)?;
        self.allowed
            .entry(app_id.to_string())
            .or_default()
            .insert(origin.to_ascii_lowercase());
        Ok(())
    }

    pub fn revoke_all(&mut self, app_id: &str) {
        self.allowed.remove(app_id);
    }

    pub fn is_allowed(&self, app_id: &str, origin: &str) -> bool {
        self.allowed
            .get(app_id)
            .is_some_and(|origins| origins.contains(&origin.to_ascii_lowercase()))
    }
}

impl NetworkProvider for AppNetworkPolicy {}

// ---------------------------------------------------------------------------
// Secrets: the PWX signer lives below the browser
// ---------------------------------------------------------------------------

/// Holds only the *location* of the signer service. There is deliberately no
/// method that returns key material, and nothing here is reachable from pages.
#[derive(Debug, Clone)]
pub struct PwxSignerSlot {
    pub signer_socket: PathBuf,
}

impl SecretProvider for PwxSignerSlot {}

// ---------------------------------------------------------------------------
// Remaining providers
// ---------------------------------------------------------------------------

pub struct MetalWindow {
    size: SurfaceSize,
}

impl WindowProvider for MetalWindow {
    fn surface_size(&self) -> SurfaceSize {
        self.size
    }
}

pub struct MetalGraphics;

impl GraphicsProvider for MetalGraphics {
    fn backend_name(&self) -> &'static str {
        "linux-fbdev-xrgb8888"
    }
}

/// Font lookup only checks that a family exists on disk. No shaping yet.
pub struct MetalFonts {
    pub font_dirs: Vec<PathBuf>,
}

impl FontProvider for MetalFonts {
    fn resolve_family(&self, family: &str) -> Option<String> {
        let wanted = family.to_ascii_lowercase().replace(' ', "");
        for dir in &self.font_dirs {
            let Ok(entries) = fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                if name.ends_with(".ttf") && name.replace(['-', '_', ' '], "").contains(&wanted) {
                    return Some(entry.path().to_string_lossy().into_owned());
                }
            }
        }
        None
    }
}

pub struct MetalUnavailable;
impl ClipboardProvider for MetalUnavailable {}
impl AccessibilityProvider for MetalUnavailable {}
impl MediaProvider for MetalUnavailable {}

pub struct MetalSandbox;
impl SandboxProvider for MetalSandbox {}

// ---------------------------------------------------------------------------
// The host
// ---------------------------------------------------------------------------

pub struct MetalHost {
    window: MetalWindow,
    graphics: MetalGraphics,
    fonts: MetalFonts,
    unavailable: MetalUnavailable,
    pub storage: AppVaultStorage,
    pub network: AppNetworkPolicy,
    pub signer: PwxSignerSlot,
    sandbox: MetalSandbox,
}

impl MetalHost {
    pub fn new(size: SurfaceSize, state_root: impl Into<PathBuf>) -> Self {
        let state_root = state_root.into();
        Self {
            window: MetalWindow { size },
            graphics: MetalGraphics,
            fonts: MetalFonts {
                font_dirs: vec![
                    PathBuf::from("/usr/share/fonts/truetype/dejavu"),
                    PathBuf::from("/usr/share/fonts/truetype"),
                ],
            },
            unavailable: MetalUnavailable,
            signer: PwxSignerSlot {
                signer_socket: state_root.join("signer.sock"),
            },
            storage: AppVaultStorage::new(state_root),
            network: AppNetworkPolicy::default(),
            sandbox: MetalSandbox,
        }
    }

    pub fn identity(&self) -> &'static str {
        METAL_HOST_IDENTITY
    }
}

impl PlatformHost for MetalHost {
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Linux
    }
    fn windowing(&self) -> &dyn WindowProvider {
        &self.window
    }
    fn graphics(&self) -> &dyn GraphicsProvider {
        &self.graphics
    }
    fn fonts(&self) -> &dyn FontProvider {
        &self.fonts
    }
    fn clipboard(&self) -> &dyn ClipboardProvider {
        &self.unavailable
    }
    fn accessibility(&self) -> &dyn AccessibilityProvider {
        &self.unavailable
    }
    fn storage(&self) -> &dyn StorageProvider {
        &self.storage
    }
    fn networking(&self) -> &dyn NetworkProvider {
        &self.network
    }
    fn media(&self) -> &dyn MediaProvider {
        &self.unavailable
    }
    fn secrets(&self) -> &dyn SecretProvider {
        &self.signer
    }
    fn sandbox(&self) -> &dyn SandboxProvider {
        &self.sandbox
    }
}

// ---------------------------------------------------------------------------
// Tests (no hardware needed)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_event(kind: u16, code: u16, value: i32) -> [u8; INPUT_EVENT_SIZE] {
        let mut bytes = [0u8; INPUT_EVENT_SIZE];
        bytes[16..18].copy_from_slice(&kind.to_le_bytes());
        bytes[18..20].copy_from_slice(&code.to_le_bytes());
        bytes[20..24].copy_from_slice(&value.to_le_bytes());
        bytes
    }

    #[test]
    fn fill_rect_clips_to_frame() {
        let mut frame = PixelFrame::new(4, 4, 0);
        frame.fill_rect(-10.0, 2.0, 100.0, 100.0, 7);
        assert_eq!(frame.pixels.iter().filter(|p| **p == 7).count(), 8);
        frame.fill_rect(f32::NAN, f32::NAN, 2.0, 2.0, 9);
        assert!(!frame.pixels.contains(&9), "NaN geometry must draw nothing");
    }

    #[test]
    fn renderer_is_deterministic_and_draws() {
        let scene = Scene {
            commands: vec![
                SceneCommand::Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 50.0,
                    height: 20.0,
                },
                SceneCommand::Text {
                    x: 2.0,
                    y: 2.0,
                    size_px: 16.0,
                    text: "Hi there".into(),
                },
            ],
            epoch: 1,
        };
        let renderer = MetalSceneRenderer::default();
        let a = renderer.render(&scene, 200, 100);
        assert_eq!(a, renderer.render(&scene, 200, 100));
        assert!(a.pixels.contains(&COLOR_BOX));
        assert!(a.pixels.contains(&COLOR_TEXT));
    }

    #[test]
    fn ppm_has_header_and_rgb_payload() {
        let ppm = PixelFrame::new(2, 1, 0x00FF_0000).to_ppm();
        assert!(ppm.starts_with(b"P6\n2 1\n255\n"));
        assert_eq!(&ppm[ppm.len() - 6..], &[255, 0, 0, 255, 0, 0]);
    }

    #[test]
    fn geometry_fails_closed() {
        let bad = FramebufferGeometry {
            width: 1600,
            height: 900,
            stride_bytes: 3200,
            bits_per_pixel: 16,
        };
        assert!(bad.validate().is_err());
        let narrow = FramebufferGeometry {
            bits_per_pixel: 32,
            ..bad
        };
        assert!(narrow.validate().is_err());
        let good = FramebufferGeometry {
            stride_bytes: 6400,
            ..narrow
        };
        assert!(good.validate().is_ok());
    }

    #[test]
    fn keymap_maps_shell_states() {
        let mut keymap = ShellKeymap::default();
        let press = |code| decode_input_event(&raw_event(EV_KEY, code, 1)).unwrap();
        assert_eq!(keymap.feed(press(KEY_F6)), Some(ShellIntent::Navigate));
        assert_eq!(keymap.feed(press(KEY_F12)), Some(ShellIntent::Inspect));
        assert_eq!(keymap.feed(press(KEY_ESC)), Some(ShellIntent::Focus));
        assert_eq!(keymap.feed(press(KEY_L)), None);
        keymap.feed(press(KEY_LEFTCTRL));
        assert_eq!(keymap.feed(press(KEY_L)), Some(ShellIntent::Navigate));
        assert_eq!(keymap.feed(press(KEY_Q)), Some(ShellIntent::Quit));
        assert!(decode_input_event(&raw_event(0x02, 0, 1)).is_none());
    }

    #[test]
    fn finds_keyboard_nodes() {
        let text = "I: Bus=0011\nN: Name=\"AT Translated Set 2 keyboard\"\nH: Handlers=sysrq kbd leds event3 \nB: EV=120013\n\n\
                    I: Bus=0011\nN: Name=\"SynPS/2 Synaptics TouchPad\"\nH: Handlers=mouse0 event5 \nB: EV=b\n";
        assert_eq!(
            keyboard_event_nodes(text),
            vec![PathBuf::from("/dev/input/event3")]
        );
    }

    #[test]
    fn app_ids_cannot_escape_the_vault() {
        for bad in ["", "..", "../etc", "a/b", ".hidden", "Upper", "a..b", "a b"] {
            assert!(validate_app_id(bad).is_err(), "{bad}");
        }
        assert!(validate_app_id("boxlog").is_ok());
        assert!(validate_app_id("dev.altru.parentpal-2").is_ok());
    }

    #[test]
    fn vault_is_created_private() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("altru-metal-test-{}", std::process::id()));
        let storage = AppVaultStorage::new(&root);
        let path = storage.open_vault("boxlog").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn network_is_denied_by_default() {
        let mut policy = AppNetworkPolicy::default();
        assert!(!policy.is_allowed("boxlog", "https://altru.dev"));
        policy.allow("boxlog", "https://Altru.dev").unwrap();
        assert!(policy.is_allowed("boxlog", "https://altru.dev"));
        assert!(!policy.is_allowed("parentpal", "https://altru.dev"));
        policy.revoke_all("boxlog");
        assert!(!policy.is_allowed("boxlog", "https://altru.dev"));
    }

    #[test]
    fn host_reports_identity() {
        let host = MetalHost::new(
            SurfaceSize {
                width: 1600,
                height: 900,
            },
            "/var/lib/altru",
        );
        assert_eq!(host.family(), PlatformFamily::Linux);
        assert_eq!(host.graphics().backend_name(), "linux-fbdev-xrgb8888");
        assert_eq!(host.windowing().surface_size().width, 1600);
        assert_eq!(host.identity(), METAL_HOST_IDENTITY);
    }
}
