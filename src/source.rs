use std::fmt::Write as _;
use std::process::Command;

use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::{MenuOutcome, UiEvent};
use crate::util::center_x;

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
            } else if let Some(rest) = t.strip_prefix("Sink:") {
                // Parsed regardless of Properties position; field order
                // differs across pactl/PipeWire versions.
                r.sink = rest.trim().parse().ok();
            } else if in_props && let Some((k, v)) = t.split_once('=') {
                let v = v.trim().trim_matches('"').to_string();
                match k.trim() {
                    "application.name" => r.app = Some(v),
                    "media.name" => r.media = Some(v),
                    "application.process.binary" => r.binary = Some(v),
                    _ => {}
                }
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

pub struct SourceMenu {
    pub target: CaptureTarget,
    pub apps: Vec<PlaybackStream>,
    pub selected: usize,
    pub open: bool,
}

impl SourceMenu {
    pub fn new() -> Self {
        Self {
            target: CaptureTarget::System,
            apps: Vec::new(),
            selected: 0,
            open: false,
        }
    }

    pub fn open(&mut self) {
        self.apps = list_playback_streams();
        self.selected = self.target.menu_index(&self.apps);
        self.open = true;
    }

    fn step(&mut self, dir: i32) {
        let n = self.apps.len() as i32 + 1;
        self.selected = (self.selected as i32 + dir).rem_euclid(n) as usize;
    }

    fn refresh(&mut self) {
        self.apps = list_playback_streams();
        let max = self.apps.len();
        self.selected = self.selected.min(max);
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> (MenuOutcome, Option<UiEvent>) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('w') => self.step(-1),
            KeyCode::Left | KeyCode::Char('h') => self.step(-1),
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => self.step(1),
            KeyCode::Right | KeyCode::Char('l') => self.step(1),
            KeyCode::Char(c @ '1'..='9') => {
                let idx = (c as usize) - ('1' as usize);
                if idx <= self.apps.len() {
                    self.selected = idx;
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => self.refresh(),
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.open = false;
                if self.selected == 0 {
                    self.target = CaptureTarget::System;
                    return (MenuOutcome::Confirmed, Some(UiEvent::SelectSystem));
                } else if let Some(app) = self.apps.get(self.selected - 1).cloned() {
                    self.target = CaptureTarget::Stream(app.clone());
                    return (MenuOutcome::Confirmed, Some(UiEvent::SelectStream(app)));
                }
                return (MenuOutcome::Confirmed, None);
            }
            KeyCode::Esc => {
                self.selected = self.target.menu_index(&self.apps);
                self.open = false;
                return (MenuOutcome::Cancelled, None);
            }
            _ => {}
        }
        (MenuOutcome::StillOpen, None)
    }
}

pub fn render(
    frame: &mut String,
    cols: usize,
    rows: usize,
    menu: &SourceMenu,
    setup: bool,
    use_color: bool,
) {
    frame.clear();
    let _ = write!(frame, "\x1b[H");
    let current_input = match &menu.target {
        CaptureTarget::Stream(s) => Some(s.input),
        CaptureTarget::System => None,
    };

    let mut lines: Vec<String> = Vec::with_capacity(menu.apps.len() + 5);
    lines.push(if setup {
        "Setup 4/4 - Select capture source".to_string()
    } else {
        "Select capture source".to_string()
    });
    lines.push("Up/Down - select  |  Enter - capture  |  R - refresh".to_string());
    lines.push(String::new());
    lines.push(menu_row(
        0,
        menu.selected,
        "System",
        "everything you hear",
        matches!(menu.target, CaptureTarget::System),
    ));
    for (i, app) in menu.apps.iter().enumerate() {
        lines.push(menu_row(
            i + 1,
            menu.selected,
            &app.app,
            &app.stream,
            current_input == Some(app.input),
        ));
    }
    if menu.apps.is_empty() {
        lines.push("(no apps playing audio right now)".to_string());
    }
    lines.push(String::new());
    lines.push(if setup {
        "Esc - skip setup  |  Q - quit".to_string()
    } else {
        "Esc - back  |  Q - quit".to_string()
    });

    let mut top = rows.saturating_sub(lines.len()) / 2;
    for _ in 0..top {
        frame.push_str("\x1b[K\r\n");
    }
    for (idx, line) in lines.iter().enumerate() {
        let is_title = idx == 0;
        let is_hint = idx == 1 || idx == lines.len() - 1;
        let is_option = !is_title && !is_hint && !line.is_empty() && !line.starts_with('(');
        if line.is_empty() {
            frame.push_str("\x1b[K\r\n");
            continue;
        }
        let x = center_x(cols, line.chars().count());
        frame.push_str(&" ".repeat(x));
        if is_title && use_color {
            frame.push_str("\x1b[1;36m");
        } else if is_hint && use_color {
            frame.push_str("\x1b[90m");
        } else if is_option && line.starts_with("> ") && use_color {
            frame.push_str("\x1b[7m");
        }
        frame.push_str(line);
        if (is_title || is_hint || (is_option && line.starts_with("> "))) && use_color {
            frame.push_str("\x1b[0m");
        }
        if idx + 1 < lines.len() {
            frame.push_str("\x1b[K\r\n");
        } else {
            frame.push_str("\x1b[K");
        }
    }
    // Clear any visualizer leftovers below the menu.
    top += lines.len();
    while top < rows {
        frame.push_str("\r\n\x1b[K");
        top += 1;
    }
}

fn menu_row(idx: usize, selected: usize, name: &str, detail: &str, is_current: bool) -> String {
    let marker = if idx == selected { "> " } else { "  " };
    let cur = if is_current { "  (current)" } else { "" };
    if detail.is_empty() {
        format!("{marker}{name}{cur}")
    } else {
        format!("{marker}{name} - {detail}{cur}")
    }
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

    #[test]
    fn parses_sink_after_properties() {
        let sample = "Sink Input #9\n\
\tProperties:\n\
\t\tapplication.name = \"late\"\n\
\tSink: 3\n";
        let apps = parse_sink_inputs(sample);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].input, 9);
        assert_eq!(apps[0].sink, 3);
        assert_eq!(apps[0].app, "late");
    }
}
