use crate::millis;


pub struct AccumulatorF32 {
    accumulated: f32,
    samples: u32,
    last_value: f32,
    last_time: u32,
    integrated_center: f32,
}

impl AccumulatorF32 {
    pub fn new() -> Self {
        AccumulatorF32::with_default(0.0)
    }

    pub fn with_default(default: f32) -> Self {
        AccumulatorF32 {
            accumulated: default,
            samples: 0,
            last_value: 0.0,
            last_time: millis::get_millis(),
            integrated_center: 0.0,
        }
    }

    /**
     * Integrate the added data using the trapezoidal method
     */
    pub fn add_data(&mut self, value: f32) {
        let time = millis::get_millis();

        let delta_time_ms = time - self.last_time;
        let area =
            delta_time_ms as f32 * 1e-3 * (self.last_value + value) / 2.0 - self.integrated_center;

        self.accumulated += area;
        self.last_value = value;
        self.last_time = time;
        self.samples += 1;
    }

    pub fn get_integrated_value(&self) -> f32 {
        self.accumulated
    }

    pub fn get_last_value(&self) -> f32 {
        self.last_value
    }

    pub fn reset(&mut self) {
        self.accumulated = 0.0;
        self.last_value = 0.0;
        self.last_time = millis::get_millis();
    }

    pub fn set_integrated_center(&mut self, center: f32) {
        self.integrated_center = center
    }

    pub fn get_integrated_average(&self) -> f32 {
        self.accumulated / self.samples as f32
    }
}
