#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServoLinkageProbe {
    pub compiled_with_servo: bool,
    pub crate_version: &'static str,
    pub api_surface: Vec<&'static str>,
}

#[cfg(feature = "servo-engine")]
pub fn linkage_probe() -> ServoLinkageProbe {
    let api_surface = vec![
        std::any::type_name::<servo::Servo>(),
        std::any::type_name::<servo::ServoBuilder>(),
        std::any::type_name::<servo::WebView>(),
        std::any::type_name::<servo::WebViewBuilder>(),
        std::any::type_name::<servo::SoftwareRenderingContext>(),
        std::any::type_name::<servo::OffscreenRenderingContext>(),
    ];

    ServoLinkageProbe {
        compiled_with_servo: true,
        crate_version: "0.6.0",
        api_surface,
    }
}

#[cfg(not(feature = "servo-engine"))]
pub fn linkage_probe() -> ServoLinkageProbe {
    ServoLinkageProbe {
        compiled_with_servo: false,
        crate_version: "0.6.0",
        api_surface: vec![
            "Servo",
            "ServoBuilder",
            "WebView",
            "WebViewBuilder",
            "SoftwareRenderingContext",
            "OffscreenRenderingContext",
        ],
    }
}

pub fn runtime_ready() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase1_does_not_claim_runtime_ready() {
        assert!(!runtime_ready());
    }

    #[test]
    fn records_expected_servo_api_surface() {
        let probe = linkage_probe();
        assert_eq!(probe.crate_version, "0.6.0");
        assert_eq!(probe.api_surface.len(), 6);
    }
}
