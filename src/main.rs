use std::{
    io::{self, IsTerminal, Write},
    process::ExitCode,
};

use ecosystem::{
    app::{StartupEnvironment, render_initial_frame},
    diagnostics::{TraceCollector, TraceEvent},
    terminal::{TerminalSession, TerminalSessionOptions, current_terminal_size},
};

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
