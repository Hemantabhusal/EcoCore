use std::time::{Duration, Instant};

use crate::{kitty::KittyImageId, layout::ImagePlacement};

#[derive(Clone, Debug)]
pub struct TraceEvent {
    pub target: String,
    pub message: String,
    pub elapsed: Duration,
}

impl TraceEvent {
    pub fn new(target: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            message: message.into(),
            elapsed: Duration::ZERO,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TraceCollector {
    enabled: bool,
    started_at: Instant,
    events: Vec<TraceEvent>,
}

impl TraceCollector {
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            started_at: Instant::now(),
            events: Vec::new(),
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            started_at: Instant::now(),
            events: Vec::new(),
        }
    }

    pub fn record(&mut self, mut event: TraceEvent) {
        if !self.enabled {
            return;
        }

        event.elapsed = self.started_at.elapsed();
        self.events.push(event);
    }

    pub fn snapshot(&self) -> Vec<TraceEvent> {
        self.events.clone()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphicsFrameTrace {
    pub tick: u64,
    pub canvas_width: u16,
    pub canvas_height: u16,
    pub placement: ImagePlacement,
    pub image_id: KittyImageId,
    pub deleted_image_id: Option<KittyImageId>,
    pub frame_bytes: usize,
    pub average_frame_bytes: u64,
    pub total_protocol_bytes: u64,
    pub skipped_deadlines: u64,
    pub interrupted: bool,
    pub encode_time: Duration,
    pub frame_time: Duration,
    pub frames_in_window: u64,
    pub window_elapsed: Duration,
}

impl GraphicsFrameTrace {
    pub fn to_trace_event(self) -> TraceEvent {
        TraceEvent::new("graphics.frame", self.message())
    }

    fn message(self) -> String {
        format!(
            "tick {}: {}x{} canvas, {}x{} cells at {},{}, {:.1} fps, image {}, deleted {}, {} bytes sent, avg {} bytes/frame, {} protocol bytes total, skipped {} deadlines, interrupted {}, encode {}us, frame {}us",
            self.tick,
            self.canvas_width,
            self.canvas_height,
            self.placement.columns,
            self.placement.rows,
            self.placement.cursor_column,
            self.placement.cursor_row,
            self.frames_per_second(),
            self.image_id.value(),
            format_deleted_image_id(self.deleted_image_id),
            self.frame_bytes,
            self.average_frame_bytes,
            self.total_protocol_bytes,
            self.skipped_deadlines,
            format_interrupted(self.interrupted),
            self.encode_time.as_micros(),
            self.frame_time.as_micros()
        )
    }

    fn frames_per_second(self) -> f64 {
        let elapsed = self.window_elapsed.as_secs_f64();
        if elapsed <= 0.0 {
            return 0.0;
        }

        self.frames_in_window as f64 / elapsed
    }
}

fn format_deleted_image_id(image_id: Option<KittyImageId>) -> String {
    image_id.map_or_else(
        || "none".to_owned(),
        |image_id| image_id.value().to_string(),
    )
}

fn format_interrupted(interrupted: bool) -> &'static str {
    if interrupted { "yes" } else { "no" }
}
