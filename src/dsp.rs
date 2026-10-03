use std::collections::VecDeque;
use std::sync::Mutex;

use rustfft::{Fft, FftPlanner, num_complex::Complex};

use crate::audio::{RATE, WINDOW};

pub const MAX_BARS: usize = 64;

const F_MIN: f32 = 30.0;
const F_MAX: f32 = 16_000.0;
// Gamma on normalized magnitudes so quiet bands stay visible.
const GAMMA: f32 = 0.6;
// Reference magnitude when auto-gain is off. Typical full-scale music peaks
// around 0.01-0.1 here (FFT magnitude / WINDOW), so gain compensates.
const FIXED_PEAK: f32 = 0.03;
const GAIN_MIN: f32 = 0.2;
const GAIN_MAX: f32 = 8.0;
const GAIN_STEP: f32 = 1.25;

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
    gain: f32,
    auto_gain: bool,
}

impl Analyzer {
    pub fn new() -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(WINDOW);

        let hann: Vec<f32> = (0..WINDOW)
            .map(|n| {
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * n as f32 / (WINDOW - 1) as f32).cos())
            })
            .collect();

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
            peak: 1e-3,
            count: 0,
            gain: 1.0,
            auto_gain: true,
        }
    }

    pub fn gain(&self) -> f32 {
        self.gain
    }

    pub fn auto_gain(&self) -> bool {
        self.auto_gain
    }

    pub fn adjust_gain(&mut self, up: bool) {
        if up {
            self.gain = (self.gain * GAIN_STEP).min(GAIN_MAX);
        } else {
            self.gain = (self.gain / GAIN_STEP).max(GAIN_MIN);
        }
    }

    pub fn reset_gain(&mut self) {
        self.gain = 1.0;
    }

    pub fn toggle_auto_gain(&mut self) {
        self.auto_gain = !self.auto_gain;
    }

    /// Returns false when the sample queue is gone and the app should stop.
    pub fn update(&mut self, queue: &Mutex<VecDeque<f32>>, bar_count: usize, dt: f32) -> bool {
        let n_bars = bar_count.clamp(1, MAX_BARS);
        self.count = n_bars;

        // Attack tau ~12 ms (1-2 frames at 120 Hz), release tau ~160 ms;
        // gain release ~0.4 s, peak fall ~0.8 s.
        let k_atk = 1.0 - (-dt / 0.012).exp();
        let k_rel = 1.0 - (-dt / 0.16).exp();
        let peak_keep = (-dt / 0.4).exp();
        let cap_fall = dt * 1.2;

        {
            let q = match queue.lock() {
                Ok(q) => q,
                Err(_) => return false,
            };
            let n = q.len().min(WINDOW);
            let skip = q.len() - n;
            // Missing head is silence; newest samples go at the window end.
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

        let mut frame_max = 1e-6f32;
        for (i, m) in self.mags.iter_mut().enumerate() {
            let c = self.spectrum[i + 1];
            let v = (c.re * c.re + c.im * c.im).sqrt() / WINDOW as f32;
            *m = v;
            if v > frame_max {
                frame_max = v;
            }
        }
        self.peak = if self.auto_gain {
            frame_max.max(self.peak * peak_keep).max(1e-4)
        } else {
            self.peak
        };
        let reference = if self.auto_gain {
            self.peak
        } else {
            FIXED_PEAK
        };

        for i in 0..n_bars {
            let lo = self.edges[i * MAX_BARS / n_bars].min(WINDOW / 2 - 1);
            let hi = self.edges[(i + 1) * MAX_BARS / n_bars]
                .max(lo + 1)
                .min(WINDOW / 2);
            let m = self.mags[lo..hi].iter().copied().fold(0.0f32, f32::max);
            let norm = (m / reference * self.gain).clamp(0.0, 1.0);
            let target = norm.powf(GAMMA);
            let b = self.bars[i];
            if target > b {
                self.bars[i] = b + (target - b) * k_atk;
            } else {
                self.bars[i] = b + (target - b) * k_rel;
            }
            if target >= self.caps[i] {
                self.caps[i] = target;
            } else {
                self.caps[i] = (self.caps[i] - cap_fall).max(target).max(0.0);
            }
        }
        true
    }

    pub fn bars(&self) -> &[f32] {
        &self.bars[..self.count]
    }

    pub fn peaks(&self) -> &[f32] {
        &self.caps[..self.count]
    }
}
