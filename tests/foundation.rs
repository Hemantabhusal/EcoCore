use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ecosystem::{
    app::{StartupEnvironment, prepare_startup},
    canvas::{Canvas, CanvasError, DirtyRegion, Rgba},
    diagnostics::{GraphicsFrameTrace, TraceCollector, TraceEvent},
    input::{EngineAction, key_event_to_action},
    kitty::KittyImageId,
    layout::{CellSize, ImagePlacement},
    runtime::{
        ResizeDebouncer, ResizeDecision, RuntimeConfig, advance_frame_deadline, resize_decision,
        target_frame_duration,
    },
    simulation::{ActivitySmoother, SceneActivity},
    terminal::{
        ColorCapability, TerminalColorEnvironment, TerminalGraphicsEnvironment, TerminalSession,
        TerminalSessionOptions, TerminalSize, detect_color_capability,
        summarize_graphics_environment, validate_terminal_environment,
    },
};

#[test]
fn canvas_rejects_zero_sized_surfaces() {
    let error =
        Canvas::new(0, 12, Rgba::TRANSPARENT).expect_err("zero-width canvases cannot be rendered");

    assert_eq!(
        error,
        CanvasError::InvalidSize {
            width: 0,
            height: 12
        }
    );
}

#[test]
fn canvas_stores_pixels_and_tracks_dirty_region() {
    let mut canvas = Canvas::new(4, 3, Rgba::rgb(1, 2, 3)).expect("valid canvas");
    let hot = Rgba::new(200, 80, 40, 220);

    canvas.set_pixel(1, 1, hot).expect("pixel in bounds");
    canvas
        .set_pixel(3, 2, Rgba::rgb(9, 8, 7))
        .expect("pixel in bounds");

    assert_eq!(canvas.width(), 4);
    assert_eq!(canvas.height(), 3);
    assert_eq!(canvas.pixel(1, 1), Some(hot));
    assert_eq!(
        canvas.dirty_region(),
        Some(DirtyRegion {
            x: 1,
            y: 1,
            width: 3,
            height: 2
        })
    );
}

#[test]
fn canvas_fill_marks_entire_surface_dirty_and_can_clear_dirty_state() {
    let mut canvas = Canvas::new(3, 2, Rgba::TRANSPARENT).expect("valid canvas");
    let fill = Rgba::rgb(10, 20, 30);

    canvas.fill(fill);

    assert!(canvas.pixels().iter().all(|pixel| *pixel == fill));
    assert_eq!(
        canvas.dirty_region(),
        Some(DirtyRegion {
            x: 0,
            y: 0,
            width: 3,
            height: 2
        })
    );

    canvas.clear_dirty();
    assert_eq!(canvas.dirty_region(), None);
}

#[test]
fn rgba_blend_over_composites_source_alpha_over_opaque_background() {
    let source = Rgba::new(200, 100, 0, 128);
    let background = Rgba::rgb(20, 40, 80);

    assert_eq!(source.blend_over(background), Rgba::new(110, 70, 40, 255));
}

#[test]
fn trace_collector_records_critical_development_events_when_enabled() {
    let mut traces = TraceCollector::enabled();

    traces.record(TraceEvent::new("canvas", "allocated 120x80 RGBA surface"));
    traces.record(TraceEvent::new("terminal", "startup validation passed"));

    let snapshot = traces.snapshot();
    assert_eq!(snapshot.len(), 2);
    assert_eq!(snapshot[0].target, "canvas");
    assert!(snapshot[0].message.contains("RGBA"));
    assert_eq!(snapshot[1].target, "terminal");
}

#[test]
fn graphics_frame_trace_formats_measurement_snapshot_for_terminal_runs() {
    let trace = GraphicsFrameTrace {
        tick: 30,
        canvas_width: 240,
        canvas_height: 135,
        placement: ImagePlacement {
            cursor_column: 46,
            cursor_row: 16,
            columns: 30,
            rows: 10,
        },
        image_id: KittyImageId::new(2),
        deleted_image_id: Some(KittyImageId::new(1)),
        frame_bytes: 173_152,
        average_frame_bytes: 172_900,
        total_protocol_bytes: 5_187_000,
        skipped_deadlines: 2,
        interrupted: true,
        encode_time: Duration::from_micros(2_400),
        frame_time: Duration::from_micros(3_100),
        frames_in_window: 30,
        window_elapsed: Duration::from_millis(1_000),
    };

    let event = trace.to_trace_event();

    assert_eq!(event.target, "graphics.frame");
    assert_eq!(
        event.message,
        "tick 30: 240x135 canvas, 30x10 cells at 46,16, 30.0 fps, image 2, deleted 1, 173152 bytes sent, avg 172900 bytes/frame, 5187000 protocol bytes total, skipped 2 deadlines, interrupted yes, encode 2400us, frame 3100us"
    );
}

#[test]
fn terminal_validation_rejects_non_tty_stdout() {
    let error = validate_terminal_environment(false, TerminalSize::new(120, 40))
        .expect_err("non-tty stdout must be rejected");

    assert_eq!(
        error.to_string(),
        "stdout is not a terminal; run ecosystem directly in an interactive terminal"
    );
}

#[test]
fn terminal_validation_rejects_small_terminal() {
    let error = validate_terminal_environment(true, TerminalSize::new(79, 24))
        .expect_err("terminal width below minimum must be rejected");

    assert_eq!(
        error.to_string(),
        "terminal is too small: got 79x24, minimum is 80x24"
    );
}

#[test]
fn terminal_validation_accepts_minimum_supported_size() {
    validate_terminal_environment(true, TerminalSize::new(80, 24))
        .expect("minimum terminal size is supported");
}

#[test]
fn terminal_size_converts_from_terminal_probe_tuple() {
    let size = TerminalSize::from((132, 43));

    assert_eq!(size.width, 132);
    assert_eq!(size.height, 43);
}

#[test]
fn terminal_color_detection_accepts_common_truecolor_markers() {
    let capability = detect_color_capability(&TerminalColorEnvironment::new(
        Some("truecolor"),
        Some("xterm-kitty"),
    ));

    assert_eq!(capability, ColorCapability::Truecolor);

    let capability = detect_color_capability(&TerminalColorEnvironment::new(
        Some("24bit"),
        Some("screen-256color"),
    ));

    assert_eq!(capability, ColorCapability::Truecolor);
}

#[test]
fn terminal_color_detection_treats_missing_truecolor_markers_as_limited() {
    let capability =
        detect_color_capability(&TerminalColorEnvironment::new(None, Some("xterm-256color")));

    assert_eq!(capability, ColorCapability::Limited);
}

#[test]
fn terminal_graphics_environment_summary_reports_kitty_hints_without_claiming_support() {
    let environment =
        TerminalGraphicsEnvironment::new(Some("xterm-kitty"), Some("truecolor"), true, true);

    let summary = summarize_graphics_environment(&environment);

    assert_eq!(
        summary,
        "target kitty protocol; TERM=xterm-kitty; COLORTERM=truecolor; kitty env hints yes; support still requires successful graphics frames"
    );
}

#[test]
fn terminal_graphics_environment_summary_sanitizes_control_characters() {
    let environment = TerminalGraphicsEnvironment::new(
        Some("xterm-kitty\u{1b}[31m"),
        Some("truecolor\n"),
        false,
        false,
    );

    let summary = summarize_graphics_environment(&environment);

    assert!(summary.contains("TERM=xterm-kitty?"));
    assert!(summary.contains("COLORTERM=truecolor?"));
}

#[test]
fn app_startup_returns_user_facing_startup_error_for_non_tty() {
    let mut traces = TraceCollector::enabled();
    let error = prepare_startup(
        StartupEnvironment::new(false, TerminalSize::new(120, 40)),
        &mut traces,
    )
    .expect_err("non-tty startup must fail before terminal mode");

    assert_eq!(
        error.to_string(),
        "stdout is not a terminal; run ecosystem directly in an interactive terminal"
    );
    assert_eq!(traces.snapshot()[0].target, "startup");
}

#[test]
fn app_startup_validates_environment_without_emitting_legacy_art_frame() {
    let mut traces = TraceCollector::enabled();
    let report = prepare_startup(
        StartupEnvironment::new(true, TerminalSize::new(80, 24)),
        &mut traces,
    )
    .expect("valid startup prepares the graphics runtime");

    assert_eq!(report.terminal_size, TerminalSize::new(80, 24));
    assert!(
        traces
            .snapshot()
            .iter()
            .any(|event| event.target == "startup" && event.message.contains("pixel canvas"))
    );
}

#[test]
fn app_startup_records_limited_color_warning_without_failing() {
    let mut traces = TraceCollector::enabled();
    let report = prepare_startup(
        StartupEnvironment::new(true, TerminalSize::new(80, 24))
            .with_color_environment(TerminalColorEnvironment::new(None, Some("xterm-256color"))),
        &mut traces,
    )
    .expect("limited color does not block startup");

    assert_eq!(report.color_capability, ColorCapability::Limited);
    assert!(
        traces
            .snapshot()
            .iter()
            .any(|event| { event.target == "terminal.color" && event.message.contains("limited") })
    );
}

#[test]
fn app_startup_records_terminal_graphics_environment_hints() {
    let mut traces = TraceCollector::enabled();
    prepare_startup(
        StartupEnvironment::new(true, TerminalSize::new(80, 24)).with_graphics_environment(
            TerminalGraphicsEnvironment::new(Some("xterm-kitty"), Some("truecolor"), true, false),
        ),
        &mut traces,
    )
    .expect("valid startup records graphics environment");

    assert!(traces.snapshot().iter().any(|event| {
        event.target == "terminal.graphics"
            && event.message.contains("TERM=xterm-kitty")
            && event.message.contains("kitty env hints yes")
            && event
                .message
                .contains("support still requires successful graphics frames")
    }));
}

#[test]
fn terminal_session_start_enters_alternate_screen_and_hides_cursor() {
    let mut output = Vec::new();

    {
        let _session = TerminalSession::start(
            &mut output,
            TerminalSessionOptions {
                enable_raw_mode: false,
            },
        )
        .expect("memory-backed session starts");
    }

    let encoded = String::from_utf8(output).expect("terminal controls are utf8");
    assert!(encoded.starts_with("\u{1b}[?1049h\u{1b}[?25l\u{1b}[2J\u{1b}[H"));
}

#[test]
fn terminal_session_drop_restores_cursor_style_and_main_screen() {
    let mut output = Vec::new();

    {
        let mut session = TerminalSession::start(
            &mut output,
            TerminalSessionOptions {
                enable_raw_mode: false,
            },
        )
        .expect("memory-backed session starts");
        session.writer_mut().extend_from_slice(b"frame bytes");
    }

    let encoded = String::from_utf8(output).expect("terminal controls are utf8");
    assert!(encoded.contains("frame bytes"));
    assert!(encoded.ends_with("\u{1b}[0m\u{1b}[?25h\u{1b}[?1049l"));
}

#[test]
fn input_maps_q_and_escape_to_quit() {
    assert_eq!(
        key_event_to_action(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
        EngineAction::Quit
    );
    assert_eq!(
        key_event_to_action(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
        EngineAction::Quit
    );
}

#[test]
fn input_ignores_non_quit_keys() {
    assert_eq!(
        key_event_to_action(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
        EngineAction::None
    );
}

#[test]
fn runtime_default_targets_thirty_frames_per_second() {
    let config = RuntimeConfig::default();

    assert_eq!(config.target_fps, 30);
    assert_eq!(config.frame_duration(), target_frame_duration(30));
    assert_eq!(config.metrics_sample_interval.as_millis(), 500);
    assert_eq!(config.resize_debounce.as_millis(), 50);
    assert_eq!(config.image_columns, 42);
    assert_eq!(config.image_rows, 14);
    assert_eq!(config.cell_size, CellSize::new(8, 16));
}

#[test]
fn frame_deadline_advances_from_previous_deadline_instead_of_frame_finish_time() {
    let start = Instant::now();
    let frame_duration = Duration::from_millis(33);
    let previous_deadline = start + frame_duration;
    let frame_finished_at = previous_deadline + Duration::from_millis(20);

    let advance = advance_frame_deadline(previous_deadline, frame_duration, frame_finished_at);

    assert_eq!(advance.next_deadline, previous_deadline + frame_duration);
    assert_eq!(advance.skipped_deadlines, 0);
}

#[test]
fn frame_deadline_skips_missed_deadlines_after_large_overrun() {
    let start = Instant::now();
    let frame_duration = Duration::from_millis(33);
    let previous_deadline = start + frame_duration;
    let frame_finished_at = previous_deadline + Duration::from_millis(90);

    let advance = advance_frame_deadline(previous_deadline, frame_duration, frame_finished_at);

    assert_eq!(
        advance.next_deadline,
        previous_deadline + frame_duration * 3
    );
    assert_eq!(advance.skipped_deadlines, 2);
}

#[test]
fn scene_activity_clamps_core_loads_to_normalized_range() {
    let activity = SceneActivity::from_core_loads(vec![-0.25, 1.40]);

    assert_eq!(activity.core_loads(), &[0.0, 1.0]);
}

#[test]
fn scene_activity_clamps_memory_pressure_to_normalized_range() {
    let activity = SceneActivity::default().with_memory_pressure(1.40);

    assert_eq!(activity.memory_pressure(), 1.0);

    let activity = SceneActivity::default().with_memory_pressure(-0.25);

    assert_eq!(activity.memory_pressure(), 0.0);
}

#[test]
fn activity_smoother_moves_world_signals_toward_target_without_snapping() {
    let target = SceneActivity::from_core_loads(vec![1.0, 0.5])
        .with_memory_pressure(1.0)
        .with_network_flow(0.8, 0.2)
        .with_disk_activity(0.6, 0.4);
    let mut smoother = ActivitySmoother::new(0.25);

    let first: &SceneActivity = smoother.step_towards(&target);

    assert_eq!(first.core_loads(), &[0.25, 0.125]);
    assert!((first.memory_pressure() - 0.25).abs() < f32::EPSILON);
    assert!((first.network_download() - 0.20).abs() < f32::EPSILON);
    assert!((first.network_upload() - 0.05).abs() < f32::EPSILON);
    assert!((first.disk_read() - 0.15).abs() < f32::EPSILON);
    assert!((first.disk_write() - 0.10).abs() < f32::EPSILON);

    let second: &SceneActivity = smoother.step_towards(&target);

    assert!((second.core_loads()[0] - 0.4375).abs() < f32::EPSILON);
    assert!((second.memory_pressure() - 0.4375).abs() < f32::EPSILON);
}

#[test]
fn activity_smoother_clamps_response_to_valid_range() {
    let target = SceneActivity::from_core_loads(vec![0.8])
        .with_memory_pressure(0.6)
        .with_network_flow(0.4, 0.2)
        .with_disk_activity(0.3, 0.1);
    let mut smoother = ActivitySmoother::new(4.0);

    let current: &SceneActivity = smoother.step_towards(&target);

    assert_eq!(current, &target);
}

#[test]
fn resize_decision_redraws_when_new_size_is_supported() {
    let decision = resize_decision(TerminalSize::new(120, 40));

    assert_eq!(
        decision,
        ResizeDecision::Redraw {
            size: TerminalSize::new(120, 40)
        }
    );
}

#[test]
fn resize_decision_suspends_when_new_size_is_too_small() {
    let decision = resize_decision(TerminalSize::new(60, 20));

    assert_eq!(
        decision,
        ResizeDecision::Suspend {
            actual: TerminalSize::new(60, 20),
            minimum: TerminalSize::new(80, 24)
        }
    );
}

#[test]
fn resize_debouncer_coalesces_rapid_events_to_latest_size() {
    let mut debouncer = ResizeDebouncer::new(Duration::from_millis(50));
    let start = Instant::now();

    debouncer.observe(TerminalSize::new(100, 30), start);
    debouncer.observe(
        TerminalSize::new(120, 40),
        start + Duration::from_millis(10),
    );

    assert_eq!(debouncer.take_due(start + Duration::from_millis(40)), None);
    assert_eq!(
        debouncer.take_due(start + Duration::from_millis(60)),
        Some(ResizeDecision::Redraw {
            size: TerminalSize::new(120, 40)
        })
    );
    assert_eq!(debouncer.take_due(start + Duration::from_millis(80)), None);
}
