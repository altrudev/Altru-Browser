use std::time::Duration;

use adaptive_web_engine_fabric::servo_runtime::render_local_document;

fn main() {
    let html = "<!doctype html><html><body><main><h1>AWEF Servo Probe</h1><p>bounded runtime evidence</p></main></body></html>";

    match render_local_document(html, 320, 200, Duration::from_secs(20)) {
        Ok(evidence) => {
            println!("servo_version={}", evidence.servo_version);
            println!("size={}x{}", evidence.width, evidence.height);
            println!("load_complete={}", evidence.load_complete);
            println!("frame_ready={}", evidence.frame_ready);
            println!("image_bytes={}", evidence.image_bytes);
            println!("non_uniform={}", evidence.non_uniform);
            println!("frame_sha256={}", evidence.frame_sha256);
        }
        Err(error) => {
            eprintln!("servo_runtime_error={error}");
            std::process::exit(1);
        }
    }
}
