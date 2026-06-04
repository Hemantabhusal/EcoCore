use std::time::{Duration, Instant};

use crate::{
    layout::CellSize,
    terminal::{TerminalSize, TerminalValidationError, validate_terminal_environment},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    pub target_fps: u16,
    pub metrics_sample_interval: Duration,
    pub resize_debounce: Duration,
    pub image_columns: u16,
    pub image_rows: u16,
    pub cell_size: CellSize,
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
            // Keep the graphics viewport fixed and bounded for predictable
            // terminal bandwidth, but large enough that the tidepool reads as
            // a scene instead of a small preview tile.
            image_columns: 42,
            image_rows: 14,
            // First-pass layout uses a conservative default until terminal
            // pixel-size probing is added during the terminal support review.
            cell_size: CellSize::new(8, 16),
        }
    }
}

pub fn target_frame_duration(target_fps: u16) -> Duration {
    let fps = u64::from(target_fps.max(1));
    Duration::from_nanos(1_000_000_000 / fps)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameDeadlineAdvance {
    pub next_deadline: Instant,
    pub skipped_deadlines: u64,
}

pub fn advance_frame_deadline(
    previous_deadline: Instant,
    frame_duration: Duration,
    now: Instant,
) -> FrameDeadlineAdvance {
    if frame_duration.is_zero() {
        return FrameDeadlineAdvance {
            next_deadline: now,
            skipped_deadlines: 0,
        };
    }

    let mut next_deadline = previous_deadline + frame_duration;
    let mut skipped_deadlines = 0;
    while next_deadline <= now {
        next_deadline += frame_duration;
        skipped_deadlines += 1;
    }

    FrameDeadlineAdvance {
        next_deadline,
        skipped_deadlines,
    }
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
