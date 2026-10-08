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
pub enum AcirInteractionPredicate {
    Focus,
    FocusVisible,
    FocusWithin,
    Hover,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcirStructuralPredicate {
    FirstChild,
    LastChild,
    OnlyChild,
    NthChildIndex(u32),
    NthChildOdd,
    NthChildEven,
}

impl AcirStructuralPredicate {
    pub fn matches(self, element_index: usize, element_count: usize) -> bool {
        if element_index == 0 || element_count == 0 || element_index > element_count {
            return false;
        }
        match self {
            Self::FirstChild => element_index == 1,
            Self::LastChild => element_index == element_count,
            Self::OnlyChild => element_count == 1,
            Self::NthChildIndex(expected) => element_index == expected as usize,
            Self::NthChildOdd => element_index % 2 == 1,
            Self::NthChildEven => element_index % 2 == 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcirSelectorBoolean {
    Any,
    None,
    AnyZeroSpecificity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcirSelectorRelation {
    Descendant,
    Child,
    AdjacentSibling,
    GeneralSibling,
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


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcirSupportCondition {
    CssDeclaration {
        property: String,
        value: String,
    },
    All(Vec<AcirSupportCondition>),
    Any(Vec<AcirSupportCondition>),
    Not(Box<AcirSupportCondition>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcirLengthBasis {
    ParentFontSize,
    CurrentFontSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcirRelativeLength {
    /// Relative factor represented in thousandths of the source unit.
    /// For example, 1em = 1000 and 1.25em = 1250.
    pub milli_factor: u32,
    pub basis: AcirLengthBasis,
}

impl AcirRelativeLength {
    pub fn resolve_px(self, parent_font_size_px: f32, current_font_size_px: f32) -> f32 {
        let basis = match self.basis {
            AcirLengthBasis::ParentFontSize => parent_font_size_px,
            AcirLengthBasis::CurrentFontSize => current_font_size_px,
        };
        basis * (self.milli_factor as f32 / 1_000.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcirSelectorChain {
    pub compounds: Vec<String>,
    pub combinators: Vec<AcirSelectorRelation>,
}

impl AcirSelectorChain {
    pub fn is_well_formed(&self) -> bool {
        !self.compounds.is_empty()
            && self.combinators.len().saturating_add(1) == self.compounds.len()
            && self.compounds.iter().all(|compound| !compound.trim().is_empty())
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
    fn structural_predicates_are_deterministic() {
        assert!(AcirStructuralPredicate::FirstChild.matches(1, 3));
        assert!(AcirStructuralPredicate::LastChild.matches(3, 3));
        assert!(AcirStructuralPredicate::OnlyChild.matches(1, 1));
        assert!(AcirStructuralPredicate::NthChildIndex(2).matches(2, 4));
        assert!(AcirStructuralPredicate::NthChildOdd.matches(3, 5));
        assert!(AcirStructuralPredicate::NthChildEven.matches(4, 5));
        assert!(!AcirStructuralPredicate::OnlyChild.matches(1, 2));
    }

    #[test]
    fn interaction_predicates_are_explicit_semantics() {
        assert_ne!(AcirInteractionPredicate::Focus, AcirInteractionPredicate::FocusVisible);
        assert_ne!(AcirInteractionPredicate::Focus, AcirInteractionPredicate::Hover);
        assert_ne!(
            AcirInteractionPredicate::FocusWithin,
            AcirInteractionPredicate::Active
        );
    }

    #[test]
    fn support_conditions_preserve_boolean_structure() {
        let condition = AcirSupportCondition::Any(vec![
            AcirSupportCondition::CssDeclaration {
                property: "display".into(),
                value: "grid".into(),
            },
            AcirSupportCondition::Not(Box::new(AcirSupportCondition::CssDeclaration {
                property: "mask-image".into(),
                value: "none".into(),
            })),
        ]);
        assert!(matches!(condition, AcirSupportCondition::Any(_)));
    }

    #[test]
    fn selector_boolean_modes_preserve_specificity_semantics() {
        assert_ne!(
            AcirSelectorBoolean::Any,
            AcirSelectorBoolean::AnyZeroSpecificity
        );
        assert_ne!(AcirSelectorBoolean::None, AcirSelectorBoolean::Any);
    }

    #[test]
    fn selector_chain_requires_one_relation_between_compounds() {
        let chain = AcirSelectorChain {
            compounds: vec!["main".into(), "section".into(), ".card".into()],
            combinators: vec![
                AcirSelectorRelation::Child,
                AcirSelectorRelation::AdjacentSibling,
            ],
        };
        assert!(chain.is_well_formed());
    }

    #[test]
    fn relative_font_length_resolves_against_parent_font_size() {
        let value = AcirRelativeLength {
            milli_factor: 1_250,
            basis: AcirLengthBasis::ParentFontSize,
        };
        assert_eq!(value.resolve_px(16.0, 20.0), 20.0);

        let current = AcirRelativeLength {
            milli_factor: 500,
            basis: AcirLengthBasis::CurrentFontSize,
        };
        assert_eq!(current.resolve_px(16.0, 20.0), 10.0);
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
