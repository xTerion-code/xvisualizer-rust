use std::process::Command;

#[derive(Clone, Debug)]
pub struct PlaybackStream {
    pub input: u32,
    pub sink: u32,
    pub app: String,
    pub stream: String,
}

impl PlaybackStream {
    pub fn label(&self) -> String {
        if self.stream.is_empty() || self.stream == self.app {
            self.app.clone()
        } else {
            format!("{} - {}", self.app, self.stream)
        }
    }
}

#[derive(Clone, Debug)]
pub enum CaptureTarget {
    System,
    Stream(PlaybackStream),
}

impl CaptureTarget {
    // Row 0 is always System, rows 1.. mirror `apps`.
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

    pub fn label(&self, system_label: &str) -> String {
        match self {
            CaptureTarget::System => system_label.to_string(),
            CaptureTarget::Stream(s) => format!("app: {}", s.label()),
        }
    }
}

pub fn list_playback_streams() -> Vec<PlaybackStream> {
    pactl(&["list", "sink-inputs"])
        .map(|out| parse_sink_inputs(&out))
        .unwrap_or_default()
}

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
        // `pactl` indents with tabs or spaces depending on version.
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("Sink Input #") {
            if let Some(prev) = cur.take() {
                raws.push(prev);
            }
            cur = Some(Raw {
                input: rest.trim().parse().ok(),
                ..Raw::default()
            });
            in_props = false;
        } else if let Some(r) = cur.as_mut() {
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

/// Active solo capture lives in `solo.rs`; restores everything on drop.
pub(crate) fn pactl(args: &[&str]) -> Result<String, String> {
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
        assert_eq!(apps[1].app, "firefox");
        assert_eq!(apps[1].label(), "firefox");
    }

    #[test]
    fn empty_output_gives_no_streams() {
        assert!(parse_sink_inputs("").is_empty());
    }

    #[test]
    fn parses_space_indented_output() {
        let sample = "Sink Input #7\n\
         Driver: PipeWire\n\
         Sink: 1\n\
         Properties:\n\
                 application.name = \"player\"\n\
                 media.name = \"music = loud\"\n";
        let apps = parse_sink_inputs(sample);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].input, 7);
        assert_eq!(apps[0].sink, 1);
        assert_eq!(apps[0].label(), "player - music = loud");
    }

    #[test]
    fn skips_entries_without_sink_or_input() {
        let sample = "Sink Input #x\n\
\tSink: 1\n\
\tProperties:\n\
\t\tapplication.name = \"broken\"\n";
        assert!(parse_sink_inputs(sample).is_empty());
    }
}
