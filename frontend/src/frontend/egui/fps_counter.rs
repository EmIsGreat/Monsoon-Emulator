use web_time::{Duration, Instant};

/// FPS counter that tracks frame times over the last second
#[derive(PartialEq, Clone)]
pub struct FpsCounter {
    frame_times: Vec<Instant>,
    last_update: Instant,
    current_fps: f32,
}

impl Default for FpsCounter {
    fn default() -> Self {
        Self {
            frame_times: Vec::new(),
            last_update: Instant::now(),
            current_fps: 0.0,
        }
    }
}

impl FpsCounter {
    pub fn update(&mut self) {
        let now = Instant::now();
        self.frame_times.push(now);

        // Keep only frames from the last second
        self.frame_times
            .retain(|&t| now.duration_since(t) < Duration::from_secs(1));

        // Update FPS counter every 0.5 seconds
        #[allow(clippy::cast_precision_loss)]
        if now.duration_since(self.last_update) >= Duration::from_millis(500) {
            self.current_fps = self.frame_times.len() as f32;
            self.last_update = now;
        }
    }

    pub fn fps(&self) -> f32 { self.current_fps }
}
