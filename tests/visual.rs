use ecosystem::{
    canvas::{Canvas, Rgba},
    simulation::SceneActivity,
    visual::{
        LayeredScene, LifeformField, LifeformTrailConfig, ProbeCanvasConfig, ProbeScene,
        SceneFrame, SceneLayer,
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
    let mut scene = ProbeScene::new(ProbeCanvasConfig::new(16, 9)).expect("valid probe scene");
    let canvas = scene.render(0, &SceneActivity::default().with_memory_pressure(0.5));

    assert_eq!(canvas.width(), 16);
    assert_eq!(canvas.height(), 9);
    assert!(canvas.pixels().iter().all(|pixel| pixel.a == 255));
}

#[test]
fn probe_canvas_changes_with_tick_and_activity_without_changing_size() {
    let mut scene = ProbeScene::new(ProbeCanvasConfig::new(16, 9)).expect("valid probe scene");
    let calm = scene.render(0, &SceneActivity::default()).pixels().to_vec();
    let active = scene
        .render(
            8,
            &SceneActivity::from_core_loads(vec![1.0]).with_network_flow(1.0, 0.0),
        )
        .pixels()
        .to_vec();

    assert_eq!(active.len(), calm.len());
    assert_ne!(active, calm);
    assert_ne!(active[4 * 16 + 8], Rgba::TRANSPARENT);
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
        [
            "background_field",
            "activity_pulse",
            "lifeform_trails",
            "lifeform_seeds",
            "flow_tint"
        ]
    );
}

#[test]
fn lifeform_field_initializes_deterministically_inside_canvas() {
    let field = LifeformField::new(6, ProbeCanvasConfig::new(32, 18));
    let snapshots = field.snapshots();

    assert_eq!(snapshots.len(), 6);
    assert!(snapshots.iter().all(|seed| seed.x >= 0.0 && seed.x < 32.0));
    assert!(snapshots.iter().all(|seed| seed.y >= 0.0 && seed.y < 18.0));
    assert!(snapshots.iter().all(|seed| seed.energy > 0.0));
    assert_eq!(
        snapshots,
        LifeformField::new(6, ProbeCanvasConfig::new(32, 18)).snapshots()
    );
}

#[test]
fn lifeform_field_moves_seeds_without_leaving_canvas_bounds() {
    let mut field = LifeformField::new(4, ProbeCanvasConfig::new(32, 18));
    let before = field.snapshots();

    field.update(
        1,
        &SceneActivity::from_core_loads(vec![1.0]).with_network_flow(0.5, 0.0),
    );
    let after = field.snapshots();

    assert_ne!(after, before);
    assert!(after.iter().all(|seed| seed.x >= 0.0 && seed.x < 32.0));
    assert!(after.iter().all(|seed| seed.y >= 0.0 && seed.y < 18.0));
}

#[test]
fn lifeform_field_keeps_bounded_trail_history() {
    let mut field = LifeformField::new(2, ProbeCanvasConfig::new(32, 18));

    for tick in 0..12 {
        field.update(tick, &SceneActivity::from_core_loads(vec![0.7]));
    }

    let trails = field.trail_snapshots();

    assert_eq!(trails.len(), 2 * LifeformTrailConfig::DEFAULT_CAPACITY);
    assert!(trails.iter().all(|trail| trail.x >= 0.0 && trail.x < 32.0));
    assert!(trails.iter().all(|trail| trail.y >= 0.0 && trail.y < 18.0));
    assert!(trails.iter().all(|trail| trail.intensity > 0.0));
    assert!(trails.iter().all(|trail| trail.intensity <= 1.0));
}

#[test]
fn lifeform_trails_render_fainter_than_current_seed_points() {
    let mut field = LifeformField::new(1, ProbeCanvasConfig::new(32, 18));
    let mut trail_canvas = Canvas::new(32, 18, Rgba::rgb(0, 0, 0)).expect("valid canvas");
    let mut seed_canvas = Canvas::new(32, 18, Rgba::rgb(0, 0, 0)).expect("valid canvas");

    for tick in 0..6 {
        field.update(tick, &SceneActivity::from_core_loads(vec![0.8]));
    }
    field.render_trails(&mut trail_canvas);
    field.render_seeds(&mut seed_canvas);

    let trail_energy: u32 = trail_canvas
        .pixels()
        .iter()
        .map(|pixel| u32::from(pixel.r) + u32::from(pixel.g) + u32::from(pixel.b))
        .sum();
    let seed_energy: u32 = seed_canvas
        .pixels()
        .iter()
        .map(|pixel| u32::from(pixel.r) + u32::from(pixel.g) + u32::from(pixel.b))
        .sum();

    assert!(trail_energy > 0);
    assert!(trail_energy < seed_energy);
}

#[test]
fn probe_scene_lifeform_layer_changes_pixels_over_time() {
    let mut scene = ProbeScene::new(ProbeCanvasConfig::new(32, 18)).expect("valid probe scene");

    let first = scene.render(0, &SceneActivity::default()).pixels().to_vec();
    let second = scene
        .render(
            12,
            &SceneActivity::from_core_loads(vec![0.8]).with_network_flow(0.4, 0.0),
        )
        .pixels()
        .to_vec();

    assert_ne!(second, first);
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
