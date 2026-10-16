//! AWEF-owned native browser runtime.
//!
//! This connects navigation, explicit resource acquisition and native document
//! execution without granting the engine ambient network/filesystem authority.

use crate::browser_kernel::{BrowserKernel, NavigationError};
use crate::engine_api::EngineAdapter;
use crate::native_engine::{NativeEngineAdapter, NativeEngineError, NativeExecution};
#[cfg(feature = "desktop-preview")]
use crate::native_engine::execute_native_document_with_external_css;
use crate::resource_api::{ResourceBroker, ResourceError, ResourceKind, ResourceRequest};

#[derive(Debug, Clone, PartialEq)]
pub struct NativePage {
    #[cfg(feature = "desktop-preview")]
    pub images: std::collections::BTreeMap<String, NativeDecodedImage>,
    #[cfg(feature = "desktop-preview")]
    pub failed_images: usize,
    #[cfg(feature = "desktop-preview")]
    pub image_failure_reasons: Vec<String>,
    pub navigation_id: u64,
    pub target: String,
    pub execution: NativeExecution,
}

#[cfg(feature = "desktop-preview")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeDecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[cfg(feature = "desktop-preview")]
fn decode_native_image(bytes: &[u8], mime: &str) -> Option<NativeDecodedImage> {
    let format = match mime.split(';').next()?.trim().to_ascii_lowercase().as_str() {
        "image/png" => image::ImageFormat::Png,
        "image/jpeg" => image::ImageFormat::Jpeg,
        "image/webp" => image::ImageFormat::WebP,
        "image/gif" => image::ImageFormat::Gif,
        _ => return None,
    };
    if bytes.len() > 2_000_000 { return None; }
    let mut reader = image::ImageReader::with_format(
        std::io::Cursor::new(bytes), format,
    );
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(16 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode().ok()?;
    if decoded.width() == 0 || decoded.height() == 0 ||
        decoded.width() > 2048 || decoded.height() > 2048 ||
        u64::from(decoded.width()) * u64::from(decoded.height()) > 4_000_000 {
        return None;
    }
    Some(NativeDecodedImage {
        width: decoded.width(), height: decoded.height(),
        rgba: decoded.into_rgba8().into_raw(),
    })
}

/// No alternate renderer is executed unless explicitly permitted and actually wired.
/// Permission alone never implies that a fallback engine is available or trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FallbackPolicy {
    #[default]
    Deny,
    PermitVerifiedServo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeRuntimeError {
    Navigation(NavigationError),
    Resource(ResourceError),
    HttpStatus(u16),
    InvalidUtf8,
    Engine(NativeEngineError),
    /// Native execution was attempted first; fallback was authorized but is not wired.
    FallbackUnavailable(NativeEngineError),
}

pub struct NativeRuntime<B: ResourceBroker> {
    kernel: BrowserKernel,
    engine: NativeEngineAdapter,
    broker: B,
    next_request_id: u64,
    fallback_policy: FallbackPolicy,
    external_css_enabled: bool,
    images_enabled: bool,
}

impl<B: ResourceBroker> NativeRuntime<B> {
    pub fn new(broker: B) -> Self {
        Self {
            kernel: BrowserKernel::default(),
            engine: NativeEngineAdapter,
            broker,
            next_request_id: 0,
            fallback_policy: FallbackPolicy::Deny,
            external_css_enabled: false,
            images_enabled: false,
        }
    }

    /// Explicit opt-in: externally fetched CSS must pass the broker's authorization.
    /// Disabled by default until native visual acceptance succeeds.
    pub fn set_images_enabled(&mut self, enabled: bool) {
        self.images_enabled = enabled;
    }

    pub fn set_external_css_enabled(&mut self, enabled: bool) {
        self.external_css_enabled = enabled;
    }

    pub fn set_fallback_policy(&mut self, policy: FallbackPolicy) {
        self.fallback_policy = policy;
    }

    pub fn fallback_policy(&self) -> FallbackPolicy {
        self.fallback_policy
    }

    pub fn kernel(&self) -> &BrowserKernel {
        &self.kernel
    }

    pub fn load(&mut self, target: impl Into<String>) -> Result<NativePage, NativeRuntimeError> {
        let target = target.into();
        let navigation = self
            .kernel
            .navigate(target.clone())
            .map_err(NativeRuntimeError::Navigation)?
            .clone();
        self.fetch_navigation(navigation.id, target)
    }

    pub fn reload(&mut self) -> Result<Option<NativePage>, NativeRuntimeError> {
        let Some(navigation) = self.kernel.current().cloned() else {
            return Ok(None);
        };
        self.fetch_navigation(navigation.id, navigation.target)
            .map(Some)
    }

    pub fn back(&mut self) -> Result<Option<NativePage>, NativeRuntimeError> {
        let Some(navigation) = self.kernel.back().cloned() else {
            return Ok(None);
        };
        self.fetch_navigation(navigation.id, navigation.target)
            .map(Some)
    }

    pub fn forward(&mut self) -> Result<Option<NativePage>, NativeRuntimeError> {
        let Some(navigation) = self.kernel.forward().cloned() else {
            return Ok(None);
        };
        self.fetch_navigation(navigation.id, navigation.target)
            .map(Some)
    }

    fn fetch_navigation(
        &mut self,
        navigation_id: u64,
        target: String,
    ) -> Result<NativePage, NativeRuntimeError> {
        self.next_request_id = self.next_request_id.saturating_add(1);
        let request = ResourceRequest {
            request_id: self.next_request_id,
            target: target.clone(),
            kind: ResourceKind::Document,
        };

        let response = self
            .broker
            .fetch(&request)
            .map_err(NativeRuntimeError::Resource)?;

        if !(200..300).contains(&response.status) {
            return Err(NativeRuntimeError::HttpStatus(response.status));
        }

        let source =
            String::from_utf8(response.body).map_err(|_| NativeRuntimeError::InvalidUtf8)?;
        #[cfg(feature = "desktop-preview")]
        let external_css = if self.external_css_enabled {
            let document = crate::native_html::parse_document(&source)
                .map_err(|error| NativeRuntimeError::Engine(
                    NativeEngineError::from(error)))?;
            let mut css = String::new();
            let mut seen = std::collections::BTreeSet::new();
            let base = url::Url::parse(&target)
                .map_err(|_| NativeRuntimeError::Resource(ResourceError::Denied))?;
            let dependencies = crate::native_dependencies::discover_dependencies(&document);
            if base.scheme() != "https" &&
                dependencies.iter().any(|item| item.kind == ResourceKind::StyleSheet) {
                return Err(NativeRuntimeError::Resource(ResourceError::Denied));
            }
            for dependency in dependencies {
                if dependency.kind != ResourceKind::StyleSheet { continue; }
                let resolved = base.join(&dependency.reference)
                    .map_err(|_| NativeRuntimeError::Resource(ResourceError::Denied))?;
                // Classic stylesheets may be loaded in no-CORS mode across origins.
                // Stylesheet links explicitly requesting CORS require response-header
                // validation, which this broker interface cannot yet evidence.
                let unsafe_host = resolved.host_str().is_some_and(|host| {
                    host.eq_ignore_ascii_case("localhost")
                        || host.ends_with(".localhost")
                        || host.parse::<std::net::IpAddr>().is_ok_and(|ip| match ip {
                            std::net::IpAddr::V4(v4) =>
                                v4.is_private() || v4.is_loopback() || v4.is_link_local()
                                || v4.is_broadcast() || v4.is_unspecified() ,
                            std::net::IpAddr::V6(v6) =>
                                v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local()
                                || v6.is_unspecified(),
                        })
                });
                if resolved.scheme() != "https" || dependency.cors_required || unsafe_host
                    || !resolved.username().is_empty() || resolved.password().is_some()
                {
                    return Err(NativeRuntimeError::Resource(ResourceError::Denied));
                }
                if !seen.insert(resolved.to_string()) {continue;}
                if seen.len() > 32 {return Err(NativeRuntimeError::Resource(ResourceError::Denied));}
                self.next_request_id = self.next_request_id.saturating_add(1);
                let response = self.broker.fetch(&ResourceRequest {
                    request_id: self.next_request_id,
                    target: resolved.into(),
                    kind: ResourceKind::StyleSheet,
                }).map_err(NativeRuntimeError::Resource)?;
                if !(200..300).contains(&response.status) {
                    return Err(NativeRuntimeError::HttpStatus(response.status));
                }
                if !response.media_type.to_ascii_lowercase().starts_with("text/css")
                    || response.body.len() > 1_000_000 || css.len() + response.body.len() > 4_000_000
                {
                    return Err(NativeRuntimeError::Resource(ResourceError::Denied));
                }
                css.push_str(&String::from_utf8(response.body)
                    .map_err(|_| NativeRuntimeError::InvalidUtf8)?);
                css.push('\n');
            }
            css
        } else { String::new() };
        #[cfg(feature = "desktop-preview")]
        let result = if self.external_css_enabled {
            execute_native_document_with_external_css(&source, &external_css)
        } else {
            self.engine.execute(source)
        };
        #[cfg(not(feature = "desktop-preview"))]
        let result = self.engine.execute(source);
        let execution = result.map_err(|error| {
            match (&self.fallback_policy, &error) {
                (FallbackPolicy::PermitVerifiedServo, NativeEngineError::Css(crate::native_css::CssError::UnsupportedSelector(_))) => {
                    // No verified Servo navigator is wired. Never claim success or
                    // pass the document to another engine implicitly.
                    NativeRuntimeError::FallbackUnavailable(error)
                }
                _ => NativeRuntimeError::Engine(error),
            }
        })?;

        #[cfg(feature = "desktop-preview")]
        let (images, image_failure_reasons) = if self.images_enabled && target.starts_with("https://") {
            let mut images = std::collections::BTreeMap::new();
            let mut failures = Vec::new();
            let mut seen = std::collections::BTreeSet::new();
            let base = url::Url::parse(&target)
                .map_err(|_| NativeRuntimeError::Resource(ResourceError::Denied))?;
            for dependency in &execution.dependencies {
                if dependency.kind != ResourceKind::Image { continue; }
                if seen.len() >= 12 {break;}
                let Ok(resolved) = base.join(&dependency.reference) else {
                    failures.push(format!("{}: invalid URL", dependency.reference));
                    continue;
                };
                if resolved.scheme() != "https" || !resolved.username().is_empty()
                    || resolved.password().is_some()
                {
                    failures.push(format!("{}: disallowed URL", dependency.reference));
                    continue;
                }
                if !seen.insert(resolved.to_string()) { continue; }
                self.next_request_id = self.next_request_id.saturating_add(1);
                let response = self.broker.fetch(&ResourceRequest {
                    request_id: self.next_request_id,
                    target: resolved.into(),
                    kind: ResourceKind::Image,
                });
                match response {
                    Ok(response) if (200..300).contains(&response.status) => {
                        if let Some(image) = decode_native_image(&response.body, &response.media_type) {
                            images.insert(dependency.reference.clone(), image);
                            continue;
                        }
                        failures.push(format!("{}: unsupported/invalid {} ({} bytes)",
                            dependency.reference, response.media_type, response.body.len()));
                    }
                    Ok(response) => failures.push(format!("{}: HTTP {}",
                        dependency.reference, response.status)),
                    Err(error) => failures.push(format!("{}: {error:?}",
                        dependency.reference)),
                }
            }
            (images, failures)
        } else { (std::collections::BTreeMap::new(), Vec::new()) };

        Ok(NativePage {
            #[cfg(feature = "desktop-preview")]
            images,
            #[cfg(feature = "desktop-preview")]
            failed_images: image_failure_reasons.len(),
            #[cfg(feature = "desktop-preview")]
            image_failure_reasons,
            navigation_id,
            target,
            execution,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_api::{ResourceError, ResourceResponse};

    struct FixtureBroker;

    impl ResourceBroker for FixtureBroker {
        fn fetch(&mut self, request: &ResourceRequest) -> Result<ResourceResponse, ResourceError> {
            Ok(ResourceResponse {
                request_id: request.request_id,
                status: 200,
                media_type: "text/html".into(),
                body: b"<html><body><p>Runtime fixture</p></body></html>".to_vec(),
            })
        }
    }

    #[cfg(feature = "desktop-preview")]
    #[test]
    fn bounded_gif_decoder_accepts_first_frame() {
        let mut encoded = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(
            image::RgbaImage::from_pixel(3, 2, image::Rgba([45, 90, 135, 255])),
        ).write_to(&mut encoded, image::ImageFormat::Gif).unwrap();
        let decoded = decode_native_image(&encoded.into_inner(), "image/gif").unwrap();
        assert_eq!((decoded.width, decoded.height), (3, 2));
        assert_eq!(decoded.rgba.len(), 3 * 2 * 4);
    }

    #[cfg(feature = "desktop-preview")]
    struct ImageFixtureBroker;

    #[cfg(feature = "desktop-preview")]
    impl ResourceBroker for ImageFixtureBroker {
        fn fetch(&mut self, request: &ResourceRequest) -> Result<ResourceResponse, ResourceError> {
            let (media_type, body) = match request.kind {
                ResourceKind::Document => ("text/html", b"<html><body><img src='https://assets.example.org/p.png' width='40' height='30'></body></html>".to_vec()),
                ResourceKind::Image if request.target == "https://assets.example.org/p.png" => {
                    let mut encoded = std::io::Cursor::new(Vec::new());
                    image::DynamicImage::ImageRgba8(
                        image::RgbaImage::from_pixel(2, 3, image::Rgba([23, 45, 67, 255]))
                    ).write_to(&mut encoded, image::ImageFormat::Png)
                        .map_err(|e| ResourceError::Transport(e.to_string()))?;
                    ("image/png", encoded.into_inner())
                },
                _ => return Err(ResourceError::Denied),
            };
            Ok(ResourceResponse {
                request_id: request.request_id,
                status: 200, media_type: media_type.into(), body,
            })
        }
    }

    #[cfg(feature = "desktop-preview")]
    #[test]
    fn native_image_full_path_fetch_decodes_and_emits_scene_item() {
        let mut runtime = NativeRuntime::new(ImageFixtureBroker);
        runtime.set_images_enabled(true);
        let page = runtime.load("https://example.test/").unwrap();
        let image = page.images.get("https://assets.example.org/p.png").unwrap();
        assert_eq!((image.width, image.height), (2, 3));
        assert_eq!(&image.rgba[..4], &[23, 45, 67, 255]);
        assert!(page.execution.scene.commands.iter().any(|command|
            matches!(command, crate::native_scene::SceneCommand::Image {
                width, height, src, ..
            } if *width == 40.0 && *height == 30.0
                && src == "https://assets.example.org/p.png")));
        assert_eq!(page.failed_images, 0);
    }

    #[cfg(feature = "desktop-preview")]
    struct CssFixtureBroker {
        cross_origin: bool,
        cors_required: bool,
    }

    #[cfg(feature = "desktop-preview")]
    impl ResourceBroker for CssFixtureBroker {
        fn fetch(&mut self, request: &ResourceRequest) -> Result<ResourceResponse, ResourceError> {
            let (media_type, body) = match request.kind {
                ResourceKind::Document => (
                    "text/html", if self.cross_origin && self.cors_required {
                        "<html><head><link rel='stylesheet' crossorigin='anonymous' href='https://assets.example.org/style.css'></head><body><p>Fixture</p></body></html>"
                    } else if self.cross_origin {
                        "<html><head><link rel='stylesheet' href='https://assets.example.org/style.css'></head><body><p>Fixture</p></body></html>"
                    } else {
                        "<html><head><link rel='stylesheet' href='/style.css'></head><body><p>Fixture</p></body></html>"
                    },
                ),
                ResourceKind::StyleSheet if request.target == "https://example.test/style.css"
                    || request.target == "https://assets.example.org/style.css" =>
                    ("text/css", "p { font-size: 27px; }"),
                _ => return Err(ResourceError::Denied),
            };
            Ok(ResourceResponse {
                request_id: request.request_id,
                status: 200,
                media_type: media_type.into(),
                body: body.as_bytes().to_vec(),
            })
        }
    }

    #[cfg(feature = "desktop-preview")]
    #[test]
    fn broker_css_changes_native_frame_and_blocks_cross_origin() {
        let mut enabled = NativeRuntime::new(CssFixtureBroker { cross_origin: false, cors_required: false });
        enabled.set_external_css_enabled(true);
        let styled = enabled.load("https://example.test/").unwrap();
        assert!(styled.execution.scene.commands.iter().any(|command|
            matches!(command, crate::native_scene::SceneCommand::Text { size_px, text, .. }
                if text == "Fixture" && (*size_px - 27.0).abs() < 0.001)));

        let mut disabled = NativeRuntime::new(CssFixtureBroker { cross_origin: false, cors_required: false });
        let plain = disabled.load("https://example.test/").unwrap();
        assert_ne!(styled.execution.evidence.execution_sha256,
            plain.execution.evidence.execution_sha256);

        let mut cross_origin = NativeRuntime::new(CssFixtureBroker { cross_origin: true, cors_required: false });
        cross_origin.set_external_css_enabled(true);
        let cross = cross_origin.load("https://example.test/").unwrap();
        assert!(cross.execution.scene.commands.iter().any(|command|
            matches!(command, crate::native_scene::SceneCommand::Text { size_px, text, .. }
                if text == "Fixture" && (*size_px - 27.0).abs() < 0.001)));

        let mut denied = NativeRuntime::new(CssFixtureBroker { cross_origin: true, cors_required: true });
        denied.set_external_css_enabled(true);
        assert!(matches!(denied.load("https://example.test/"),
            Err(NativeRuntimeError::Resource(ResourceError::Denied))));
    }

    #[test]
    fn runtime_loads_only_through_explicit_broker() {
        let mut runtime = NativeRuntime::new(FixtureBroker);
        let page = runtime.load("awef://fixture").unwrap();
        assert_eq!(page.navigation_id, 1);
        assert!(page.execution.artifact.contains("Runtime fixture"));
        assert_eq!(runtime.kernel().history().len(), 1);
    }

    #[test]
    fn back_forward_and_reload_reuse_existing_history_entries() {
        let mut runtime = NativeRuntime::new(FixtureBroker);
        let first = runtime.load("awef://one").unwrap();
        let second = runtime.load("awef://two").unwrap();
        assert_eq!(runtime.kernel().history().len(), 2);

        let back = runtime.back().unwrap().unwrap();
        assert_eq!(back.navigation_id, first.navigation_id);
        assert_eq!(back.target, "awef://one");
        assert_eq!(runtime.kernel().history().len(), 2);

        let forward = runtime.forward().unwrap().unwrap();
        assert_eq!(forward.navigation_id, second.navigation_id);
        assert_eq!(forward.target, "awef://two");

        let reloaded = runtime.reload().unwrap().unwrap();
        assert_eq!(reloaded.navigation_id, second.navigation_id);
        assert_eq!(runtime.kernel().history().len(), 2);
    }

    struct UnsupportedCssBroker;

    impl ResourceBroker for UnsupportedCssBroker {
        fn fetch(&mut self, request: &ResourceRequest) -> Result<ResourceResponse, ResourceError> {
            Ok(ResourceResponse {
                request_id: request.request_id,
                status: 200,
                media_type: "text/html".into(),
                body: b"<html><style>p:never-supported(42) { color: red; }</style><body><p>Denied</p></body></html>".to_vec(),
            })
        }
    }

    #[test]
    fn native_default_does_not_invoke_fallback() {
        let mut runtime = NativeRuntime::new(UnsupportedCssBroker);
        assert_eq!(runtime.fallback_policy(), FallbackPolicy::Deny);
        assert!(matches!(runtime.load("awef://unsupported"), Err(NativeRuntimeError::Engine(NativeEngineError::Css(_)))));
    }

    #[test]
    fn authorized_but_unwired_fallback_still_fails_closed() {
        let mut runtime = NativeRuntime::new(UnsupportedCssBroker);
        runtime.set_fallback_policy(FallbackPolicy::PermitVerifiedServo);
        assert!(matches!(runtime.load("awef://unsupported"), Err(NativeRuntimeError::FallbackUnavailable(NativeEngineError::Css(_)))));
    }

    #[test]
    fn deny_all_broker_blocks_page_load() {
        let mut runtime = NativeRuntime::new(crate::resource_api::DenyAllResourceBroker);
        assert_eq!(
            runtime.load("https://example.invalid/"),
            Err(NativeRuntimeError::Resource(ResourceError::Denied))
        );
    }
}
