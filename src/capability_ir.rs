//! Altru Capability Intermediate Representation (ACIR).
//!
//! ACIR is intentionally smaller than the source technologies translated into it.
//! Source syntax is lowered into deterministic semantic conditions before the native
//! engine consumes it.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcirComparison {
    AtLeast,
    AtMost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcirEnvironmentFeature {
    ResolutionMilliDpi,
    ViewportWidthMilliPx,
    PrefersReducedMotionFlag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcirEnvironmentPredicate {
    pub feature: AcirEnvironmentFeature,
    pub comparison: AcirComparison,
    pub value: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcirMediaType {
    Screen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcirSelectorRelation {
    Descendant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcirEnvironmentCondition {
    pub media_type: Option<AcirMediaType>,
    pub predicates: Vec<AcirEnvironmentPredicate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityEnvironment {
    /// Resolution represented in milli-DPI.
    pub resolution_milli_dpi: u32,
    /// Layout viewport width represented in milli-CSS-pixels.
    pub viewport_width_milli_px: u32,
    pub prefers_reduced_motion: bool,
    pub media_type: AcirMediaType,
}

impl Default for CapabilityEnvironment {
    fn default() -> Self {
        Self {
            resolution_milli_dpi: 96_000,
            viewport_width_milli_px: 800_000,
            prefers_reduced_motion: false,
            media_type: AcirMediaType::Screen,
        }
    }
}

impl CapabilityEnvironment {
    pub const fn new(
        resolution_milli_dpi: u32,
        viewport_width_milli_px: u32,
        prefers_reduced_motion: bool,
        media_type: AcirMediaType,
    ) -> Self {
        Self {
            resolution_milli_dpi,
            viewport_width_milli_px,
            prefers_reduced_motion,
            media_type,
        }
    }

    pub const fn desktop(viewport_width_px: u32) -> Self {
        Self {
            resolution_milli_dpi: 96_000,
            viewport_width_milli_px: viewport_width_px.saturating_mul(1_000),
            prefers_reduced_motion: false,
            media_type: AcirMediaType::Screen,
        }
    }

    pub fn evaluate_predicate(self, predicate: AcirEnvironmentPredicate) -> bool {
        let observed = match predicate.feature {
            AcirEnvironmentFeature::ResolutionMilliDpi => self.resolution_milli_dpi,
            AcirEnvironmentFeature::ViewportWidthMilliPx => self.viewport_width_milli_px,
            AcirEnvironmentFeature::PrefersReducedMotionFlag => {
                u32::from(self.prefers_reduced_motion)
            }
        };
        match predicate.comparison {
            AcirComparison::AtLeast => observed >= predicate.value,
            AcirComparison::AtMost => observed <= predicate.value,
        }
    }

    pub fn evaluate_condition(self, condition: &AcirEnvironmentCondition) -> bool {
        if let Some(media_type) = condition.media_type
            && media_type != self.media_type
        {
            return false;
        }
        condition
            .predicates
            .iter()
            .copied()
            .all(|predicate| self.evaluate_predicate(predicate))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_conditions_combine_media_width_and_preferences() {
        let environment = CapabilityEnvironment::desktop(800);
        let condition = AcirEnvironmentCondition {
            media_type: Some(AcirMediaType::Screen),
            predicates: vec![
                AcirEnvironmentPredicate {
                    feature: AcirEnvironmentFeature::ViewportWidthMilliPx,
                    comparison: AcirComparison::AtMost,
                    value: 991_980,
                },
                AcirEnvironmentPredicate {
                    feature: AcirEnvironmentFeature::PrefersReducedMotionFlag,
                    comparison: AcirComparison::AtMost,
                    value: 0,
                },
            ],
        };
        assert!(environment.evaluate_condition(&condition));
    }

    #[test]
    fn resolution_condition_is_deterministic() {
        let environment = CapabilityEnvironment::desktop(800);
        assert!(!environment.evaluate_predicate(AcirEnvironmentPredicate {
            feature: AcirEnvironmentFeature::ResolutionMilliDpi,
            comparison: AcirComparison::AtLeast,
            value: 192_000,
        }));
    }
}
