use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ecosystem::{
    app::{StartupEnvironment, render_initial_frame},
    diagnostics::{TraceCollector, TraceEvent},
    framebuffer::{Cell, Color, Framebuffer},
    input::{EngineAction, key_event_to_action},
    render::{
        VisualTheme, build_landscape_frame, build_landscape_frame_with_activity,
        build_static_landscape_frame,
    },
    runtime::{
        FrameStats, ResizeDebouncer, ResizeDecision, RuntimeConfig, resize_decision,
        target_frame_duration,
    },
    simulation::{ActivitySmoother, SceneActivity},
    terminal::{
        AnsiDiffEncoder, ColorCapability, TerminalColorEnvironment, TerminalSession,
        TerminalSessionOptions, TerminalSize, detect_color_capability,
        validate_terminal_environment,
    },
};

#[test]
fn framebuffer_set_updates_exact_cell_without_touching_neighbors() {
    let mut framebuffer = Framebuffer::new(3, 2, Cell::blank()).expect("valid framebuffer");
    let creature = Cell::new('@', Color::rgb(255, 180, 80), Color::rgb(0, 0, 0));

    framebuffer.set(1, 0, creature).expect("cell in bounds");

    assert_eq!(framebuffer.get(1, 0), Some(&creature));
    assert_eq!(framebuffer.get(0, 0), Some(&Cell::blank()));
    assert_eq!(framebuffer.get(2, 1), Some(&Cell::blank()));
}

#[test]
fn ansi_diff_encoder_emits_only_changed_cells() {
    let mut previous = Framebuffer::new(3, 1, Cell::blank()).expect("valid previous buffer");
    let mut current = Framebuffer::new(3, 1, Cell::blank()).expect("valid current buffer");
    let water = Cell::new('~', Color::rgb(40, 120, 220), Color::rgb(0, 0, 0));

    previous.set(0, 0, water).expect("cell in bounds");
    current.set(0, 0, water).expect("cell in bounds");
    current
        .set(
            2,
            0,
            Cell::new('o', Color::rgb(255, 180, 80), Color::rgb(0, 0, 0)),
        )
        .expect("cell in bounds");

    let output = AnsiDiffEncoder::new()
        .encode_diff(&previous, &current)
        .expect("matching buffer sizes");

    assert_eq!(output.changed_cells, 1);
    let encoded = String::from_utf8(output.bytes).expect("ansi output is utf8");
    assert!(encoded.contains("\u{1b}[1;3H"));
    assert!(encoded.contains('o'));
    assert!(!encoded.contains("\u{1b}[1;1H~"));
}

#[test]
fn trace_collector_records_critical_development_events_when_enabled() {
    let mut traces = TraceCollector::enabled();

    traces.record(TraceEvent::new(
        "render",
        "framebuffer diff produced 1 changed cell",
    ));
    traces.record(TraceEvent::new("terminal", "startup validation passed"));

    let snapshot = traces.snapshot();
    assert_eq!(snapshot.len(), 2);
    assert_eq!(snapshot[0].target, "render");
    assert!(snapshot[0].message.contains("changed cell"));
    assert_eq!(snapshot[1].target, "terminal");
}

#[test]
fn static_landscape_frame_contains_ground_water_and_a_visible_creature() {
    let frame = build_static_landscape_frame(20, 8).expect("valid static frame");
    let theme = VisualTheme::default();

    assert_eq!(frame.width(), 20);
    assert_eq!(frame.height(), 8);
    assert_eq!(frame.get(0, 7).expect("ground cell").glyph, theme.ground);
    assert_eq!(
        frame.get(10, 6).expect("water cell").glyph,
        theme.water_idle
    );
    assert_eq!(
        frame.get(10, 4).expect("creature cell").glyph,
        theme.creature_idle
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
fn app_initial_frame_returns_user_facing_startup_error_for_non_tty() {
    let mut traces = TraceCollector::enabled();
    let error = render_initial_frame(
        StartupEnvironment::new(false, TerminalSize::new(120, 40)),
        &mut traces,
    )
    .expect_err("non-tty startup must fail before rendering");

    assert_eq!(
        error.to_string(),
        "stdout is not a terminal; run ecosystem directly in an interactive terminal"
    );
    assert_eq!(traces.snapshot()[0].target, "startup");
}

#[test]
fn app_initial_frame_uses_terminal_size_for_rendered_frame() {
    let mut traces = TraceCollector::enabled();
    let output = render_initial_frame(
        StartupEnvironment::new(true, TerminalSize::new(80, 24)),
        &mut traces,
    )
    .expect("valid startup renders a frame");

    assert_eq!(output.changed_cells, 80 * 24);
    assert!(
        traces
            .snapshot()
            .iter()
            .any(|event| event.message.contains("encoded 1920 changed cells"))
    );
}

#[test]
fn app_initial_frame_records_limited_color_warning_without_failing() {
    let mut traces = TraceCollector::enabled();
    let output = render_initial_frame(
        StartupEnvironment::new(true, TerminalSize::new(80, 24))
            .with_color_environment(TerminalColorEnvironment::new(None, Some("xterm-256color"))),
        &mut traces,
    )
    .expect("limited color does not block startup");

    assert_eq!(output.changed_cells, 80 * 24);
    assert!(
        traces
            .snapshot()
            .iter()
            .any(|event| { event.target == "terminal.color" && event.message.contains("limited") })
    );
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
}

#[test]
fn animated_landscape_changes_incrementally_between_ticks() {
    let previous = build_landscape_frame(20, 8, 0).expect("valid previous frame");
    let current = build_landscape_frame(20, 8, 2).expect("valid current frame");

    let output = AnsiDiffEncoder::new()
        .encode_diff(&previous, &current)
        .expect("matching frames");

    assert!(output.changed_cells > 0);
    assert!(
        output.changed_cells < usize::from(previous.width()) * usize::from(previous.height()),
        "animation should not redraw the whole frame"
    );
}

#[test]
fn landscape_maps_cpu_activity_to_stable_creature_intensity() {
    let activity = SceneActivity::from_core_loads(vec![0.10, 0.50, 0.95]);
    let theme = VisualTheme::default();

    let frame =
        build_landscape_frame_with_activity(32, 10, 0, &activity).expect("valid active frame");

    assert_eq!(
        frame.get(7, 5).expect("idle creature left").glyph,
        theme.creature_idle_left
    );
    assert_eq!(
        frame.get(8, 5).expect("idle creature body").glyph,
        theme.creature_idle
    );
    assert_eq!(
        frame.get(9, 5).expect("idle creature right").glyph,
        theme.creature_idle_right
    );
    assert_eq!(
        frame.get(16, 5).expect("active creature").glyph,
        theme.creature_active
    );
    assert_eq!(
        frame.get(24, 5).expect("busy creature").glyph,
        theme.creature_busy
    );
}

#[test]
fn default_visual_theme_replaces_phase_two_placeholder_glyphs() {
    let theme = VisualTheme::default();

    assert_ne!(theme.sky_top, theme.sky_horizon);
    assert_eq!(theme.horizon_marker, '·');
    assert_eq!(theme.shore, '▔');
    assert_eq!(theme.water_idle, '≈');
    assert_eq!(theme.water_download, '›');
    assert_eq!(theme.water_upload, '‹');
    assert_eq!(theme.water_bidirectional, '≋');
    assert_eq!(theme.weather_read, '∙');
    assert_eq!(theme.weather_write, '✦');
    assert_eq!(theme.weather_mixed, '✶');
    assert_eq!(theme.vegetation_high, '♣');
    assert_eq!(theme.creature_idle_left, '▗');
    assert_eq!(theme.creature_idle, '▄');
    assert_eq!(theme.creature_idle_right, '▖');
    assert_eq!(theme.creature_active_left, '▐');
    assert_eq!(theme.creature_active, '▄');
    assert_eq!(theme.creature_active_right, '▌');
    assert_eq!(theme.creature_busy_left, '▟');
    assert_eq!(theme.creature_busy, '█');
    assert_eq!(theme.creature_busy_right, '▙');
}

#[test]
fn landscape_renders_depth_bands_and_shoreline_before_activity_layers() {
    let activity = SceneActivity::default();
    let theme = VisualTheme::default();
    let frame =
        build_landscape_frame_with_activity(24, 12, 0, &activity).expect("valid layered frame");

    assert_eq!(frame.get(0, 0).expect("upper sky").bg, theme.sky_top);
    assert_eq!(frame.get(0, 4).expect("middle sky").bg, theme.sky_mid);
    assert_eq!(frame.get(0, 8).expect("horizon sky").bg, theme.sky_horizon);
    assert_eq!(
        frame.get(6, 8).expect("horizon marker").glyph,
        theme.horizon_marker
    );
    assert_eq!(frame.get(1, 9).expect("shoreline").glyph, theme.shore);
    assert_eq!(frame.get(1, 10).expect("water").glyph, theme.water_idle);
    assert_eq!(frame.get(0, 11).expect("ground").glyph, theme.ground);
}

#[test]
fn landscape_uses_irregular_horizon_spacing_instead_of_a_fixed_grid() {
    let activity = SceneActivity::default();
    let theme = VisualTheme::default();
    let frame =
        build_landscape_frame_with_activity(80, 24, 0, &activity).expect("valid organic frame");
    let horizon_y = 20;
    let positions = glyph_positions_on_row(&frame, horizon_y, theme.horizon_marker);

    assert!(
        positions.len() >= 5,
        "horizon should have enough marks to read as distant texture"
    );

    let gaps: Vec<u16> = positions
        .windows(2)
        .map(|window| window[1] - window[0])
        .collect();
    assert!(
        gaps.windows(2).any(|window| window[0] != window[1]),
        "horizon spacing should not be a rigid fixed-step ruler: {gaps:?}"
    );
}

#[test]
fn calm_landscape_keeps_shoreline_broken_into_readable_clusters() {
    let activity = SceneActivity::default();
    let theme = VisualTheme::default();
    let frame =
        build_landscape_frame_with_activity(80, 24, 0, &activity).expect("valid organic frame");
    let shore_y = 21;
    let occupied = count_glyphs_on_row(&frame, shore_y, theme.shore)
        + count_glyphs_on_row(&frame, shore_y, theme.vegetation_low);

    assert!(
        occupied > 24,
        "shoreline should remain visually present, got {occupied} occupied cells"
    );
    assert!(
        occupied < 72,
        "shoreline should leave natural gaps instead of filling the whole row, got {occupied}"
    );
}

#[test]
fn ambient_sky_motes_are_bounded_and_drift_between_ticks() {
    let activity = SceneActivity::default();
    let theme = VisualTheme::default();
    let previous =
        build_landscape_frame_with_activity(80, 24, 0, &activity).expect("valid previous frame");
    let current =
        build_landscape_frame_with_activity(80, 24, 1, &activity).expect("valid current frame");
    let previous_motes = count_glyphs(&previous, theme.sky_mote);
    let current_motes = count_glyphs(&current, theme.sky_mote);
    let output = AnsiDiffEncoder::new()
        .encode_diff(&previous, &current)
        .expect("matching organic frames");

    assert!((4..=14).contains(&previous_motes));
    assert!((4..=14).contains(&current_motes));
    assert!(
        output.changed_cells <= 32,
        "ambient drift should stay cheap, changed {} cells",
        output.changed_cells
    );
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
fn landscape_maps_memory_pressure_to_bounded_vegetation_density() {
    let calm_activity = SceneActivity::default().with_memory_pressure(0.0);
    let pressured_activity = SceneActivity::default().with_memory_pressure(1.0);
    let theme = VisualTheme::default();

    let calm_frame =
        build_landscape_frame_with_activity(20, 10, 0, &calm_activity).expect("valid calm frame");
    let pressured_frame = build_landscape_frame_with_activity(20, 10, 0, &pressured_activity)
        .expect("valid pressured frame");

    assert_eq!(
        count_glyphs_on_row(&calm_frame, 7, theme.vegetation_high),
        0
    );
    assert_eq!(
        count_glyphs_on_row(&pressured_frame, 7, theme.vegetation_high),
        5
    );
    assert_eq!(
        pressured_frame.get(10, 8).expect("water cell").glyph,
        theme.water_idle
    );
    assert_eq!(
        pressured_frame.get(10, 9).expect("ground cell").glyph,
        theme.ground
    );
}

#[test]
fn landscape_maps_network_flow_to_directional_water() {
    let download_activity = SceneActivity::default().with_network_flow(1.0, 0.0);
    let upload_activity = SceneActivity::default().with_network_flow(0.0, 1.0);
    let mixed_activity = SceneActivity::default().with_network_flow(1.0, 1.0);
    let theme = VisualTheme::default();

    let download_frame = build_landscape_frame_with_activity(20, 10, 0, &download_activity)
        .expect("valid download frame");
    let upload_frame = build_landscape_frame_with_activity(20, 10, 0, &upload_activity)
        .expect("valid upload frame");
    let mixed_frame =
        build_landscape_frame_with_activity(20, 10, 0, &mixed_activity).expect("valid mixed frame");

    assert!(count_glyphs_on_row(&download_frame, 8, theme.water_download) > 10);
    assert!(count_glyphs_on_row(&upload_frame, 8, theme.water_upload) > 10);
    assert!(count_glyphs_on_row(&mixed_frame, 8, theme.water_bidirectional) > 5);
}

#[test]
fn landscape_maps_disk_activity_to_bounded_weather() {
    let read_activity = SceneActivity::default().with_disk_activity(1.0, 0.0);
    let write_activity = SceneActivity::default().with_disk_activity(0.0, 1.0);
    let mixed_activity = SceneActivity::default().with_disk_activity(1.0, 1.0);
    let theme = VisualTheme::default();

    let read_frame =
        build_landscape_frame_with_activity(20, 10, 0, &read_activity).expect("valid read frame");
    let write_frame =
        build_landscape_frame_with_activity(20, 10, 0, &write_activity).expect("valid write frame");
    let mixed_frame =
        build_landscape_frame_with_activity(20, 10, 0, &mixed_activity).expect("valid mixed frame");

    assert_eq!(count_glyphs_on_row(&read_frame, 2, theme.weather_read), 5);
    assert_eq!(count_glyphs_on_row(&write_frame, 2, theme.weather_write), 5);
    assert_eq!(count_glyphs_on_row(&mixed_frame, 2, theme.weather_mixed), 5);
}

#[test]
fn activity_smoother_moves_world_signals_toward_target_without_snapping() {
    let target = SceneActivity::from_core_loads(vec![1.0, 0.5])
        .with_memory_pressure(1.0)
        .with_network_flow(0.8, 0.2)
        .with_disk_activity(0.6, 0.4);
    let mut smoother = ActivitySmoother::new(0.25);

    let first = smoother.step_towards(&target);

    assert_eq!(first.core_loads(), &[0.25, 0.125]);
    assert!((first.memory_pressure() - 0.25).abs() < f32::EPSILON);
    assert!((first.network_download() - 0.20).abs() < f32::EPSILON);
    assert!((first.network_upload() - 0.05).abs() < f32::EPSILON);
    assert!((first.disk_read() - 0.15).abs() < f32::EPSILON);
    assert!((first.disk_write() - 0.10).abs() < f32::EPSILON);

    let second = smoother.step_towards(&target);

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

    let current = smoother.step_towards(&target);

    assert_eq!(current, target);
}

#[test]
fn full_ecosystem_scene_keeps_layers_readable_and_output_bounded() {
    let activity = SceneActivity::from_core_loads(vec![0.95; 8])
        .with_memory_pressure(1.0)
        .with_network_flow(1.0, 1.0)
        .with_disk_activity(1.0, 1.0);
    let theme = VisualTheme::default();

    let previous =
        build_landscape_frame_with_activity(40, 14, 0, &activity).expect("valid previous frame");
    let current =
        build_landscape_frame_with_activity(40, 14, 1, &activity).expect("valid current frame");
    let output = AnsiDiffEncoder::new()
        .encode_diff(&previous, &current)
        .expect("matching active frames");

    assert_eq!(count_glyphs_on_row(&current, 3, theme.weather_mixed), 10);
    assert_eq!(count_glyphs_on_row(&current, 11, theme.vegetation_high), 10);
    assert_eq!(
        count_glyphs_on_row(&current, 12, theme.water_bidirectional),
        26
    );
    assert_eq!(count_glyphs_on_row(&current, 13, theme.ground), 40);
    assert_eq!(count_glyphs(&current, theme.creature_busy), 8);
    assert!(
        output.changed_cells <= 48,
        "full ecosystem animation changed {} cells",
        output.changed_cells
    );
}

#[test]
fn full_ecosystem_scene_stays_within_phase_two_render_budget_over_multiple_frames() {
    let activity = SceneActivity::from_core_loads(vec![0.95; 16])
        .with_memory_pressure(1.0)
        .with_network_flow(1.0, 1.0)
        .with_disk_activity(1.0, 1.0);
    let mut previous =
        build_landscape_frame_with_activity(80, 24, 0, &activity).expect("valid initial frame");
    let mut stats = FrameStats::default();

    for tick in 1..=16 {
        let current = build_landscape_frame_with_activity(80, 24, tick, &activity)
            .expect("valid active frame");
        let output = AnsiDiffEncoder::new()
            .encode_diff(&previous, &current)
            .expect("matching active frames");
        stats.record_frame(output.changed_cells, output.bytes.len());
        previous = current;
    }

    assert!(
        stats.average_changed_cells() <= 80,
        "average changed cells was {}",
        stats.average_changed_cells()
    );
    assert!(
        stats.average_bytes() <= 4_000,
        "average encoded bytes was {}",
        stats.average_bytes()
    );
}

#[test]
fn landscape_wraps_dense_cpu_activity_into_readable_lanes() {
    let activity = SceneActivity::from_core_loads(vec![0.50; 8]);
    let theme = VisualTheme::default();

    let frame =
        build_landscape_frame_with_activity(24, 10, 0, &activity).expect("valid dense frame");

    let upper_lane = count_glyphs_on_row(&frame, 4, theme.creature_active);
    let lower_lane = count_glyphs_on_row(&frame, 5, theme.creature_active);

    assert_eq!(upper_lane, 4);
    assert_eq!(lower_lane, 4);
}

#[test]
fn active_cpu_creatures_drift_one_cell_without_leaving_bounds() {
    let activity = SceneActivity::from_core_loads(vec![0.95]);
    let theme = VisualTheme::default();

    let first_frame =
        build_landscape_frame_with_activity(24, 10, 0, &activity).expect("valid first frame");
    let second_frame =
        build_landscape_frame_with_activity(24, 10, 4, &activity).expect("valid second frame");

    assert_eq!(
        first_frame.get(12, 5).expect("center creature").glyph,
        theme.creature_busy
    );
    assert_eq!(
        second_frame.get(13, 5).expect("drifted creature").glyph,
        theme.creature_busy
    );
}

#[test]
fn active_cpu_creatures_hold_sprite_shape_across_adjacent_ticks() {
    let activity = SceneActivity::from_core_loads(vec![0.95]);
    let previous =
        build_landscape_frame_with_activity(24, 10, 0, &activity).expect("valid first frame");
    let current =
        build_landscape_frame_with_activity(24, 10, 1, &activity).expect("valid second frame");
    let output = AnsiDiffEncoder::new()
        .encode_diff(&previous, &current)
        .expect("matching creature frames");

    assert_eq!(
        creature_sprite_on_row(&previous, 5),
        creature_sprite_on_row(&current, 5)
    );
    assert!(
        output.changed_cells <= 8,
        "stable creature sprite should avoid harsh flicker, changed {} cells",
        output.changed_cells
    );
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

#[test]
fn frame_stats_summarize_render_output_for_trace_mode() {
    let mut stats = FrameStats::default();

    stats.record_frame(10, 100);
    stats.record_frame(30, 260);

    assert_eq!(stats.frames(), 2);
    assert_eq!(stats.average_changed_cells(), 20);
    assert_eq!(stats.average_bytes(), 180);
    assert_eq!(
        stats.take_summary(),
        "2 frames, avg 20 changed cells, avg 180 bytes"
    );
    assert_eq!(stats.frames(), 0);
}

fn count_glyphs_on_row(frame: &Framebuffer, y: u16, glyph: char) -> usize {
    (0..frame.width())
        .filter(|x| frame.get(*x, y).is_some_and(|cell| cell.glyph == glyph))
        .count()
}

fn glyph_positions_on_row(frame: &Framebuffer, y: u16, glyph: char) -> Vec<u16> {
    (0..frame.width())
        .filter(|x| frame.get(*x, y).is_some_and(|cell| cell.glyph == glyph))
        .collect()
}

fn creature_sprite_on_row(frame: &Framebuffer, y: u16) -> Vec<char> {
    (0..frame.width())
        .filter_map(|x| {
            let glyph = frame.get(x, y)?.glyph;
            (glyph != ' ').then_some(glyph)
        })
        .collect()
}

fn count_glyphs(frame: &Framebuffer, glyph: char) -> usize {
    (0..frame.height())
        .flat_map(|y| (0..frame.width()).map(move |x| (x, y)))
        .filter(|(x, y)| frame.get(*x, *y).is_some_and(|cell| cell.glyph == glyph))
        .count()
}
