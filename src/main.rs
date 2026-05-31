use std::{
    error::Error,
    io::{self, Write},
};

use ecosystem::{
    diagnostics::{TraceCollector, TraceEvent},
    framebuffer::{Cell, Framebuffer},
    render::build_static_landscape_frame,
    terminal::AnsiDiffEncoder,
};

const MVP_WIDTH: u16 = 80;
const MVP_HEIGHT: u16 = 24;

fn main() -> Result<(), Box<dyn Error>> {
    let mut traces = if std::env::var_os("ECOSYSTEM_TRACE").is_some() {
        TraceCollector::enabled()
    } else {
        TraceCollector::disabled()
    };

    traces.record(TraceEvent::new(
        "startup",
        "building static Phase 1 MVP frame",
    ));

    let previous = Framebuffer::new(MVP_WIDTH, MVP_HEIGHT, Cell::blank())?;
    let current = build_static_landscape_frame(MVP_WIDTH, MVP_HEIGHT)?;
    let encoded = AnsiDiffEncoder::new().encode_diff(&previous, &current)?;

    traces.record(TraceEvent::new(
        "render",
        format!("encoded {} changed cells", encoded.changed_cells),
    ));

    let mut stdout = io::stdout().lock();
    stdout.write_all(&encoded.bytes)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;

    for event in traces.snapshot() {
        eprintln!(
            "[{:>6}ms] {}: {}",
            event.elapsed.as_millis(),
            event.target,
            event.message
        );
    }

    Ok(())
}
