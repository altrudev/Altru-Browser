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
    pub navigation_id: u64,
    pub target: String,
    pub execution: NativeExecution,
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
        }
    }

    /// Explicit opt-in: externally fetched CSS must pass the broker's authorization.
    /// Disabled by default until native visual acceptance succeeds.
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
            if base.scheme() != "https" {
                return Err(NativeRuntimeError::Resource(ResourceError::Denied));
            }
            for dependency in crate::native_dependencies::discover_dependencies(&document) {
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

        Ok(NativePage {
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
