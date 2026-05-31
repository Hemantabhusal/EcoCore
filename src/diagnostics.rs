use std::time::{Duration, Instant};

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
