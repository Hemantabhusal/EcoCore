use ecosystem::{
    canvas::{Canvas, Rgba},
    simulation::SceneActivity,
    visual::{
        LayeredScene, ProbeCanvasConfig, ProbeScene, SceneFrame, SceneLayer, build_probe_canvas,
    },
};

struct FillLayer {
    color: Rgba,
}

impl SceneLayer for FillLayer {
    fn render(&mut self, canvas: &mut Canvas, _frame: SceneFrame<'_>) {
        canvas.fill(self.color);
    }
}

struct MarkerLayer;

impl SceneLayer for MarkerLayer {
    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>) {
        let color = if frame.tick().is_multiple_of(2) {
            Rgba::rgb(200, 30, 80)
        } else {
            Rgba::rgb(40, 210, 160)
        };
        canvas.set_pixel(0, 0, color).expect("marker in bounds");
    }
}

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

#[test]
fn probe_scene_reuses_canvas_storage_between_frames() {
    let mut scene = ProbeScene::new(ProbeCanvasConfig::new(16, 9)).expect("valid probe scene");

    let first = scene.render(0, &SceneActivity::default());
    let first_ptr = first.pixels().as_ptr();
    let first_pixels = first.pixels().to_vec();

    let second = scene.render(
        8,
        &SceneActivity::from_core_loads(vec![1.0]).with_network_flow(1.0, 0.0),
    );
    let second_ptr = second.pixels().as_ptr();

    assert_eq!(second.width(), 16);
    assert_eq!(second.height(), 9);
    assert_eq!(second_ptr, first_ptr);
    assert_ne!(second.pixels(), first_pixels.as_slice());
}

#[test]
fn probe_scene_keeps_rendered_canvas_clean_for_full_frame_presentation() {
    let mut scene = ProbeScene::new(ProbeCanvasConfig::new(16, 9)).expect("valid probe scene");

    let canvas = scene.render(0, &SceneActivity::default());

    assert_eq!(canvas.dirty_region(), None);
}

#[test]
fn probe_scene_exposes_named_internal_composition_layers() {
    let scene = ProbeScene::new(ProbeCanvasConfig::new(16, 9)).expect("valid probe scene");

    assert_eq!(
        scene.layer_names(),
        ["background_field", "activity_pulse", "flow_tint"]
    );
}

#[test]
fn layered_scene_composes_layers_in_order_and_reuses_canvas_storage() {
    let mut scene = LayeredScene::new(
        ProbeCanvasConfig::new(4, 3),
        vec![
            Box::new(FillLayer {
                color: Rgba::rgb(1, 2, 3),
            }),
            Box::new(MarkerLayer),
        ],
    )
    .expect("valid layered scene");

    let first = scene.render(2, &SceneActivity::default());
    let first_ptr = first.pixels().as_ptr();

    assert_eq!(first.pixel(0, 0), Some(Rgba::rgb(200, 30, 80)));
    assert_eq!(first.pixel(1, 0), Some(Rgba::rgb(1, 2, 3)));

    let second = scene.render(3, &SceneActivity::default());

    assert_eq!(second.pixels().as_ptr(), first_ptr);
    assert_eq!(second.pixel(0, 0), Some(Rgba::rgb(40, 210, 160)));
    assert_eq!(second.dirty_region(), None);
}
