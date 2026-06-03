use std::{
    io::{self, IsTerminal, Write},
    process::ExitCode,
    time::{Duration, Instant},
};

use crossterm::event::{self, Event};
use ecosystem::{
    app::{StartupEnvironment, prepare_startup},
    diagnostics::{TraceCollector, TraceEvent},
    input::{EngineAction, key_event_to_action},
    kitty::{KittyGraphicsEncoder, KittyImageId},
    metrics::cpu::{CpuSampler, CpuSamplerStatus},
    metrics::disk::{DiskSampler, DiskSamplerStatus},
    metrics::memory::MemorySampler,
    metrics::network::{NetworkSampler, NetworkSamplerStatus},
    runtime::{ResizeDebouncer, ResizeDecision, RuntimeConfig},
    simulation::{ActivitySmoother, SceneActivity},
    terminal::{
        TerminalSession, TerminalSessionOptions, TerminalSize, clear_screen, current_terminal_size,
    },
    visual::{ProbeCanvasConfig, build_probe_canvas},
};

const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const ACTIVITY_SMOOTHING_RESPONSE: f32 = 0.25;
const SPIKE_IMAGE_ID: KittyImageId = KittyImageId::new(1);

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
    let startup = prepare_startup(
        StartupEnvironment::new(io::stdout().is_terminal(), size),
        traces,
    )?;
    traces.record(TraceEvent::new(
        "graphics",
        format!(
            "pixel canvas runtime prepared for {}x{} terminal",
            startup.terminal_size.width, startup.terminal_size.height
        ),
    ));

    let stdout = io::stdout();
    let mut session = TerminalSession::start(stdout.lock(), TerminalSessionOptions::default())?;
    let kitty_encoder = KittyGraphicsEncoder::new(SPIKE_IMAGE_ID);
    traces.record(TraceEvent::new(
        "input",
        "entering frame loop; press q or Esc to quit",
    ));

    let mut tick = 1_u64;
    let frame_duration = config.frame_duration();
    let mut next_frame_at = Instant::now() + frame_duration;
    let mut cpu_sampler = CpuSampler::default();
    let mut memory_sampler = MemorySampler;
    let mut network_sampler = NetworkSampler::default();
    let mut disk_sampler = DiskSampler::default();
    let mut last_network_sample_at = None;
    let mut last_disk_sample_at = None;
    let mut target_activity = SceneActivity::default();
    let mut activity_smoother = ActivitySmoother::new(ACTIVITY_SMOOTHING_RESPONSE);
    let mut next_metrics_at = Instant::now();
    let mut output_suspended = false;
    let mut resize_debouncer = ResizeDebouncer::new(config.resize_debounce);
    traces.record(TraceEvent::new(
        "frame",
        format!("targeting {} fps", config.target_fps),
    ));

    loop {
        let now = Instant::now();
        if now >= next_metrics_at {
            // Metrics are sampled below the frame rate so Linux `/proc` reads
            // do not become part of the hot render path.
            match cpu_sampler.sample_from_system(traces) {
                Ok(CpuSamplerStatus::Primed { .. }) => {}
                Ok(CpuSamplerStatus::Usage(usage)) => {
                    target_activity = target_activity.with_core_loads(usage.per_core);
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
                    target_activity = target_activity.with_memory_pressure(pressure.value);
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
                    target_activity = target_activity.with_network_flow(flow.download, flow.upload);
                    last_network_sample_at = Some(now);
                }
                Err(error) => {
                    traces.record(TraceEvent::new(
                        "metrics.network",
                        format!("sample failed: {error}"),
                    ));
                }
            }
            let disk_elapsed = last_disk_sample_at
                .map(|sampled_at| now.saturating_duration_since(sampled_at))
                .unwrap_or(Duration::ZERO);
            match disk_sampler.sample_from_system(disk_elapsed, traces) {
                Ok(DiskSamplerStatus::Primed { .. }) => {
                    last_disk_sample_at = Some(now);
                }
                Ok(DiskSamplerStatus::Activity(activity)) => {
                    target_activity =
                        target_activity.with_disk_activity(activity.read, activity.write);
                    last_disk_sample_at = Some(now);
                }
                Err(error) => {
                    traces.record(TraceEvent::new(
                        "metrics.disk",
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
                    redraw_after_resize(&mut session, size, traces)?;
                    output_suspended = false;
                    next_frame_at = Instant::now() + frame_duration;
                }
                ResizeDecision::Suspend { actual, minimum } => {
                    suspend_for_unsupported_resize(&mut session, actual, minimum, traces)?;
                    output_suspended = true;
                }
            }
            continue;
        }

        if now >= next_frame_at {
            if !output_suspended && resize_debouncer.ready_at().is_none() {
                // Metrics arrive at a lower cadence than frames. Smoothing here
                // keeps future canvas state responsive without snapping every sample tick.
                let scene_activity = activity_smoother.step_towards(&target_activity);
                let frame_started = Instant::now();
                let canvas = build_probe_canvas(
                    ProbeCanvasConfig::new(config.canvas_width, config.canvas_height),
                    tick,
                    &scene_activity,
                )?;
                let encode_started = Instant::now();
                let output = kitty_encoder.encode_canvas(&canvas);
                let encode_time = encode_started.elapsed();
                session.writer_mut().write_all(&output)?;
                session.writer_mut().flush()?;

                if tick.is_multiple_of(u64::from(config.target_fps)) {
                    traces.record(TraceEvent::new(
                        "frame",
                        format!(
                            "tick {tick}: {}x{} canvas, {} bytes sent, encode {:?}, frame {:?}",
                            canvas.width(),
                            canvas.height(),
                            output.len(),
                            encode_time,
                            frame_started.elapsed()
                        ),
                    ));
                }
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

    session
        .writer_mut()
        .write_all(&kitty_encoder.encode_delete())?;
    session.writer_mut().flush()?;
    session.finish()?;

    Ok(())
}

fn redraw_after_resize<W: Write>(
    session: &mut TerminalSession<W>,
    size: TerminalSize,
    traces: &mut TraceCollector,
) -> io::Result<()> {
    session.writer_mut().write_all(clear_screen())?;
    session
        .writer_mut()
        .write_all(&KittyGraphicsEncoder::new(SPIKE_IMAGE_ID).encode_delete())?;
    session.writer_mut().flush()?;
    traces.record(TraceEvent::new(
        "terminal.resize",
        format!("accepted resized terminal {}x{}", size.width, size.height),
    ));

    Ok(())
}

fn suspend_for_unsupported_resize<W: Write>(
    session: &mut TerminalSession<W>,
    actual: TerminalSize,
    minimum: TerminalSize,
    traces: &mut TraceCollector,
) -> io::Result<()> {
    session.writer_mut().write_all(clear_screen())?;
    session
        .writer_mut()
        .write_all(&KittyGraphicsEncoder::new(SPIKE_IMAGE_ID).encode_delete())?;
    session.writer_mut().flush()?;
    traces.record(TraceEvent::new(
        "terminal.resize",
        format!(
            "suspended output for {}x{} terminal; minimum is {}x{}",
            actual.width, actual.height, minimum.width, minimum.height
        ),
    ));

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
