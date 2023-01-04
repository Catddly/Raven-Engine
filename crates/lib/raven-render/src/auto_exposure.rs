use crate::renderer::post_process_renderer;

pub struct AutoExposureAdjustment {
    pub speed_log2: f32,

    ev_fast: f32,
    ev_slow: f32,

    enabled: bool,
}

impl AutoExposureAdjustment {
    pub fn new() -> Self {
        Self {
            speed_log2: 2.5_f32.log2(),

            ev_fast: 0.0,
            ev_slow: 0.0,

            // TODO: change to runtime variable
            enabled: post_process_renderer::ENABLE_AUTO_EXPOSURE,
        }
    }

    /// Get the smoothed transitioned exposure value
    pub fn get_ev_smoothed(&self) -> f32 {
        const DYNAMIC_EXPOSURE_BIAS: f32 = -2.0;

        if self.enabled {
            (self.ev_slow + self.ev_fast) * 0.5 + DYNAMIC_EXPOSURE_BIAS
        } else {
            0.0
        }
    }

    pub fn update_ev(&mut self, ev: f32, dt: f32) {
        if !self.enabled {
            return;
        }

        let ev = ev.clamp(post_process_renderer::LUMINANCE_HISTOGRAM_MIN_LOG2 as f32, post_process_renderer::LUMINANCE_HISTOGRAM_MAX_LOG2 as f32);

        let dt = dt * self.speed_log2.exp2(); // reverse operation

        let t_fast = 1.0 - (-1.0 * dt).exp();
        self.ev_fast = (ev - self.ev_fast) * t_fast + self.ev_fast;

        let t_slow = 1.0 - (-0.25 * dt).exp();
        self.ev_slow = (ev - self.ev_slow) * t_slow + self.ev_slow;
    }
}

#[derive(Clone, Copy)]
pub struct ExposureState {
    pub pre_mult: f32,
    pub post_mult: f32,

    pub pre_mult_prev_frame: f32,
    // pre_mult / pre_mult_prev_frame
    pub pre_mult_delta: f32,
}

impl Default for ExposureState {
    fn default() -> Self {
        Self {
            pre_mult: 1.0,
            post_mult: 1.0,
            pre_mult_prev_frame: 1.0,
            pre_mult_delta: 1.0,
        }
    }
}