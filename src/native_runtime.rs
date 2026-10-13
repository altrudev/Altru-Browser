//! AWEF-owned native browser runtime.
//!
//! This connects navigation, explicit resource acquisition and native document
//! execution without granting the engine ambient network/filesystem authority.

use crate::browser_kernel::{BrowserKernel, NavigationError};
use crate::engine_api::EngineAdapter;
use crate::native_engine::{NativeEngineAdapter, NativeEngineError, NativeExecution};
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
}

impl<B: ResourceBroker> NativeRuntime<B> {
    pub fn new(broker: B) -> Self {
        Self {
            kernel: BrowserKernel::default(),
            engine: NativeEngineAdapter,
            broker,
            next_request_id: 0,
            fallback_policy: FallbackPolicy::Deny,
        }
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
        let execution = self.engine.execute(source).map_err(|error| {
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
