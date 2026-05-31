use ecosystem::{
    app::{StartupEnvironment, render_initial_frame},
    diagnostics::{TraceCollector, TraceEvent},
    framebuffer::{Cell, Color, Framebuffer},
    render::build_static_landscape_frame,
    terminal::{
        AnsiDiffEncoder, TerminalSession, TerminalSessionOptions, TerminalSize,
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

    assert_eq!(frame.width(), 20);
    assert_eq!(frame.height(), 8);
    assert_eq!(frame.get(0, 7).expect("ground cell").glyph, '.');
    assert_eq!(frame.get(10, 6).expect("water cell").glyph, '~');
    assert_eq!(frame.get(10, 4).expect("creature cell").glyph, 'o');
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
