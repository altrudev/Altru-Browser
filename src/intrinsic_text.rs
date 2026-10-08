//! Bounded intrinsic widths measured from real font outlines and pair kerning.
//!
//! This provider deliberately handles the simple Latin/ASCII case only. It is
//! not a Unicode shaping engine: unsupported scripts and line-breaking semantics
//! must remain Unknown rather than being reported as correct layout geometry.

use crate::intrinsic_measurement::{IntrinsicMeasurement, UnknownMeasurement};
use ab_glyph::{Font, FontRef, PxScale, ScaleFont};

/// Limits ensure untrusted page text cannot force unbounded work.
pub const MAX_TEXT_BYTES: usize = 1_048_576;

/// Measure CSS min-content and max-content inline widths in CSS pixels.
/// The caller must supply the *resolved* face bytes and actual CSS pixel size.
/// No fallback font substitution is performed by this module.
///
/// Widths account for horizontal glyph advances and font pair kerning. This
/// is not ink bounding-box width. Hard line breaks separate max-content lines;
/// ASCII collapsible whitespace separates min-content opportunities. Tabs,
/// controls, non-ASCII shaping, and unsupported break modes fail closed.
pub fn measure_ascii_text(
    font_bytes: &[u8],
    font_size_px: f32,
    text: &str,
) -> IntrinsicMeasurement {
    if text.len() > MAX_TEXT_BYTES || !font_size_px.is_finite() || font_size_px <= 0.0 {
        return IntrinsicMeasurement::Unknown(UnknownMeasurement::UnboundedContent);
    }
    if !text.is_ascii()
        || text
            .bytes()
            .any(|b| (b < 32 && b != b'\n' && b != b' ') || b == 127)
    {
        return IntrinsicMeasurement::Unknown(UnknownMeasurement::FontMetricsUnavailable);
    }
    let Ok(font) = FontRef::try_from_slice(font_bytes) else {
        return IntrinsicMeasurement::Unknown(UnknownMeasurement::FontMetricsUnavailable);
    };
    let scaled = font.as_scaled(PxScale::from(font_size_px));
    let mut max_line = 0.0_f32;
    let mut line_width = 0.0_f32;
    let mut max_word = 0.0_f32;
    let mut word_width = 0.0_f32;
    let mut previous_line = None;
    let mut previous_word = None;
    for ch in text.chars() {
        if ch == '\n' {
            max_line = max_line.max(line_width);
            max_word = max_word.max(word_width);
            line_width = 0.0;
            word_width = 0.0;
            previous_line = None;
            previous_word = None;
            continue;
        }
        let glyph = scaled.glyph_id(ch);
        // A missing glyph must not silently become a .notdef width.
        if glyph.0 == 0 && ch != '\0' {
            return IntrinsicMeasurement::Unknown(UnknownMeasurement::FontMetricsUnavailable);
        }
        let advance = scaled.h_advance(glyph);
        if !advance.is_finite() || advance < 0.0 {
            return IntrinsicMeasurement::Unknown(UnknownMeasurement::FontMetricsUnavailable);
        }
        line_width += previous_line.map_or(0.0, |last| scaled.kern(last, glyph)) + advance;
        previous_line = Some(glyph);
        if ch == ' ' {
            max_word = max_word.max(word_width);
            word_width = 0.0;
            previous_word = None;
        } else {
            word_width += previous_word.map_or(0.0, |last| scaled.kern(last, glyph)) + advance;
            previous_word = Some(glyph);
        }
        if !line_width.is_finite() || !word_width.is_finite() {
            return IntrinsicMeasurement::Unknown(UnknownMeasurement::UnboundedContent);
        }
    }
    max_line = max_line.max(line_width);
    max_word = max_word.max(word_width);
    IntrinsicMeasurement::measured(max_word, max_line)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_missing_fonts_and_unsupported_shaping() {
        assert_eq!(
            measure_ascii_text(b"garbage", 16.0, "hello"),
            IntrinsicMeasurement::Unknown(UnknownMeasurement::FontMetricsUnavailable)
        );
        assert_eq!(
            measure_ascii_text(b"garbage", 16.0, "مرحبا"),
            IntrinsicMeasurement::Unknown(UnknownMeasurement::FontMetricsUnavailable)
        );
        assert_eq!(
            measure_ascii_text(b"garbage", f32::NAN, "hello"),
            IntrinsicMeasurement::Unknown(UnknownMeasurement::UnboundedContent)
        );
        assert_eq!(
            measure_ascii_text(b"garbage", 16.0, "hello\tworld"),
            IntrinsicMeasurement::Unknown(UnknownMeasurement::FontMetricsUnavailable)
        );
    }
    #[test]
    fn bounds_untrusted_text() {
        let enormous = "x".repeat(MAX_TEXT_BYTES + 1);
        assert_eq!(
            measure_ascii_text(b"", 16.0, &enormous),
            IntrinsicMeasurement::Unknown(UnknownMeasurement::UnboundedContent)
        );
    }
    #[test]
    fn actual_face_metrics_when_available() {
        // Environment-specific test: no font or fallback is silently bundled.
        let Ok(path) = std::env::var("ALTRU_TEST_FONT") else {
            return;
        };
        let bytes = std::fs::read(path).unwrap();
        let IntrinsicMeasurement::Measured(single) = measure_ascii_text(&bytes, 16.0, "hello")
        else {
            panic!("resolved font must measure")
        };
        let IntrinsicMeasurement::Measured(split) = measure_ascii_text(&bytes, 16.0, "hello world")
        else {
            panic!("resolved font must measure")
        };
        assert!(single.max_content_px > 0.0);
        assert!(split.min_content_px < split.max_content_px);
        let IntrinsicMeasurement::Measured(two_lines) =
            measure_ascii_text(&bytes, 16.0, "hello\nworld")
        else {
            panic!("resolved font must measure")
        };
        assert!(two_lines.max_content_px < split.max_content_px);
    }
}
