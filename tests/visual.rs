use ecosystem::{
    canvas::Rgba,
    simulation::SceneActivity,
    visual::{ProbeCanvasConfig, build_probe_canvas},
};

#[test]
fn probe_canvas_builds_expected_dimensions_and_opaque_pixels() {
    let canvas = build_probe_canvas(
        ProbeCanvasConfig::new(16, 9),
        0,
        &SceneActivity::default().with_memory_pressure(0.5),
    )
    .expect("valid probe canvas");

    assert_eq!(canvas.width(), 16);
    assert_eq!(canvas.height(), 9);
    assert!(canvas.pixels().iter().all(|pixel| pixel.a == 255));
}

#[test]
fn probe_canvas_changes_with_tick_and_activity_without_changing_size() {
    let calm = build_probe_canvas(ProbeCanvasConfig::new(16, 9), 0, &SceneActivity::default())
        .expect("valid calm canvas");
    let active = build_probe_canvas(
        ProbeCanvasConfig::new(16, 9),
        8,
        &SceneActivity::from_core_loads(vec![1.0]).with_network_flow(1.0, 0.0),
    )
    .expect("valid active canvas");

    assert_eq!(active.width(), calm.width());
    assert_eq!(active.height(), calm.height());
    assert_ne!(active.pixels(), calm.pixels());
    assert_ne!(active.pixel(8, 4), Some(Rgba::TRANSPARENT));
}
