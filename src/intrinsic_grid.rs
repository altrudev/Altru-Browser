//! H3 candidate-only mixed-track geometry, never wired into live native rendering.
//! All auto-track contributions must be measured; unknown content fails closed.
use crate::capability_ir::{AcirGridTrack, AcirGridTrackList};
use crate::intrinsic_measurement::IntrinsicMeasurement;

#[derive(Debug, Clone, PartialEq)]
pub struct CandidateTrackGeometry {
    pub widths_px: Vec<f32>,
    pub offsets_px: Vec<f32>,
}

/// Single-row intrinsic sizing prototype. This deliberately does not claim
/// CSS Grid's multi-row spanning or full automatic minimum semantics.
pub fn candidate_single_row_tracks(
    tracks: &AcirGridTrackList,
    measurements: &[IntrinsicMeasurement],
    available_width_px: f32,
    gap_px: f32,
) -> Result<CandidateTrackGeometry, &'static str> {
    if !tracks.is_well_formed() || tracks.columns.len() != measurements.len() {
        return Err("invalid track or measurement count");
    }
    if !available_width_px.is_finite()
        || available_width_px < 0.0
        || !gap_px.is_finite()
        || gap_px < 0.0
    {
        return Err("invalid available geometry");
    }
    let mut widths = vec![0.0f32; tracks.columns.len()];
    let mut fractions = 0.0f64;
    let mut auto_sum = 0.0f64;
    for (index, track) in tracks.columns.iter().enumerate() {
        let IntrinsicMeasurement::Measured(sizes) = measurements[index] else {
            return Err("unknown intrinsic contribution");
        };
        if !sizes.min_content_px.is_finite()
            || !sizes.max_content_px.is_finite()
            || sizes.min_content_px < 0.0
            || sizes.max_content_px < sizes.min_content_px
        {
            return Err("invalid intrinsic contribution");
        }
        match track {
            AcirGridTrack::Auto => {
                widths[index] = sizes.max_content_px;
                auto_sum += f64::from(sizes.max_content_px);
            }
            AcirGridTrack::FractionMilli(weight) => {
                fractions += f64::from(*weight);
            }
        }
    }
    let gaps = (tracks.columns.len() - 1) as f64 * f64::from(gap_px);
    let free = (f64::from(available_width_px) - gaps - auto_sum).max(0.0);
    if !free.is_finite() || !fractions.is_finite() {
        return Err("unbounded track arithmetic");
    }
    for (index, track) in tracks.columns.iter().enumerate() {
        if let AcirGridTrack::FractionMilli(weight) = track {
            widths[index] = (free * f64::from(*weight) / fractions) as f32;
        }
    }
    let mut offsets = Vec::with_capacity(widths.len());
    let mut x = 0.0f32;
    for &width in &widths {
        if !width.is_finite() || !x.is_finite() {
            return Err("unbounded track coordinates");
        }
        offsets.push(x);
        x += width + gap_px;
    }
    Ok(CandidateTrackGeometry {
        widths_px: widths,
        offsets_px: offsets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordering_is_semantic() {
        let a = AcirGridTrackList {
            columns: vec![AcirGridTrack::FractionMilli(1000), AcirGridTrack::Auto],
        };
        let b = AcirGridTrackList {
            columns: vec![AcirGridTrack::Auto, AcirGridTrack::FractionMilli(1000)],
        };
        let m = [
            IntrinsicMeasurement::measured(20.0, 40.0),
            IntrinsicMeasurement::measured(30.0, 60.0),
        ];
        let x = candidate_single_row_tracks(&a, &m, 300.0, 10.0).unwrap();
        let y = candidate_single_row_tracks(&b, &m, 300.0, 10.0).unwrap();
        assert_eq!(x.widths_px, vec![230.0, 60.0]);
        assert_eq!(y.widths_px, vec![40.0, 250.0]);
        assert_ne!(x, y);
    }
    #[test]
    fn missing_measurement_is_never_synthesized() {
        let t = AcirGridTrackList {
            columns: vec![AcirGridTrack::FractionMilli(1000), AcirGridTrack::Auto],
        };
        let m = [
            IntrinsicMeasurement::measured(5.0, 10.0),
            IntrinsicMeasurement::Unknown(
                crate::intrinsic_measurement::UnknownMeasurement::FontMetricsUnavailable,
            ),
        ];
        assert!(candidate_single_row_tracks(&t, &m, 300.0, 0.0).is_err());
    }
}
