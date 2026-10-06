#[cfg(target_os = "linux")]
use std::fs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessMetrics {
    pub peak_rss_kib: Option<u64>,
}

pub fn process_metrics() -> ProcessMetrics {
    ProcessMetrics {
        peak_rss_kib: peak_rss_kib(),
    }
}

#[cfg(target_os = "linux")]
fn peak_rss_kib() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let value = line.strip_prefix("VmHWM:")?;
        value
            .split_whitespace()
            .next()
            .and_then(|number| number.parse::<u64>().ok())
    })
}

#[cfg(not(target_os = "linux"))]
fn peak_rss_kib() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_probe_is_safe() {
        let _ = process_metrics();
    }
}
