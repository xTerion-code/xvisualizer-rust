use std::collections::VecDeque;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use libpulse_binding::def::BufferAttr;
use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;

pub const RATE: u32 = 48_000;
pub const WINDOW: usize = 2048;
pub const READ_FRAMES: usize = 512;
// Newest ~170 ms of mono samples; each frame analyzes the newest WINDOW of them.
const QUEUE_CAP: usize = 8192;

pub fn detect_monitor() -> Option<String> {
    let sinks = Command::new("pactl")
        .arg("list")
        .arg("short")
        .arg("sources")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();

    let monitors: Vec<String> = sinks
        .lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .filter(|n| n.contains("monitor"))
        .map(|s| s.to_string())
        .collect();

    if monitors.is_empty() {
        return None;
    }

    if let Ok(o) = Command::new("pactl").arg("get-default-sink").output() {
        let sink = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !sink.is_empty() {
            let cand = format!("{sink}.monitor");
            if monitors.contains(&cand) {
                return Some(cand);
            }
        }
    }
    monitors.into_iter().next()
}

/// Restartable background capture sharing one sample queue.
///
/// The device can change at runtime, so the worker has its own `stop` flag
/// separate from the global `running` flag.
pub struct CaptureSession {
    queue: Arc<Mutex<VecDeque<f32>>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl CaptureSession {
    pub fn start(monitor: Option<String>, running: Arc<AtomicBool>) -> Self {
        let queue: Arc<Mutex<VecDeque<f32>>> =
            Arc::new(Mutex::new(VecDeque::with_capacity(QUEUE_CAP)));
        let stop = Arc::new(AtomicBool::new(false));
        let handle = spawn_worker(queue.clone(), monitor, running, stop.clone());
        Self {
            queue,
            stop,
            handle: Some(handle),
        }
    }

    pub fn queue(&self) -> &Arc<Mutex<VecDeque<f32>>> {
        &self.queue
    }

    /// Stops the worker, drops stale audio, and captures `monitor` instead.
    pub fn switch(&mut self, monitor: Option<String>, running: &Arc<AtomicBool>) {
        self.stop_worker();
        if let Ok(mut q) = self.queue.lock() {
            q.clear();
        }
        let stop = Arc::new(AtomicBool::new(false));
        self.stop = stop.clone();
        self.handle = Some(spawn_worker(
            self.queue.clone(),
            monitor,
            running.clone(),
            stop,
        ));
    }

    fn stop_worker(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for CaptureSession {
    fn drop(&mut self) {
        self.stop_worker();
    }
}

fn spawn_worker(
    queue: Arc<Mutex<VecDeque<f32>>>,
    monitor: Option<String>,
    running: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || capture_loop(queue, monitor, running, stop))
}

fn capture_loop(
    buf: Arc<Mutex<VecDeque<f32>>>,
    monitor: Option<String>,
    running: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
) {
    // Monitors are usually stereo; a single channel would capture left only.
    let spec = Spec {
        format: Format::S16le,
        channels: 2,
        rate: RATE,
    };
    let dev = monitor.as_deref();
    // Pulse defaults to ~2s fragments (multi-second delay); request ~10 ms
    // chunks with a small buffer instead.
    let attr = BufferAttr {
        maxlength: 8192,
        tlength: u32::MAX,
        prebuf: u32::MAX,
        minreq: u32::MAX,
        fragsize: (READ_FRAMES * 4) as u32,
    };
    let simple = match Simple::new(
        None,
        "xvisualizer",
        Direction::Record,
        dev,
        "capture",
        &spec,
        None,
        Some(&attr),
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("pulse record error: {e:?}");
            running.store(false, Ordering::SeqCst);
            return;
        }
    };

    let mut raw = vec![0u8; READ_FRAMES * 4];
    let mut mono = vec![0.0f32; READ_FRAMES];
    let mut failures = 0u32;
    while running.load(Ordering::SeqCst) && !stop.load(Ordering::SeqCst) {
        if simple.read(&mut raw).is_err() {
            failures += 1;
            // Persistent read errors mean the device is gone; shut down
            // instead of spinning silently (fatal-by-design, see AGENTS.md).
            if failures > 20 {
                eprintln!("pulse record error: device read failed repeatedly");
                running.store(false, Ordering::SeqCst);
                return;
            }
            thread::sleep(Duration::from_millis(50));
            continue;
        }
        failures = 0;
        // Decode outside the lock, then take one short lock to publish.
        for (i, frame) in raw.as_chunks::<4>().0.iter().enumerate() {
            let l = i16::from_le_bytes([frame[0], frame[1]]) as f32;
            let r = i16::from_le_bytes([frame[2], frame[3]]) as f32;
            mono[i] = (l + r) / 2.0 / 32768.0;
        }
        let mut q = match buf.lock() {
            Ok(q) => q,
            Err(_) => return,
        };
        q.extend(mono.iter().copied());
        let excess = q.len().saturating_sub(QUEUE_CAP);
        if excess > 0 {
            q.drain(..excess);
        }
    }
}
