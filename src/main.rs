use std::{
    io::{self, IsTerminal, Write},
    process::ExitCode,
    time::{Duration, Instant},
};

use crossterm::event::{self, Event};
use ecosystem::{
    app::{StartupEnvironment, render_initial_frame},
    diagnostics::{TraceCollector, TraceEvent},
    input::{EngineAction, key_event_to_action},
    render::build_landscape_frame,
    runtime::RuntimeConfig,
    terminal::{AnsiDiffEncoder, TerminalSession, TerminalSessionOptions, current_terminal_size},
};

const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(50);

fn main() -> ExitCode {
    let mut traces = if std::env::var_os("ECOSYSTEM_TRACE").is_some() {
        TraceCollector::enabled()
    } else {
        TraceCollector::disabled()
    };

    let result = run_once(&mut traces);
    emit_traces(&traces);

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ecosystem startup failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_once(traces: &mut TraceCollector) -> Result<(), Box<dyn std::error::Error>> {
    let config = RuntimeConfig::default();
    let size = current_terminal_size()?;
    let encoded = render_initial_frame(
        StartupEnvironment::new(io::stdout().is_terminal(), size),
        traces,
    )?;
    traces.record(TraceEvent::new(
        "stdout",
        format!("writing {} bytes", encoded.bytes.len()),
    ));

    let stdout = io::stdout();
    let mut session = TerminalSession::start(stdout.lock(), TerminalSessionOptions::default())?;
    session.writer_mut().write_all(&encoded.bytes)?;
    session.writer_mut().flush()?;
    traces.record(TraceEvent::new(
        "input",
        "entering frame loop; press q or Esc to quit",
    ));

    let mut previous_frame = build_landscape_frame(size.width, size.height, 0)?;
    let mut tick = 1_u64;
    let frame_duration = config.frame_duration();
    let mut next_frame_at = Instant::now() + frame_duration;
    traces.record(TraceEvent::new(
        "frame",
        format!("targeting {} fps", config.target_fps),
    ));

    loop {
        let now = Instant::now();
        if now >= next_frame_at {
            let current_frame = build_landscape_frame(size.width, size.height, tick)?;
            let frame_output =
                AnsiDiffEncoder::new().encode_diff(&previous_frame, &current_frame)?;

            if !frame_output.bytes.is_empty() {
                session.writer_mut().write_all(&frame_output.bytes)?;
                session.writer_mut().flush()?;
            }

            if tick % u64::from(config.target_fps) == 0 {
                traces.record(TraceEvent::new(
                    "frame",
                    format!(
                        "tick {tick}: {} changed cells, {} bytes",
                        frame_output.changed_cells,
                        frame_output.bytes.len()
                    ),
                ));
            }

            previous_frame = current_frame;
            tick = tick.wrapping_add(1);
            next_frame_at = Instant::now() + frame_duration;
            continue;
        }

        let poll_timeout = (next_frame_at - now).min(INPUT_POLL_INTERVAL);
        if !event::poll(poll_timeout)? {
            continue;
        }
        let Event::Key(key_event) = event::read()? else {
            continue;
        };

        match key_event_to_action(key_event) {
            EngineAction::Quit => {
                traces.record(TraceEvent::new("input", "quit action received"));
                break;
            }
            EngineAction::None => {}
        }
    }

    session.finish()?;

    Ok(())
}

fn emit_traces(traces: &TraceCollector) {
    for event in traces.snapshot() {
        eprintln!(
            "[{:>6}ms] {}: {}",
            event.elapsed.as_millis(),
            event.target,
            event.message
        );
    }
}
