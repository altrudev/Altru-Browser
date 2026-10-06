//! Explicit resource boundary for AWEF.
//!
//! Network/filesystem authority belongs to the host/broker, not the document
//! engine. Requests and responses are observable values.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    Document,
    StyleSheet,
    Script,
    Image,
    Font,
    Media,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRequest {
    pub request_id: u64,
    pub target: String,
    pub kind: ResourceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceResponse {
    pub request_id: u64,
    pub status: u16,
    pub media_type: String,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceError {
    Denied,
    NotFound,
    Transport(String),
}

pub trait ResourceBroker {
    fn fetch(&mut self, request: &ResourceRequest) -> Result<ResourceResponse, ResourceError>;
}

#[derive(Debug, Default)]
pub struct DenyAllResourceBroker;

impl ResourceBroker for DenyAllResourceBroker {
    fn fetch(&mut self, _request: &ResourceRequest) -> Result<ResourceResponse, ResourceError> {
        Err(ResourceError::Denied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_resource_boundary_has_no_ambient_authority() {
        let mut broker = DenyAllResourceBroker;
        let request = ResourceRequest {
            request_id: 1,
            target: "https://example.invalid/".into(),
            kind: ResourceKind::Document,
        };
        assert_eq!(broker.fetch(&request), Err(ResourceError::Denied));
    }
}
