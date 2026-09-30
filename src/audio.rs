//! System audio capture via PulseAudio.
//!
//! This module only captures audio into a shared sample queue.
//! It knows nothing about FFT, themes or rendering.

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

/// Sample rate of the capture stream.
pub const RATE: u32 = 48_000;
/// FFT window size in samples.
pub const WINDOW: usize = 2048;
/// Stereo frames per blocking PulseAudio read (~10.7 ms).
pub const READ_FRAMES: usize = 512;
/// How many mono samples the shared queue keeps (~170 ms).
const QUEUE_CAP: usize = 8192;

/// Returns the monitor of the default sink (whatever is currently playing).
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

    // Prefer the monitor of the default sink.
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

/// Restartable background capture writing into one shared sample queue.
///
/// The capture device can change at runtime (system monitor vs. solo sink),
/// so the worker thread must be stoppable independently of the global
/// `running` flag.
pub struct CaptureSession {
    queue: Arc<Mutex<VecDeque<f32>>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl CaptureSession {
    /// Starts capturing `monitor` (None = default source).
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

    /// Shared sample queue (stable across [`CaptureSession::switch`]).
    pub fn queue(&self) -> &Arc<Mutex<VecDeque<f32>>> {
        &self.queue
    }

    /// Stops the current worker, drops stale audio, and starts capturing
    /// `monitor` instead. Blocks briefly while the old thread exits
    /// (it wakes from blocking reads within ~10ms).
    pub fn switch(&mut self, monitor: Option<String>, running: &Arc<AtomicBool>) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
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
    // Request stereo and average down to mono: Pulse monitors are usually stereo,
    // recording a single channel would capture the left channel only.
    let spec = Spec {
        format: Format::S16le,
        channels: 2,
        rate: RATE,
    };
    let dev = monitor.as_deref();
    // Low latency: Pulse's default fragsize is ~2s - that was the source of the
    // multi-second delay. Request ~10ms chunks and a small buffer (~43ms).
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
    while running.load(Ordering::SeqCst) && !stop.load(Ordering::SeqCst) {
        if simple.read(&mut raw).is_err() {
            thread::sleep(Duration::from_millis(50));
            continue;
        }
        // Decode WITHOUT holding the mutex, then take one short lock.
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
        // Short queue: old samples are discarded immediately,
        // nothing stale piles up.
        let excess = q.len().saturating_sub(QUEUE_CAP);
        if excess > 0 {
            q.drain(..excess);
        }
    }
}
