use std::env;
use std::fs;

use adaptive_web_engine_fabric::route_document;

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: awef <html-file>");
        std::process::exit(2);
    });

    let input = fs::read_to_string(&path).unwrap_or_else(|error| {
        eprintln!("failed to read {path}: {error}");
        std::process::exit(2);
    });

    let (receipt, result) = route_document(&input);

    println!("input_sha256={}", receipt.input_sha256);
    println!("decision_sha256={}", receipt.decision_sha256);
    println!("plane={:?}", receipt.classification.plane);
    println!("confidence={:?}", receipt.classification.confidence);
    println!("signals={:?}", receipt.classification.signals);
    println!("rationale={}", receipt.classification.rationale);
    println!("engine={}", result.summary);
    println!("authoritative={}", result.authoritative);
}
