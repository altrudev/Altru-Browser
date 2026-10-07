//! Altru Capability Intermediate Representation (ACIR).
//!
//! ACIR is intentionally smaller than the source technologies translated into it.
//! Source syntax is lowered into deterministic semantic predicates before the native
//! engine consumes it.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcirComparison {
    AtLeast,
    AtMost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcirEnvironmentFeature {
    ResolutionMilliDpi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcirEnvironmentPredicate {
    pub feature: AcirEnvironmentFeature,
    pub comparison: AcirComparison,
    pub value: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityEnvironment {
    /// Physical/logical rendering resolution represented in milli-DPI.
    pub resolution_milli_dpi: u32,
}

impl Default for CapabilityEnvironment {
    fn default() -> Self {
        Self {
            resolution_milli_dpi: 96_000,
        }
    }
}

impl CapabilityEnvironment {
    pub const fn new(resolution_milli_dpi: u32) -> Self {
        Self {
            resolution_milli_dpi,
        }
    }

    pub fn evaluate(self, predicate: AcirEnvironmentPredicate) -> bool {
        let observed = match predicate.feature {
            AcirEnvironmentFeature::ResolutionMilliDpi => self.resolution_milli_dpi,
        };
        match predicate.comparison {
            AcirComparison::AtLeast => observed >= predicate.value,
            AcirComparison::AtMost => observed <= predicate.value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_predicates_are_deterministic() {
        let environment = CapabilityEnvironment::new(96_000);
        assert!(!environment.evaluate(AcirEnvironmentPredicate {
            feature: AcirEnvironmentFeature::ResolutionMilliDpi,
            comparison: AcirComparison::AtLeast,
            value: 192_000,
        }));
        assert!(environment.evaluate(AcirEnvironmentPredicate {
            feature: AcirEnvironmentFeature::ResolutionMilliDpi,
            comparison: AcirComparison::AtMost,
            value: 192_000,
        }));
    }
}
