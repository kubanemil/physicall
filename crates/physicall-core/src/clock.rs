/// Length of one substep. The CPU and GPU solvers both take it from here.
pub fn substep(dt: f32, substeps: u32) -> f32 {
    dt / substeps.max(1) as f32
}

/// Turns variable frame times into a whole number of fixed steps, so the
/// simulation result depends only on simulated time, not on frame rate.
#[derive(Clone, Debug)]
pub struct FixedClock {
    step: f64,
    max_steps: u32,
    accumulator: f64,
}

impl FixedClock {
    pub fn new(step: f64, max_steps: u32) -> Self {
        Self {
            step,
            max_steps,
            accumulator: 0.0,
        }
    }

    pub fn step(&self) -> f64 {
        self.step
    }

    /// Adds `elapsed` seconds and returns how many fixed steps are now due.
    /// When more than `max_steps` are due the backlog is dropped, so a stalled
    /// tab slows the simulation down instead of freezing the page.
    pub fn advance(&mut self, elapsed: f64) -> u32 {
        // Tolerates rounding in summed frame times such as 3 * (1/180).
        const SLACK: f64 = 1e-6;
        self.accumulator += elapsed.max(0.0);
        let due = (self.accumulator / self.step + SLACK).floor();
        if due > f64::from(self.max_steps) {
            self.accumulator = 0.0;
            return self.max_steps;
        }
        self.accumulator = (self.accumulator - due * self.step).max(0.0);
        due as u32
    }

    pub fn reset(&mut self) {
        self.accumulator = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_of_any_length_add_up_to_the_same_steps() {
        for fps in [24.0, 30.0, 60.0, 75.0, 144.0, 240.0] {
            let mut clock = FixedClock::new(1.0 / 60.0, 100);
            let frames = (fps * 2.0) as u32;
            let steps: u32 = (0..frames).map(|_| clock.advance(1.0 / fps)).sum();
            assert_eq!(steps, 120, "{fps} fps");
        }
    }

    #[test]
    fn a_long_stall_is_capped_and_forgotten() {
        let mut clock = FixedClock::new(1.0 / 60.0, 4);
        assert_eq!(clock.advance(10.0), 4);
        assert_eq!(clock.advance(1.0 / 60.0), 1);
    }
}
