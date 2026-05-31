use ecosystem::{
    diagnostics::{TraceCollector, TraceEvent},
    framebuffer::{Cell, Color, Framebuffer},
    render::build_static_landscape_frame,
    terminal::AnsiDiffEncoder,
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
