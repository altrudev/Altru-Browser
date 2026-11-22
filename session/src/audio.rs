//! Audio latency per kind of work, through PipeWire.
//!
//! Music needs small audio buffers so playing a key or a guitar sounds instantly.
//! Other work is better off with PipeWire's defaults (fewer dropouts, less CPU).
//! The session sets the PipeWire clock for its profile when it starts and puts the
//! defaults back when it ends. Apps it starts are also told the latency it wants
//! (`PIPEWIRE_LATENCY`), so JACK and PipeWire clients ask for the same buffer.

use std::process::Command;

/// A PipeWire clock setting: buffer size in frames and sample rate in Hz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioClock {
    pub quantum: u32,
    pub rate: u32,
}

impl AudioClock {
    /// One buffer's latency in milliseconds.
    pub fn latency_ms(self) -> f32 {
        self.quantum as f32 * 1000.0 / self.rate as f32
    }

    /// `PIPEWIRE_LATENCY` value, e.g. `128/48000`.
    pub fn env_value(self) -> String {
        format!("{}/{}", self.quantum, self.rate)
    }
}

/// Low latency for playing and recording: 128 frames at 48 kHz, about 2.7 ms.
pub const STUDIO: AudioClock = AudioClock {
    quantum: 128,
    rate: 48_000,
};

/// What the session does with audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioMode {
    /// Leave PipeWire as it is.
    Default,
    /// Force this clock while the session runs.
    Fixed(AudioClock),
}

/// Parse `--latency low|default|<frames>[@<rate>]`.
pub fn parse_latency(value: &str) -> Result<AudioMode, String> {
    match value {
        "low" | "studio" => return Ok(AudioMode::Fixed(STUDIO)),
        "default" | "normal" => return Ok(AudioMode::Default),
        _ => {}
    }
    let (frames, rate) = value.split_once('@').unwrap_or((value, "48000"));
    let quantum = frames
        .parse::<u32>()
        .ok()
        .filter(|q| q.is_power_of_two() && (16..=8192).contains(q))
        .ok_or_else(|| {
            format!("latency {value:?}: frames must be a power of two from 16 to 8192")
        })?;
    let rate = rate
        .parse::<u32>()
        .ok()
        .filter(|r| [44_100, 48_000, 88_200, 96_000, 176_400, 192_000].contains(r))
        .ok_or_else(|| {
            format!("latency {value:?}: rate must be 44100, 48000, 88200, 96000, 176400 or 192000")
        })?;
    Ok(AudioMode::Fixed(AudioClock { quantum, rate }))
}

/// `pw-metadata` invocations that apply a mode (`None` restores PipeWire defaults).
pub fn pw_metadata_commands(clock: Option<AudioClock>) -> Vec<Vec<String>> {
    let (quantum, rate) = clock.map_or((0, 0), |c| (c.quantum, c.rate));
    [("clock.force-rate", rate), ("clock.force-quantum", quantum)]
        .iter()
        .map(|(key, value)| {
            [
                "pw-metadata",
                "-n",
                "settings",
                "0",
                key,
                &value.to_string(),
            ]
            .iter()
            .map(|s| s.to_string())
            .collect()
        })
        .collect()
}

/// Apply (or restore) the clock. Returns a one-line status for the log.
pub fn apply(clock: Option<AudioClock>) -> String {
    for argv in pw_metadata_commands(clock) {
        let status = Command::new(&argv[0]).args(&argv[1..]).output();
        match status {
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                return format!(
                    "audio: pw-metadata failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }
            Err(error) => {
                return format!(
                    "audio: PipeWire not available ({error}); install: sudo apt install pipewire pipewire-bin wireplumber"
                );
            }
        }
    }
    match clock {
        Some(c) => format!(
            "audio: {} frames at {} Hz ({:.1} ms)",
            c.quantum,
            c.rate,
            c.latency_ms()
        ),
        None => "audio: PipeWire defaults restored".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn studio_is_about_three_milliseconds() {
        assert!((STUDIO.latency_ms() - 2.667).abs() < 0.01);
        assert_eq!(STUDIO.env_value(), "128/48000");
    }

    #[test]
    fn parses_latency_settings() {
        assert_eq!(parse_latency("low"), Ok(AudioMode::Fixed(STUDIO)));
        assert_eq!(parse_latency("default"), Ok(AudioMode::Default));
        assert_eq!(
            parse_latency("64@96000"),
            Ok(AudioMode::Fixed(AudioClock {
                quantum: 64,
                rate: 96_000
            }))
        );
        assert_eq!(
            parse_latency("256"),
            Ok(AudioMode::Fixed(AudioClock {
                quantum: 256,
                rate: 48_000
            }))
        );
        for bad in ["100", "8", "16384", "128@22050", "fast", "128@"] {
            assert!(parse_latency(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn builds_and_restores_the_pipewire_clock() {
        let set = pw_metadata_commands(Some(STUDIO));
        assert_eq!(
            set[1],
            [
                "pw-metadata",
                "-n",
                "settings",
                "0",
                "clock.force-quantum",
                "128"
            ]
        );
        assert_eq!(set[0][5], "48000");
        let restore = pw_metadata_commands(None);
        assert!(restore.iter().all(|argv| argv[5] == "0"));
    }
}
