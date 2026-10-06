#![cfg(feature = "servo-engine")]

use std::time::Duration;

use adaptive_web_engine_fabric::servo_runtime::render_local_document;

#[test]
fn servo_software_runtime_produces_hashed_frame() {
    let html = "<!doctype html><html><body><main><h1>AWEF Servo Runtime</h1><p>local deterministic fixture</p></main></body></html>";

    let evidence = render_local_document(html, 320, 200, Duration::from_secs(20))
        .expect("Servo software runtime must load, paint, and read back the local fixture");

    assert_eq!(evidence.servo_version, "0.6.0");
    assert!(evidence.load_complete);
    assert!(evidence.frame_ready);
    assert_eq!(evidence.width, 320);
    assert_eq!(evidence.height, 200);
    assert_eq!(evidence.image_bytes, 320 * 200 * 4);
    assert!(evidence.non_uniform);
    assert_eq!(evidence.frame_sha256.len(), 64);
}
