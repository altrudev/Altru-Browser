//! Altru Capability Intermediate Representation (ACIR).
//!
//! ACIR is a small semantic substrate used to translate external technology
//! into owned Altru Browser capabilities. It is deliberately deterministic:
//! translation is not authority, and unsupported semantics remain fail-closed.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvironmentSnapshot {
    pub viewport_width_px: u32,
    pub resolution_dpi: u32,
    pub prefers_reduced_motion: bool,
}

impl EnvironmentSnapshot {
    pub const fn desktop_preview() -> Self {
        Self {
            viewport_width_px: 800,
            resolution_dpi: 96,
            prefers_reduced_motion: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvironmentMetric {
    ViewportWidthPx,
    ResolutionDpi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparison {
    AtLeast,
    AtMost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvironmentPredicate {
    pub metric: EnvironmentMetric,
    pub comparison: Comparison,
    pub threshold: u32,
}

impl EnvironmentPredicate {
    pub fn evaluate(&self, environment: EnvironmentSnapshot) -> bool {
        let actual = match self.metric {
            EnvironmentMetric::ViewportWidthPx => environment.viewport_width_px,
            EnvironmentMetric::ResolutionDpi => environment.resolution_dpi,
        };

        match self.comparison {
            Comparison::AtLeast => actual >= self.threshold,
            Comparison::AtMost => actual <= self.threshold,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    Environment(EnvironmentPredicate),
    PrefersReducedMotion,
    Not(Box<Condition>),
    All(Vec<Condition>),
}

impl Condition {
    pub fn evaluate(&self, environment: EnvironmentSnapshot) -> bool {
        match self {
            Self::Environment(predicate) => predicate.evaluate(environment),
            Self::PrefersReducedMotion => environment.prefers_reduced_motion,
            Self::Not(inner) => !inner.evaluate(environment),
            Self::All(conditions) => conditions
                .iter()
                .all(|condition| condition.evaluate(environment)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationRecord {
    pub translator_id: &'static str,
    pub source_domain: &'static str,
    pub target_domain: &'static str,
    pub exact_for_bounded_subset: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_predicates_are_deterministic() {
        let environment = EnvironmentSnapshot {
            viewport_width_px: 1280,
            resolution_dpi: 144,
            prefers_reduced_motion: false,
        };
        assert!(
            EnvironmentPredicate {
                metric: EnvironmentMetric::ResolutionDpi,
                comparison: Comparison::AtLeast,
                threshold: 120,
            }
            .evaluate(environment)
        );
        assert!(
            EnvironmentPredicate {
                metric: EnvironmentMetric::ViewportWidthPx,
                comparison: Comparison::AtMost,
                threshold: 1440,
            }
            .evaluate(environment)
        );
    }

    #[test]
    fn boolean_conditions_compose_without_new_runtime_authority() {
        let environment = EnvironmentSnapshot::desktop_preview();
        let condition = Condition::All(vec![
            Condition::Environment(EnvironmentPredicate {
                metric: EnvironmentMetric::ViewportWidthPx,
                comparison: Comparison::AtLeast,
                threshold: 768,
            }),
            Condition::Environment(EnvironmentPredicate {
                metric: EnvironmentMetric::ViewportWidthPx,
                comparison: Comparison::AtMost,
                threshold: 991,
            }),
            Condition::Not(Box::new(Condition::PrefersReducedMotion)),
        ]);
        assert!(condition.evaluate(environment));
    }
}
