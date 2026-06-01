use std::{
    io::{self, IsTerminal, Write},
    process::ExitCode,
    time::{Duration, Instant},
};

use crossterm::event::{self, Event};
use ecosystem::{
    app::{StartupEnvironment, render_initial_frame},
    diagnostics::{TraceCollector, TraceEvent},
    framebuffer::{Cell, Framebuffer},
    input::{EngineAction, key_event_to_action},
    metrics::cpu::{CpuSampler, CpuSamplerStatus},
    metrics::memory::MemorySampler,
    metrics::network::{NetworkSampler, NetworkSamplerStatus},
    render::{SceneActivity, build_landscape_frame, build_landscape_frame_with_activity},
    runtime::{FrameStats, ResizeDebouncer, ResizeDecision, RuntimeConfig},
    terminal::{
        AnsiDiffEncoder, TerminalSession, TerminalSessionOptions, TerminalSize, clear_screen,
        current_terminal_size,
    },
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
    let mut size = current_terminal_size()?;
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

    // Keep the last rendered frame so animation frames can be emitted as ANSI
    // diffs. This is the core performance contract for the terminal renderer.
    let mut previous_frame = build_landscape_frame(size.width, size.height, 0)?;
    let mut tick = 1_u64;
    let frame_duration = config.frame_duration();
    let mut next_frame_at = Instant::now() + frame_duration;
    let mut cpu_sampler = CpuSampler::default();
    let mut memory_sampler = MemorySampler;
    let mut network_sampler = NetworkSampler::default();
    let mut last_network_sample_at = None;
    let mut scene_activity = SceneActivity::default();
    let mut next_metrics_at = Instant::now();
    let mut rendering_suspended = false;
    let mut resize_debouncer = ResizeDebouncer::new(config.resize_debounce);
    let mut frame_stats = FrameStats::default();
    traces.record(TraceEvent::new(
        "frame",
        format!("targeting {} fps", config.target_fps),
    ));

    loop {
        let now = Instant::now();
        if now >= next_metrics_at {
            // Metrics are sampled below the frame rate so `/proc/stat` reads do
            // not become part of the hot render path.
            match cpu_sampler.sample_from_system(traces) {
                Ok(CpuSamplerStatus::Primed { .. }) => {}
                Ok(CpuSamplerStatus::Usage(usage)) => {
                    scene_activity = scene_activity.with_core_loads(usage.per_core);
                }
                Err(error) => {
                    traces.record(TraceEvent::new(
                        "metrics.cpu",
                        format!("sample failed: {error}"),
                    ));
                }
            }
            match memory_sampler.sample_from_system(traces) {
                Ok(pressure) => {
                    scene_activity = scene_activity.with_memory_pressure(pressure.value);
                }
                Err(error) => {
                    traces.record(TraceEvent::new(
                        "metrics.memory",
                        format!("sample failed: {error}"),
                    ));
                }
            }
            let network_elapsed = last_network_sample_at
                .map(|sampled_at| now.saturating_duration_since(sampled_at))
                .unwrap_or(Duration::ZERO);
            match network_sampler.sample_from_system(network_elapsed, traces) {
                Ok(NetworkSamplerStatus::Primed { .. }) => {
                    last_network_sample_at = Some(now);
                }
                Ok(NetworkSamplerStatus::Flow(flow)) => {
                    scene_activity = scene_activity.with_network_flow(flow.download, flow.upload);
                    last_network_sample_at = Some(now);
                }
                Err(error) => {
                    traces.record(TraceEvent::new(
                        "metrics.network",
                        format!("sample failed: {error}"),
                    ));
                }
            }
            next_metrics_at = now + config.metrics_sample_interval;
        }

        if let Some(decision) = resize_debouncer.take_due(now) {
            match decision {
                ResizeDecision::Redraw { size: new_size } => {
                    size = new_size;
                    previous_frame =
                        redraw_resized_frame(&mut session, size, tick, &scene_activity, traces)?;
                    rendering_suspended = false;
                    next_frame_at = Instant::now() + frame_duration;
                }
                ResizeDecision::Suspend { actual, minimum } => {
                    render_resize_suspended_message(&mut session, actual, minimum, traces)?;
                    rendering_suspended = true;
                }
            }
            continue;
        }

        if now >= next_frame_at {
            if !rendering_suspended && resize_debouncer.ready_at().is_none() {
                let current_frame = build_landscape_frame_with_activity(
                    size.width,
                    size.height,
                    tick,
                    &scene_activity,
                )?;
                let frame_output =
                    AnsiDiffEncoder::new().encode_diff(&previous_frame, &current_frame)?;

                if !frame_output.bytes.is_empty() {
                    session.writer_mut().write_all(&frame_output.bytes)?;
                    session.writer_mut().flush()?;
                }

                frame_stats.record_frame(frame_output.changed_cells, frame_output.bytes.len());
                if tick.is_multiple_of(u64::from(config.target_fps)) {
                    let summary = frame_stats.take_summary();
                    traces.record(TraceEvent::new(
                        "frame",
                        format!(
                            "tick {tick}: {} changed cells, {} bytes; {summary}",
                            frame_output.changed_cells,
                            frame_output.bytes.len()
                        ),
                    ));
                }

                previous_frame = current_frame;
            }

            tick = tick.wrapping_add(1);
            next_frame_at = Instant::now() + frame_duration;
            continue;
        }

        // Poll only until the next frame deadline so input remains responsive
        // without waking the process unnecessarily between animation ticks.
        let mut poll_timeout = (next_frame_at - now).min(INPUT_POLL_INTERVAL);
        if let Some(ready_at) = resize_debouncer.ready_at() {
            poll_timeout = poll_timeout.min(ready_at.saturating_duration_since(now));
        }
        if !event::poll(poll_timeout)? {
            continue;
        }

        match event::read()? {
            Event::Key(key_event) => match key_event_to_action(key_event) {
                EngineAction::Quit => {
                    traces.record(TraceEvent::new("input", "quit action received"));
                    break;
                }
                EngineAction::None => {}
            },
            Event::Resize(width, height) => {
                let new_size = TerminalSize::new(width, height);
                // Terminal emulators can emit many resize events while the
                // user drags a window edge. Queue the latest size and redraw
                // once after the burst settles.
                resize_debouncer.observe(new_size, Instant::now());
                traces.record(TraceEvent::new(
                    "terminal.resize",
                    format!("queued resize to {}x{}", new_size.width, new_size.height),
                ));
            }
            _ => {}
        }
    }

    session.finish()?;

    Ok(())
}

fn redraw_resized_frame<W: Write>(
    session: &mut TerminalSession<W>,
    size: TerminalSize,
    tick: u64,
    scene_activity: &SceneActivity,
    traces: &mut TraceCollector,
) -> Result<Framebuffer, Box<dyn std::error::Error>> {
    let blank = Framebuffer::new(size.width, size.height, Cell::blank())?;
    let current =
        build_landscape_frame_with_activity(size.width, size.height, tick, scene_activity)?;
    let frame_output = AnsiDiffEncoder::new().encode_diff(&blank, &current)?;

    // A resize invalidates the terminal's existing cell grid, so the next draw
    // must clear and repaint the full framebuffer instead of applying a diff
    // against the old dimensions.
    session.writer_mut().write_all(clear_screen())?;
    session.writer_mut().write_all(&frame_output.bytes)?;
    session.writer_mut().flush()?;
    traces.record(TraceEvent::new(
        "terminal.resize",
        format!(
            "redrew {}x{} frame with {} changed cells",
            size.width, size.height, frame_output.changed_cells
        ),
    ));

    Ok(current)
}

fn render_resize_suspended_message<W: Write>(
    session: &mut TerminalSession<W>,
    actual: TerminalSize,
    minimum: TerminalSize,
    traces: &mut TraceCollector,
) -> io::Result<()> {
    let message = format!(
        "terminal too small: got {}x{}, minimum is {}x{}",
        actual.width, actual.height, minimum.width, minimum.height
    );
    session.writer_mut().write_all(clear_screen())?;
    session.writer_mut().write_all(message.as_bytes())?;
    session.writer_mut().flush()?;
    traces.record(TraceEvent::new("terminal.resize", message));

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
