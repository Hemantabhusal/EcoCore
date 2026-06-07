use std::{
    io::{self, IsTerminal, Write},
    process::ExitCode,
    time::{Duration, Instant},
};

use crossterm::event::{self, Event};
use ecosystem::{
    app::{StartupEnvironment, prepare_startup},
    diagnostics::{GraphicsFrameTrace, TraceCollector, TraceEvent},
    input::{EngineAction, key_event_to_action},
    kitty::KittyImageId,
    layout::{GraphicsLayout, graphics_layout},
    metrics::cpu::{CpuSampler, CpuSamplerStatus},
    metrics::disk::{DiskSampler, DiskSamplerStatus},
    metrics::memory::MemorySampler,
    metrics::network::{NetworkSampler, NetworkSamplerStatus},
    renderer::{KittyRenderer, KittyRendererConfig},
    runtime::{ResizeDebouncer, ResizeDecision, RuntimeConfig, advance_frame_deadline},
    simulation::{ActivitySmoother, SceneActivity},
    terminal::{
        TerminalSession, TerminalSessionOptions, TerminalSize, clear_screen, current_terminal_size,
    },
    visual::{CafeCanvasConfig, CafeScene},
};

const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const ACTIVITY_SMOOTHING_RESPONSE: f32 = 0.25;
const KITTY_IMAGE_IDS: [KittyImageId; 2] = [KittyImageId::new(1), KittyImageId::new(2)];

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
    prepare_startup(
        StartupEnvironment::new(io::stdout().is_terminal(), size),
        traces,
    )?;
    let mut graphics_layout = runtime_graphics_layout(config, size);
    traces.record(graphics_layout_trace_event(size, graphics_layout));

    let stdout = io::stdout();
    let mut session = TerminalSession::start(stdout.lock(), TerminalSessionOptions::default())?;
    let mut renderer = KittyRenderer::new(KittyRendererConfig {
        image_ids: KITTY_IMAGE_IDS,
        image_columns: config.image_columns,
        image_rows: config.image_rows,
    });
    let mut visual_scene = CafeScene::new(CafeCanvasConfig::new(
        graphics_layout.canvas_width,
        graphics_layout.canvas_height,
    ))?;
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
    let mut measurement_window_started_at = Instant::now();
    let mut measurement_window_frames = 0_u64;
    let mut measurement_window_skipped_deadlines = 0_u64;
    let mut measurement_window_interrupted = false;
    let mut measurement_window_render_time = Duration::ZERO;
    let mut measurement_window_encode_time = Duration::ZERO;
    let mut measurement_window_write_time = Duration::ZERO;
    let mut measurement_window_frame_time = Duration::ZERO;
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
                    let new_graphics_layout = runtime_graphics_layout(config, size);
                    if new_graphics_layout.canvas_width != graphics_layout.canvas_width
                        || new_graphics_layout.canvas_height != graphics_layout.canvas_height
                    {
                        visual_scene = CafeScene::new(CafeCanvasConfig::new(
                            new_graphics_layout.canvas_width,
                            new_graphics_layout.canvas_height,
                        ))?;
                    }
                    graphics_layout = new_graphics_layout;
                    redraw_after_resize(&mut session, &mut renderer, size, traces)?;
                    traces.record(graphics_layout_trace_event(size, graphics_layout));
                    output_suspended = false;
                    next_frame_at = Instant::now() + frame_duration;
                    measurement_window_started_at = Instant::now();
                    measurement_window_frames = 0;
                    measurement_window_skipped_deadlines = 0;
                    measurement_window_render_time = Duration::ZERO;
                    measurement_window_encode_time = Duration::ZERO;
                    measurement_window_write_time = Duration::ZERO;
                    measurement_window_frame_time = Duration::ZERO;
                    measurement_window_interrupted = true;
                }
                ResizeDecision::Suspend { actual, minimum } => {
                    suspend_for_unsupported_resize(
                        &mut session,
                        &mut renderer,
                        actual,
                        minimum,
                        traces,
                    )?;
                    output_suspended = true;
                    measurement_window_started_at = Instant::now();
                    measurement_window_frames = 0;
                    measurement_window_skipped_deadlines = 0;
                    measurement_window_render_time = Duration::ZERO;
                    measurement_window_encode_time = Duration::ZERO;
                    measurement_window_write_time = Duration::ZERO;
                    measurement_window_frame_time = Duration::ZERO;
                    measurement_window_interrupted = true;
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
                let canvas = visual_scene.render(tick, scene_activity);
                let encode_started = Instant::now();
                let render_time = encode_started.duration_since(frame_started);
                let frame = renderer.render_frame(size, canvas);
                let write_started = Instant::now();
                let encode_time = write_started.duration_since(encode_started);
                session.writer_mut().write_all(&frame.bytes)?;
                session.writer_mut().flush()?;
                let write_time = write_started.elapsed();
                let frame_time = frame_started.elapsed();
                measurement_window_frames += 1;
                measurement_window_render_time += render_time;
                measurement_window_encode_time += encode_time;
                measurement_window_write_time += write_time;
                measurement_window_frame_time += frame_time;

                if tick.is_multiple_of(u64::from(config.target_fps)) {
                    let renderer_stats = renderer.stats();
                    traces.record(
                        GraphicsFrameTrace {
                            tick,
                            canvas_width: canvas.width(),
                            canvas_height: canvas.height(),
                            placement: frame.placement,
                            image_id: frame.image_id,
                            deleted_image_id: frame.deleted_image_id,
                            frame_bytes: frame.bytes.len(),
                            full_frame_bytes: renderer_stats.full_frame_bytes(),
                            partial_frame_bytes: renderer_stats.partial_frame_bytes(),
                            average_frame_bytes: renderer_stats.average_frame_bytes(),
                            total_protocol_bytes: renderer_stats.total_protocol_bytes(),
                            skipped_deadlines: measurement_window_skipped_deadlines,
                            interrupted: measurement_window_interrupted,
                            dirty_summary: frame.dirty_summary,
                            render_time,
                            encode_time,
                            write_time,
                            frame_time,
                            average_render_time: average_duration(
                                measurement_window_render_time,
                                measurement_window_frames,
                            ),
                            average_encode_time: average_duration(
                                measurement_window_encode_time,
                                measurement_window_frames,
                            ),
                            average_write_time: average_duration(
                                measurement_window_write_time,
                                measurement_window_frames,
                            ),
                            average_frame_time: average_duration(
                                measurement_window_frame_time,
                                measurement_window_frames,
                            ),
                            frames_in_window: measurement_window_frames,
                            window_elapsed: measurement_window_started_at.elapsed(),
                        }
                        .to_trace_event(),
                    );
                    measurement_window_started_at = Instant::now();
                    measurement_window_frames = 0;
                    measurement_window_skipped_deadlines = 0;
                    measurement_window_render_time = Duration::ZERO;
                    measurement_window_encode_time = Duration::ZERO;
                    measurement_window_write_time = Duration::ZERO;
                    measurement_window_frame_time = Duration::ZERO;
                    measurement_window_interrupted = false;
                }
            }

            tick = tick.wrapping_add(1);
            let deadline_advance =
                advance_frame_deadline(next_frame_at, frame_duration, Instant::now());
            measurement_window_skipped_deadlines += deadline_advance.skipped_deadlines;
            next_frame_at = deadline_advance.next_deadline;
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

    session.writer_mut().write_all(&renderer.reset())?;
    session.writer_mut().flush()?;
    session.finish()?;

    Ok(())
}

fn redraw_after_resize<W: Write>(
    session: &mut TerminalSession<W>,
    renderer: &mut KittyRenderer,
    size: TerminalSize,
    traces: &mut TraceCollector,
) -> io::Result<()> {
    session.writer_mut().write_all(clear_screen())?;
    session.writer_mut().write_all(&renderer.reset())?;
    session.writer_mut().flush()?;
    traces.record(TraceEvent::new(
        "terminal.resize",
        format!("accepted resized terminal {}x{}", size.width, size.height),
    ));

    Ok(())
}

fn runtime_graphics_layout(config: RuntimeConfig, size: TerminalSize) -> GraphicsLayout {
    graphics_layout(
        size,
        config.image_columns,
        config.image_rows,
        config.cell_size,
    )
}

fn graphics_layout_trace_event(size: TerminalSize, layout: GraphicsLayout) -> TraceEvent {
    TraceEvent::new(
        "graphics.layout",
        format!(
            "{}x{} terminal -> {}x{} cells at {},{}, {}x{} canvas",
            size.width,
            size.height,
            layout.placement.columns,
            layout.placement.rows,
            layout.placement.cursor_column,
            layout.placement.cursor_row,
            layout.canvas_width,
            layout.canvas_height
        ),
    )
}

fn suspend_for_unsupported_resize<W: Write>(
    session: &mut TerminalSession<W>,
    renderer: &mut KittyRenderer,
    actual: TerminalSize,
    minimum: TerminalSize,
    traces: &mut TraceCollector,
) -> io::Result<()> {
    session.writer_mut().write_all(clear_screen())?;
    session.writer_mut().write_all(&renderer.reset())?;
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

fn average_duration(total: Duration, count: u64) -> Duration {
    match u32::try_from(count) {
        Ok(0) | Err(_) => Duration::ZERO,
        Ok(count) => total / count,
    }
}
