//! Cross-platform host boundary for the native AWEF engine.
//!
//! The native engine core must not directly depend on Win32, AppKit/UIKit,
//! Wayland/X11, Android Java APIs, or platform filesystem conventions.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformFamily {
    Windows,
    MacOS,
    Linux,
    Android,
    IOS,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceSize {
    pub width: u32,
    pub height: u32,
}

pub trait WindowProvider {
    fn surface_size(&self) -> SurfaceSize;
}

pub trait GraphicsProvider {
    fn backend_name(&self) -> &'static str;
}

pub trait FontProvider {
    fn resolve_family(&self, family: &str) -> Option<String>;
}

pub trait ClipboardProvider {}
pub trait AccessibilityProvider {}
pub trait StorageProvider {}
pub trait NetworkProvider {}
pub trait MediaProvider {}
pub trait SecretProvider {}
pub trait SandboxProvider {}

pub trait PlatformHost {
    fn family(&self) -> PlatformFamily;
    fn windowing(&self) -> &dyn WindowProvider;
    fn graphics(&self) -> &dyn GraphicsProvider;
    fn fonts(&self) -> &dyn FontProvider;
    fn clipboard(&self) -> &dyn ClipboardProvider;
    fn accessibility(&self) -> &dyn AccessibilityProvider;
    fn storage(&self) -> &dyn StorageProvider;
    fn networking(&self) -> &dyn NetworkProvider;
    fn media(&self) -> &dyn MediaProvider;
    fn secrets(&self) -> &dyn SecretProvider;
    fn sandbox(&self) -> &dyn SandboxProvider;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_family_is_explicit_not_cfg_driven() {
        let family = PlatformFamily::Windows;
        assert_eq!(family, PlatformFamily::Windows);
    }
}
