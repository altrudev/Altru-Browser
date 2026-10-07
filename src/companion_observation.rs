use crate::native_runtime::NativeRuntimeError;
use serde::{Deserialize, Serialize};
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const COMPANION_OBSERVATION_SCHEMA: &str = "altru.companion.browser-observation.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowserObservationKind {
    NavigationSucceeded,
    NavigationFailed,
    CapabilityGap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserCapabilityObservation {
    pub schema: String,
    pub observed_at_unix_ms: u128,
    pub source: String,
    pub target: String,
    pub kind: BrowserObservationKind,
    pub gap_id: Option<String>,
    pub detail: String,
    pub authority_effect: String,
}

impl BrowserCapabilityObservation {
    pub fn success(target: impl Into<String>) -> Self {
        Self {
            schema: COMPANION_OBSERVATION_SCHEMA.into(),
            observed_at_unix_ms: now_ms(),
            source: "altru-browser".into(),
            target: target.into(),
            kind: BrowserObservationKind::NavigationSucceeded,
            gap_id: None,
            detail: "native runtime completed top-level navigation".into(),
            authority_effect: "none".into(),
        }
    }

    pub fn failure(target: impl Into<String>, error: &NativeRuntimeError) -> Self {
        let (gap_id, detail) = classify_runtime_error(error);
        Self {
            schema: COMPANION_OBSERVATION_SCHEMA.into(),
            observed_at_unix_ms: now_ms(),
            source: "altru-browser".into(),
            target: target.into(),
            kind: BrowserObservationKind::CapabilityGap,
            gap_id: Some(gap_id.into()),
            detail,
            authority_effect: "observation-only".into(),
        }
    }
}

pub fn append_local_observation(
    observation: &BrowserCapabilityObservation,
) -> std::io::Result<PathBuf> {
    let path = observation_path();
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    serde_json::to_writer(&mut file, observation)?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    Ok(path)
}

pub fn observation_path() -> PathBuf {
    if let Some(state_home) = std::env::var_os("XDG_STATE_HOME") {
        return PathBuf::from(state_home)
            .join("altru-companion")
            .join("browser-observations.jsonl");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("altru-companion")
            .join("browser-observations.jsonl");
    }
    std::env::temp_dir()
        .join("altru-companion")
        .join("browser-observations.jsonl")
}

fn classify_runtime_error(error: &NativeRuntimeError) -> (&'static str, String) {
    match error {
        NativeRuntimeError::Navigation(inner) => {
            ("navigation", format!("navigation failure: {inner:?}"))
        }
        NativeRuntimeError::Resource(inner) => (
            "resource-acquisition",
            format!("resource failure: {inner:?}"),
        ),
        NativeRuntimeError::HttpStatus(status) => {
            ("http-status", format!("top-level HTTP status {status}"))
        }
        NativeRuntimeError::InvalidUtf8 => (
            "document-decoding",
            "top-level document was not valid UTF-8".into(),
        ),
        NativeRuntimeError::Engine(inner) => {
            ("native-engine", format!("native engine failure: {inner:?}"))
        }
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_api::ResourceError;

    #[test]
    fn failure_observation_is_explicitly_non_authoritative() {
        let observation = BrowserCapabilityObservation::failure(
            "https://example.invalid",
            &NativeRuntimeError::Resource(ResourceError::Denied),
        );
        assert_eq!(observation.gap_id.as_deref(), Some("resource-acquisition"));
        assert_eq!(observation.authority_effect, "observation-only");
        assert_eq!(observation.kind, BrowserObservationKind::CapabilityGap);
    }

    #[test]
    fn success_observation_never_claims_authority() {
        let observation = BrowserCapabilityObservation::success("https://example.com");
        assert_eq!(observation.authority_effect, "none");
        assert!(observation.gap_id.is_none());
    }
}
