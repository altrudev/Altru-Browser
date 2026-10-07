//! Altru Capability Intermediate Representation (ACIR).
//!
//! ACIR is the canonical semantic layer between foreign technology syntax
//! and Altru Browser's owned native primitives. It is intentionally small,
//! deterministic, and model-free.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityEnvironment {
    /// Effective resolution used for bounded CSS conditional evaluation.
    pub resolution_dpi: u32,
    /// Viewport width represented in thousandths of a CSS pixel.
    ///
    /// v1 defaults to a 1280 CSS px desktop compatibility snapshot.
    /// Live platform binding is a later, separately governed capability.
    pub viewport_width_milli_px: u32,
    /// Whether the user/environment requests reduced motion.
    pub prefers_reduced_motion: bool,
}

impl Default for CapabilityEnvironment {
    fn default() -> Self {
        Self {
            resolution_dpi: 96,
            viewport_width_milli_px: 1_280_000,
            prefers_reduced_motion: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentPredicate {
    MediaTypeScreen,
    MinResolutionDpi(u32),
    MinViewportWidthMilliPx(u32),
    MaxViewportWidthMilliPx(u32),
    PrefersReducedMotion,
    Not(Box<EnvironmentPredicate>),
    All(Vec<EnvironmentPredicate>),
}

impl EnvironmentPredicate {
    pub fn evaluate(&self, environment: &CapabilityEnvironment) -> bool {
        match self {
            Self::MediaTypeScreen => true,
            Self::MinResolutionDpi(required) => environment.resolution_dpi >= *required,
            Self::MinViewportWidthMilliPx(required) => {
                environment.viewport_width_milli_px >= *required
            }
            Self::MaxViewportWidthMilliPx(required) => {
                environment.viewport_width_milli_px <= *required
            }
            Self::PrefersReducedMotion => environment.prefers_reduced_motion,
            Self::Not(inner) => !inner.evaluate(environment),
            Self::All(predicates) => predicates.iter().all(|predicate| predicate.evaluate(environment)),
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
