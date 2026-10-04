const MIN: f32 = 0.2;
const MAX: f32 = 8.0;
const STEP: f32 = 1.25;

pub struct Gain {
    value: f32,
}

impl Gain {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn up(&mut self) {
        self.value = (self.value * STEP).min(MAX);
    }

    pub fn down(&mut self) {
        self.value = (self.value / STEP).max(MIN);
    }

    pub fn reset(&mut self) {
        self.value = 1.0;
    }
}

impl Default for Gain {
    fn default() -> Self {
        Self { value: 1.0 }
    }
}
