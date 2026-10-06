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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeRuntimeError {
    Navigation(NavigationError),
    Resource(ResourceError),
    HttpStatus(u16),
    InvalidUtf8,
    Engine(NativeEngineError),
}

pub struct NativeRuntime<B: ResourceBroker> {
    kernel: BrowserKernel,
    engine: NativeEngineAdapter,
    broker: B,
    next_request_id: u64,
}

impl<B: ResourceBroker> NativeRuntime<B> {
    pub fn new(broker: B) -> Self {
        Self {
            kernel: BrowserKernel::default(),
            engine: NativeEngineAdapter,
            broker,
            next_request_id: 0,
        }
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
        let execution = self
            .engine
            .execute(source)
            .map_err(NativeRuntimeError::Engine)?;

        Ok(NativePage {
            navigation_id: navigation.id,
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
    fn deny_all_broker_blocks_page_load() {
        let mut runtime = NativeRuntime::new(crate::resource_api::DenyAllResourceBroker);
        assert_eq!(
            runtime.load("https://example.invalid/"),
            Err(NativeRuntimeError::Resource(ResourceError::Denied))
        );
    }
}
