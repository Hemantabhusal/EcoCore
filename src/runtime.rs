use std::time::{Duration, Instant};

use crate::terminal::{TerminalSize, TerminalValidationError, validate_terminal_environment};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    pub target_fps: u16,
    pub metrics_sample_interval: Duration,
    pub resize_debounce: Duration,
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
            resize_debounce: Duration::from_millis(50),
        }
    }
}

pub fn target_frame_duration(target_fps: u16) -> Duration {
    let fps = u64::from(target_fps.max(1));
    Duration::from_nanos(1_000_000_000 / fps)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResizeDecision {
    Redraw {
        size: TerminalSize,
    },
    Suspend {
        actual: TerminalSize,
        minimum: TerminalSize,
    },
}

pub fn resize_decision(size: TerminalSize) -> ResizeDecision {
    match validate_terminal_environment(true, size) {
        Ok(()) => ResizeDecision::Redraw { size },
        Err(TerminalValidationError::TooSmall { actual, minimum }) => {
            ResizeDecision::Suspend { actual, minimum }
        }
        Err(TerminalValidationError::StdoutNotTerminal) => {
            unreachable!("resize events only occur after stdout was validated as a terminal")
        }
    }
}

#[derive(Clone, Debug)]
pub struct ResizeDebouncer {
    delay: Duration,
    pending_size: Option<TerminalSize>,
    ready_at: Option<Instant>,
}

impl ResizeDebouncer {
    pub const fn new(delay: Duration) -> Self {
        Self {
            delay,
            pending_size: None,
            ready_at: None,
        }
    }

    pub fn observe(&mut self, size: TerminalSize, now: Instant) {
        self.pending_size = Some(size);
        self.ready_at = Some(now + self.delay);
    }

    pub fn ready_at(&self) -> Option<Instant> {
        self.ready_at
    }

    pub fn take_due(&mut self, now: Instant) -> Option<ResizeDecision> {
        let ready_at = self.ready_at?;
        if now < ready_at {
            return None;
        }

        let size = self
            .pending_size
            .take()
            .expect("pending size exists when ready_at is set");
        self.ready_at = None;
        Some(resize_decision(size))
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FrameStats {
    frames: u64,
    changed_cells: u64,
    bytes: u64,
}

impl FrameStats {
    pub fn record_frame(&mut self, changed_cells: usize, bytes: usize) {
        self.frames += 1;
        self.changed_cells += changed_cells as u64;
        self.bytes += bytes as u64;
    }

    pub const fn frames(&self) -> u64 {
        self.frames
    }

    pub fn average_changed_cells(&self) -> u64 {
        average(self.changed_cells, self.frames)
    }

    pub fn average_bytes(&self) -> u64 {
        average(self.bytes, self.frames)
    }

    pub fn take_summary(&mut self) -> String {
        let summary = format!(
            "{} frames, avg {} changed cells, avg {} bytes",
            self.frames,
            self.average_changed_cells(),
            self.average_bytes()
        );
        *self = Self::default();
        summary
    }
}

fn average(total: u64, count: u64) -> u64 {
    total.checked_div(count).unwrap_or(0)
}
