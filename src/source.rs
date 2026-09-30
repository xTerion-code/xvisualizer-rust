//! Capture source selection: system mix or a single app.
//!
//! PulseAudio/PipeWire can only record sources (monitors), not individual
//! playback streams. Capturing one app therefore uses rerouting:
//! a private null sink is created, the app's sink-input is moved there,
//! and this app records the null sink's monitor. A loopback module routes
//! the audio back to the original sink so it stays audible on speakers.
//!
//! All state is restored (stream moved back, modules unloaded) when the
//! solo session ends, including on panic via [`SoloSession`]'s Drop impl.

use std::process::Command;

/// One playback stream (sink-input) currently producing audio.
#[derive(Clone, Debug)]
pub struct PlaybackStream {
    /// Sink-input index (`pactl move-sink-input` target).
    pub input: u32,
    /// Sink index the stream currently plays to (restored on stop).
    pub sink: u32,
    /// Human-readable app name (`application.name` or binary fallback).
    pub app: String,
    /// Stream name (`media.name` or fallback).
    pub stream: String,
}

impl PlaybackStream {
    /// Short label for menus and the footer.
    pub fn label(&self) -> String {
        if self.stream.is_empty() || self.stream == self.app {
            self.app.clone()
        } else {
            format!("{} - {}", self.app, self.stream)
        }
    }
}

/// What the visualizer captures.
#[derive(Clone, Debug)]
pub enum CaptureTarget {
    /// Default monitor: the whole system mix.
    System,
    /// A single playback stream via a solo session.
    Stream(PlaybackStream),
}

impl CaptureTarget {
    /// Menu row index for this target given the current app list.
    /// Row 0 is always "System", rows 1.. mirror `apps`.
    pub fn menu_index(&self, apps: &[PlaybackStream]) -> usize {
        match self {
            CaptureTarget::System => 0,
            CaptureTarget::Stream(s) => apps
                .iter()
                .position(|a| a.input == s.input)
                .map(|i| i + 1)
                .unwrap_or(0),
        }
    }

    /// Short label for the footer.
    pub fn label(&self, system_label: &str) -> String {
        match self {
            CaptureTarget::System => system_label.to_string(),
            CaptureTarget::Stream(s) => format!("app: {}", s.label()),
        }
    }
}

/// Lists current playback streams via `pactl list sink-inputs`.
pub fn list_playback_streams() -> Vec<PlaybackStream> {
    pactl(&["list", "sink-inputs"])
        .map(|out| parse_sink_inputs(&out))
        .unwrap_or_default()
}

/// Parses `pactl list sink-inputs` output into playback streams.
fn parse_sink_inputs(out: &str) -> Vec<PlaybackStream> {
    #[derive(Default)]
    struct Raw {
        input: Option<u32>,
        sink: Option<u32>,
        app: Option<String>,
        media: Option<String>,
        binary: Option<String>,
    }
    let mut raws: Vec<Raw> = Vec::new();
    let mut cur: Option<Raw> = None;
    let mut in_props = false;

    for line in out.lines() {
        if let Some(rest) = line.strip_prefix("Sink Input #") {
            if let Some(prev) = cur.take() {
                raws.push(prev);
            }
            cur = Some(Raw {
                input: rest.trim().parse().ok(),
                ..Raw::default()
            });
            in_props = false;
        } else if let Some(r) = cur.as_mut() {
            let t = line.trim_start_matches('\t');
            if t == "Properties:" {
                in_props = true;
            } else if in_props {
                if let Some((k, v)) = t.split_once('=') {
                    let v = v.trim().trim_matches('"').to_string();
                    match k.trim() {
                        "application.name" => r.app = Some(v),
                        "media.name" => r.media = Some(v),
                        "application.process.binary" => r.binary = Some(v),
                        _ => {}
                    }
                }
            } else if let Some(rest) = t.strip_prefix("Sink:") {
                r.sink = rest.trim().parse().ok();
            }
        }
    }
    if let Some(prev) = cur.take() {
        raws.push(prev);
    }

    raws.into_iter()
        .filter_map(|r| {
            let app = r
                .app
                .or(r.binary)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "unknown app".to_string());
            Some(PlaybackStream {
                input: r.input?,
                sink: r.sink?,
                stream: r.media.unwrap_or_default(),
                app,
            })
        })
        .collect()
}

/// Active solo capture of one app. Restores everything on drop.
pub struct SoloSession {
    null_module: u32,
    loop_module: Option<u32>,
    null_sink: String,
    input: u32,
    home_sink: u32,
    done: bool,
}

impl SoloSession {
    /// Reroutes `stream` through a private null sink and returns the session.
    /// After this returns Ok, capture from [`SoloSession::monitor_source`].
    pub fn start(stream: &PlaybackStream) -> Result<Self, String> {
        let null_sink = format!("xviz_cap_{}", std::process::id());
        let null_module: u32 = pactl(&[
            "load-module",
            "module-null-sink",
            &format!("sink_name={null_sink}"),
            "sink_properties=device.description=XVisualizerAppCapture",
        ])
        .and_then(|out| {
            out.trim()
                .parse()
                .map_err(|_| "unexpected load-module output".to_string())
        })
        .map_err(|e| format!("cannot create capture sink: {e}"))?;

        let mut session = Self {
            null_module,
            loop_module: None,
            null_sink: null_sink.clone(),
            input: stream.input,
            home_sink: stream.sink,
            done: false,
        };

        if let Err(e) = pactl(&["move-sink-input", &stream.input.to_string(), &null_sink]) {
            session.shutdown();
            return Err(format!(
                "cannot move app stream (is it still playing?): {e}"
            ));
        }

        // Route the audio back to the speakers so capturing stays inaudible
        // as a side effect. Best effort: without it the app simply goes mute.
        session.loop_module = pactl(&[
            "load-module",
            "module-loopback",
            &format!("source={null_sink}.monitor"),
            &format!("sink={}", stream.sink),
            "latency_msec=50",
        ])
        .ok()
        .and_then(|out| out.trim().parse().ok());

        Ok(session)
    }

    /// Monitor source to record while this session is alive.
    pub fn monitor_source(&self) -> String {
        format!("{}.monitor", self.null_sink)
    }

    /// Whether the audibility loopback is active.
    pub fn loopback_active(&self) -> bool {
        self.loop_module.is_some()
    }

    fn shutdown(&mut self) {
        if self.done {
            return;
        }
        self.done = true;
        // Move the stream back where it was; ignore errors (it may be gone).
        let _ = pactl(&[
            "move-sink-input",
            &self.input.to_string(),
            &self.home_sink.to_string(),
        ]);
        if let Some(m) = self.loop_module.take() {
            let _ = pactl(&["unload-module", &m.to_string()]);
        }
        let _ = pactl(&["unload-module", &self.null_module.to_string()]);
    }
}

impl Drop for SoloSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn pactl(args: &[&str]) -> Result<String, String> {
    let out = Command::new("pactl")
        .args(args)
        .output()
        .map_err(|e| format!("pactl failed to run: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "pactl {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Sink Input #471\n\
\tDriver: PipeWire\n\
\tClient: 83\n\
\tSink: 443\n\
\tCorked: no\n\
\tProperties:\n\
\t\tapplication.name = \"WEBRTC VoiceEngine\"\n\
\t\tapplication.process.binary = \"Discord\"\n\
\t\tmedia.name = \"playStream\"\n\
\n\
Sink Input #550\n\
\tSink: 0\n\
\tProperties:\n\
\t\tapplication.process.binary = \"firefox\"\n\
";

    #[test]
    fn parses_sink_inputs() {
        let apps = parse_sink_inputs(SAMPLE);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].input, 471);
        assert_eq!(apps[0].sink, 443);
        assert_eq!(apps[0].app, "WEBRTC VoiceEngine");
        assert_eq!(apps[0].label(), "WEBRTC VoiceEngine - playStream");
        // No application.name: falls back to the binary.
        assert_eq!(apps[1].app, "firefox");
        assert_eq!(apps[1].label(), "firefox");
    }

    #[test]
    fn empty_output_gives_no_streams() {
        assert!(parse_sink_inputs("").is_empty());
    }
}
