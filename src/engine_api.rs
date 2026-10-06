//! Stable AWEF-owned engine boundary.
//!
//! Implementations may come and go. These contracts remain authoritative.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineCapability {
    HtmlDocument,
    CssCascade,
    BlockLayout,
    InlineLayout,
    FlexLayout,
    GridLayout,
    Script,
    Events,
    Fetch,
    Storage,
    Canvas,
    Media,
    Accessibility,
    OffscreenRender,
    SuspendResume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromotionState {
    Experiment,
    SimulateFirst,
    OracleCompared,
    ConformancePartial,
    PlatformVerified,
    Promoted,
    Demoted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformStatus {
    Planned,
    CompileVerified,
    RuntimeVerified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformClaim {
    pub platform: String,
    pub status: PlatformStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineManifest {
    pub implementation: String,
    pub version: String,
    pub capabilities: Vec<EngineCapability>,
    pub platform_support: Vec<PlatformClaim>,
    pub promotion: PromotionState,
    pub provenance: String,
}

impl EngineManifest {
    pub fn supports(&self, capability: EngineCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    pub fn production_eligible(&self) -> bool {
        self.promotion == PromotionState::Promoted
    }

    pub fn platform_status(&self, platform: &str) -> Option<PlatformStatus> {
        self.platform_support
            .iter()
            .find(|claim| claim.platform == platform)
            .map(|claim| claim.status)
    }
}

pub trait EngineAdapter {
    type Request;
    type Response;
    type Error;

    fn manifest(&self) -> EngineManifest;
    fn execute(&mut self, request: Self::Request) -> Result<Self::Response, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_conformance_is_not_production_promotion() {
        let manifest = EngineManifest {
            implementation: "awef-native".into(),
            version: "n1".into(),
            capabilities: vec![EngineCapability::HtmlDocument],
            platform_support: vec![PlatformClaim {
                platform: "linux".into(),
                status: PlatformStatus::RuntimeVerified,
            }],
            promotion: PromotionState::ConformancePartial,
            provenance: "AWEF".into(),
        };
        assert!(manifest.supports(EngineCapability::HtmlDocument));
        assert_eq!(
            manifest.platform_status("linux"),
            Some(PlatformStatus::RuntimeVerified)
        );
        assert!(!manifest.production_eligible());
    }
}
