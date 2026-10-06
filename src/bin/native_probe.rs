use adaptive_web_engine_fabric::engine_api::EngineAdapter;
use adaptive_web_engine_fabric::native_engine::NativeEngineAdapter;
use sha2::{Digest, Sha256};

fn main() {
    let mut engine = NativeEngineAdapter;
    let manifest = engine.manifest();
    let input = "<html><body><main><h1>AWEF Native</h1><p>Servo-independent evidence</p></main></body></html>";

    match engine.execute(input.to_string()) {
        Ok(execution) => {
            let artifact_sha256 = hex::encode(Sha256::digest(execution.artifact.as_bytes()));
            println!("implementation={}", manifest.implementation);
            println!("version={}", manifest.version);
            println!("promotion={:?}", manifest.promotion);
            for claim in &manifest.platform_support {
                println!("platform_{}={:?}", claim.platform, claim.status);
            }
            println!("nodes={}", execution.document.nodes().len());
            println!("scene_commands={}", execution.scene.commands.len());
            println!("native_semantics={}", execution.native_semantics);
            println!("production_promoted={}", execution.production_promoted);
            println!("artifact_sha256={artifact_sha256}");
        }
        Err(error) => {
            eprintln!("native_probe_error={error:?}");
            std::process::exit(1);
        }
    }
}
