// Solo capture via rerouting: the app is moved to a private null sink
// whose monitor is recorded, plus a loopback so it stays audible.
// Dropping the session moves the stream back and unloads the modules.

use std::sync::atomic::{AtomicU32, Ordering};

use crate::source::{PlaybackStream, pactl};

static SINK_SEQ: AtomicU32 = AtomicU32::new(0);

/// Active solo capture; restores everything on drop, including on panic.
pub struct SoloSession {
    null_module: u32,
    loop_module: Option<u32>,
    null_sink: String,
    input: u32,
    home_sink: u32,
    done: bool,
}

impl SoloSession {
    pub fn start(stream: &PlaybackStream) -> Result<Self, String> {
        // pid + time + sequence: unique across parallel app instances
        // and rapid successive solo switches.
        let uniq = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let seq = SINK_SEQ.fetch_add(1, Ordering::Relaxed);
        let null_sink = format!("xviz_cap_{}_{}_{}", std::process::id(), uniq, seq);
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

        // Best effort: without the loopback the app goes mute.
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

    pub fn monitor_source(&self) -> String {
        format!("{}.monitor", self.null_sink)
    }

    pub fn loopback_active(&self) -> bool {
        self.loop_module.is_some()
    }

    fn shutdown(&mut self) {
        if self.done {
            return;
        }
        self.done = true;
        // The stream may already be gone; ignore errors here.
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
