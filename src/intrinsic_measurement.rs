//! Owned intrinsic inline measurement contract (DDC/Frequency H2).
//!
//! No approximation may be silently presented as intrinsic content geometry.
//! Missing font, shaping, child, or asset metrics remain Unknown.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntrinsicInlineSize {
    pub min_content_px: f32,
    pub max_content_px: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownMeasurement {
    FontMetricsUnavailable,
    ChildUnmeasured,
    ResourcePending,
    UnboundedContent,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IntrinsicMeasurement {
    Measured(IntrinsicInlineSize),
    Unknown(UnknownMeasurement),
}

impl IntrinsicInlineSize {
    pub fn checked(min_content_px: f32, max_content_px: f32) -> Option<Self> {
        if !min_content_px.is_finite()
            || !max_content_px.is_finite()
            || min_content_px < 0.0
            || max_content_px < min_content_px
        {
            return None;
        }
        Some(Self { min_content_px, max_content_px })
    }
}

impl IntrinsicMeasurement {
    pub fn measured(min_content_px: f32, max_content_px: f32) -> Self {
        IntrinsicInlineSize::checked(min_content_px, max_content_px)
            .map(Self::Measured)
            .unwrap_or(Self::Unknown(UnknownMeasurement::UnboundedContent))
    }

    /// Combine inline siblings. Widths add; this is *not* block aggregation.
    pub fn combine_inline(self, other: Self) -> Self {
        match (self, other) {
            (Self::Measured(a), Self::Measured(b)) => Self::measured(
                a.min_content_px + b.min_content_px,
                a.max_content_px + b.max_content_px,
            ),
            (Self::Unknown(why), _) | (_, Self::Unknown(why)) => Self::Unknown(why),
        }
    }

    /// A block container needs the maximum intrinsic contribution of its
    /// independently stacked children, not the sum of their widths.
    pub fn combine_block(self, other: Self) -> Self {
        match (self, other) {
            (Self::Measured(a), Self::Measured(b)) => Self::measured(
                a.min_content_px.max(b.min_content_px),
                a.max_content_px.max(b.max_content_px),
            ),
            (Self::Unknown(why), _) | (_, Self::Unknown(why)) => Self::Unknown(why),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeasurementCacheKey {
    pub dom_mutation_epoch: u64,
    pub style_generation: u64,
    pub font_generation: u64,
    pub resource_generation: u64,
    pub available_width_milli_px: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_or_nonfinite_sizes_remain_unknown() {
        assert!(matches!(IntrinsicMeasurement::measured(-1.0, 10.0), IntrinsicMeasurement::Unknown(_)));
        assert!(matches!(IntrinsicMeasurement::measured(20.0, 10.0), IntrinsicMeasurement::Unknown(_)));
        assert!(matches!(IntrinsicMeasurement::measured(f32::NAN, 10.0), IntrinsicMeasurement::Unknown(_)));
    }

    #[test]
    fn inline_and_block_aggregation_have_distinct_meanings() {
        let a = IntrinsicMeasurement::measured(10.0, 30.0);
        let b = IntrinsicMeasurement::measured(20.0, 40.0);
        assert_eq!(a.combine_inline(b), IntrinsicMeasurement::measured(30.0, 70.0));
        assert_eq!(a.combine_block(b), IntrinsicMeasurement::measured(20.0, 40.0));
    }

    #[test]
    fn missing_font_metrics_cannot_be_fabricated() {
        let unknown = IntrinsicMeasurement::Unknown(UnknownMeasurement::FontMetricsUnavailable);
        assert_eq!(unknown.combine_inline(IntrinsicMeasurement::measured(20.0, 50.0)), unknown);
    }
}
