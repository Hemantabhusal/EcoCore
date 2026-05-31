use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    pub target_fps: u16,
    pub metrics_sample_interval: Duration,
}

impl RuntimeConfig {
    pub fn frame_duration(self) -> Duration {
        target_frame_duration(self.target_fps)
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            target_fps: 30,
            metrics_sample_interval: Duration::from_millis(500),
        }
    }
}

pub fn target_frame_duration(target_fps: u16) -> Duration {
    let fps = u64::from(target_fps.max(1));
    Duration::from_nanos(1_000_000_000 / fps)
}
