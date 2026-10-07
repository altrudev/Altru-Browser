//! Altru Capability Intermediate Representation (ACIR).
//!
//! ACIR is the canonical semantic layer between foreign technology syntax
//! and Altru Browser's owned native primitives. It is intentionally small,
//! deterministic, and model-free.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityEnvironment {
    /// Effective resolution used for bounded CSS conditional evaluation.
    ///
    /// v1 intentionally defaults to CSS reference resolution (96 dpi).
    /// Platform scale binding is a later, separately governed capability.
    pub resolution_dpi: u32,
}

impl Default for CapabilityEnvironment {
    fn default() -> Self {
        Self { resolution_dpi: 96 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentPredicate {
    MinResolutionDpi(u32),
}

impl EnvironmentPredicate {
    pub fn evaluate(&self, environment: &CapabilityEnvironment) -> bool {
        match self {
            Self::MinResolutionDpi(required) => environment.resolution_dpi >= *required,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityNode {
    Conditional {
        predicate: EnvironmentPredicate,
        source_construct: String,
        body: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslationDecision {
    Included,
    Excluded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationReceipt {
    pub source_construct: String,
    pub target_primitive: String,
    pub decision: TranslationDecision,
    /// v1 translations are exact only for the explicitly supported bounded syntax.
    pub exact: bool,
}
