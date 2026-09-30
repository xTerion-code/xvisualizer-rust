//! Spectrum analysis: Hann window, FFT, log bands, adaptive gain.
//!
//! This module turns queued audio samples into smoothed per-bar levels
//! in the 0..1 range. It knows nothing about the terminal or rendering.

use std::collections::VecDeque;
use std::sync::Mutex;

use rustfft::{Fft, FftPlanner, num_complex::Complex};

use crate::audio::{RATE, WINDOW};

/// Upper bound for the bar count; the real count adapts to terminal width.
pub const MAX_BARS: usize = 64;

/// Lowest analyzed frequency in Hz.
const F_MIN: f32 = 30.0;
/// Highest analyzed frequency in Hz.
const F_MAX: f32 = 16_000.0;
/// Gamma applied to normalized magnitudes so quiet bands stay visible.
const GAMMA: f32 = 0.6;

/// FFT-based analyzer with inertial bar smoothing.
pub struct Analyzer {
    fft: std::sync::Arc<dyn Fft<f32>>,
    spectrum: Vec<Complex<f32>>,
    samples: Vec<f32>,
    mags: Vec<f32>,
    hann: Vec<f32>,
    edges: Vec<usize>,
    bars: Vec<f32>,
    caps: Vec<f32>,
    peak: f32,
    count: usize,
}

impl Analyzer {
    /// Creates an analyzer with a Hann window and a logarithmic bin grid.
    pub fn new() -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(WINDOW);

        // Hann window.
        let hann: Vec<f32> = (0..WINDOW)
            .map(|n| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * n as f32 / WINDOW as f32).cos()))
            .collect();

        // Logarithmic bin grid: F_MIN .. F_MAX.
        let bin_hz = RATE as f32 / WINDOW as f32;
        let mut edges = Vec::with_capacity(MAX_BARS + 1);
        for i in 0..=MAX_BARS {
            let f = F_MIN * (F_MAX / F_MIN).powf(i as f32 / MAX_BARS as f32);
            edges.push((f / bin_hz) as usize);
        }

        Self {
            fft,
            spectrum: vec![Complex::new(0.0, 0.0); WINDOW],
            samples: vec![0.0f32; WINDOW],
            mags: vec![0.0f32; WINDOW / 2],
            hann,
            edges,
            bars: vec![0.0f32; MAX_BARS],
            caps: vec![0.0f32; MAX_BARS],
            peak: 1e-3, // adaptive gain
            count: 0,
        }
    }

    /// Consumes the newest samples and advances bar smoothing by `dt` seconds.
    ///
    /// Returns `false` when the sample queue is gone and the app should stop.
    pub fn update(&mut self, queue: &Mutex<VecDeque<f32>>, bar_count: usize, dt: f32) -> bool {
        let n_bars = bar_count.clamp(1, MAX_BARS);
        self.count = n_bars;

        // Smooth yet instantly responsive: very fast attack (tau ~12ms,
        // 1-2 frames at 120Hz), soft release (tau ~160ms). The gain adapts
        // in ~0.4s, peaks fall in ~0.8s.
        let k_atk = 1.0 - (-dt / 0.012).exp();
        let k_rel = 1.0 - (-dt / 0.16).exp();
        let peak_keep = (-dt / 0.4).exp();
        let cap_fall = dt * 1.2;

        // Grab the sample window (short lock, no allocations).
        {
            let q = match queue.lock() {
                Ok(q) => q,
                Err(_) => return false,
            };
            let n = q.len().min(WINDOW);
            let skip = q.len() - n;
            // New data goes at the end of the window, the missing head is silence.
            let offset = WINDOW - n;
            self.samples[..offset].fill(0.0);
            for (dst, src) in self.samples[offset..].iter_mut().zip(q.iter().skip(skip)) {
                *dst = *src;
            }
        }
        for (s, (v, w)) in self
            .spectrum
            .iter_mut()
            .zip(self.samples.iter().zip(self.hann.iter()))
        {
            *s = Complex::new(v * w, 0.0);
        }
        self.fft.process(&mut self.spectrum);

        // Magnitudes.
        let mut frame_max = 1e-6f32;
        for (i, m) in self.mags.iter_mut().enumerate() {
            let c = self.spectrum[i + 1];
            let v = (c.re * c.re + c.im * c.im).sqrt() / WINDOW as f32;
            *m = v;
            if v > frame_max {
                frame_max = v;
            }
        }
        // Auto-volume: fast attack, slow release.
        self.peak = frame_max.max(self.peak * peak_keep).max(1e-4);

        // Bars + peaks (inertial, no stepping).
        for i in 0..n_bars {
            let lo = self.edges[i * MAX_BARS / n_bars].min(WINDOW / 2 - 1);
            let hi = self.edges[(i + 1) * MAX_BARS / n_bars]
                .max(lo + 1)
                .min(WINDOW / 2);
            let m = self.mags[lo..hi].iter().copied().fold(0.0f32, f32::max);
            let norm = (m / self.peak).clamp(0.0, 1.0);
            let target = norm.powf(GAMMA);
            let b = self.bars[i];
            // Ease towards the target: fast up, slow down.
            if target > b {
                self.bars[i] = b + (target - b) * k_atk;
            } else {
                self.bars[i] = b + (target - b) * k_rel;
            }
            // Peak marker: instantly up, linearly slowly down.
            if target >= self.caps[i] {
                self.caps[i] = target;
            } else {
                self.caps[i] = (self.caps[i] - cap_fall).max(target).max(0.0);
            }
        }
        true
    }

    /// Smoothed bar levels (0..1) from the last [`Analyzer::update`].
    pub fn bars(&self) -> &[f32] {
        &self.bars[..self.count]
    }

    /// Falling peak levels (0..1) from the last [`Analyzer::update`].
    pub fn peaks(&self) -> &[f32] {
        &self.caps[..self.count]
    }
}
