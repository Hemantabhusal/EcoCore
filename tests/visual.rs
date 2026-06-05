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
fn probe_scene_marks_first_render_as_full_frame_dirty() {
    let mut scene = ProbeScene::new(ProbeCanvasConfig::new(16, 9)).expect("valid probe scene");

    let canvas = scene.render(0, &SceneActivity::default());

    assert_eq!(
        canvas.dirty_region(),
        Some(ecosystem::canvas::DirtyRegion {
            x: 0,
            y: 0,
            width: 16,
            height: 9
        })
    );
    assert!(canvas.full_frame_required());
}

#[test]
fn probe_scene_keeps_quiet_frame_on_partial_update_path_after_environment_refresh() {
    let mut scene = ProbeScene::new(ProbeCanvasConfig::new(64, 40)).expect("valid probe scene");

    scene.render(0, &SceneActivity::default());
    let canvas = scene.render(1, &SceneActivity::default());

    assert!(
        canvas
            .dirty_region()
            .is_some_and(|dirty| dirty.width > 0 && dirty.height > 0)
    );
    assert!(!canvas.full_frame_required());
}

#[test]
fn probe_scene_exposes_named_internal_composition_layers() {
    let scene = ProbeScene::new(ProbeCanvasConfig::new(16, 9)).expect("valid probe scene");

    assert_eq!(
        scene.layer_names(),
        [
            "deep_water",
            "surface_light",
            "reef_growth",
            "current_bands",
            "drift_motes",
            "reef_polyps",
            "lifeform_wakes",
            "glow_lifeforms",
            "sediment_sparks"
        ]
    );
}

#[test]
fn tidepool_scene_renders_non_flat_idle_water() {
    let mut scene = ProbeScene::new(ProbeCanvasConfig::new(32, 18)).expect("valid probe scene");

    let canvas = scene.render(4, &SceneActivity::default());
    let top_left = canvas.pixel(0, 0).expect("pixel in bounds");
    let center = canvas.pixel(16, 9).expect("pixel in bounds");
    let bottom_right = canvas.pixel(31, 17).expect("pixel in bounds");

    assert_ne!(top_left, center);
    assert_ne!(center, bottom_right);
    assert!(center.b > center.r);
}

#[test]
fn tidepool_memory_pressure_increases_reef_glow_near_bottom() {
    let mut calm_scene =
        ProbeScene::new(ProbeCanvasConfig::new(32, 18)).expect("valid probe scene");
    let mut pressured_scene =
        ProbeScene::new(ProbeCanvasConfig::new(32, 18)).expect("valid probe scene");

    let calm = calm_scene
        .render(10, &SceneActivity::default().with_memory_pressure(0.05))
        .pixels()
        .to_vec();
    let pressured = pressured_scene
        .render(10, &SceneActivity::default().with_memory_pressure(0.95))
        .pixels()
        .to_vec();

    assert!(bottom_green_energy(&pressured, 32, 18) > bottom_green_energy(&calm, 32, 18));
}

#[test]
fn tidepool_disk_activity_adds_bright_sediment_sparks() {
    let mut calm_scene =
        ProbeScene::new(ProbeCanvasConfig::new(32, 18)).expect("valid probe scene");
    let mut active_scene =
        ProbeScene::new(ProbeCanvasConfig::new(32, 18)).expect("valid probe scene");

    let calm = calm_scene
        .render(18, &SceneActivity::default())
        .pixels()
        .to_vec();
    let active = active_scene
        .render(18, &SceneActivity::default().with_disk_activity(0.0, 1.0))
        .pixels()
        .to_vec();

    assert!(bright_pixel_count(&active) > bright_pixel_count(&calm));
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
fn lifeform_seed_renders_directional_body_instead_of_round_dot() {
    let mut field = LifeformField::new(1, ProbeCanvasConfig::new(48, 28));
    let mut canvas = Canvas::new(48, 28, Rgba::rgb(0, 0, 0)).expect("valid canvas");

    field.update(4, &SceneActivity::from_core_loads(vec![0.6]));
    let lifeform = field.snapshots()[0];
    field.render_seeds(&mut canvas);

    let spans = lit_axis_spans(&canvas, lifeform.heading_x, lifeform.heading_y)
        .expect("lifeform body renders lit pixels");

    assert!(spans.forward >= spans.side * 1.6);
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
    assert_eq!(
        second.dirty_region(),
        Some(ecosystem::canvas::DirtyRegion {
            x: 0,
            y: 0,
            width: 4,
            height: 3
        })
    );
}

fn bottom_green_energy(pixels: &[Rgba], width: usize, height: usize) -> u32 {
    let start = width * (height * 2 / 3);
    pixels[start..].iter().map(|pixel| u32::from(pixel.g)).sum()
}

fn bright_pixel_count(pixels: &[Rgba]) -> usize {
    pixels
        .iter()
        .filter(|pixel| u16::from(pixel.r) + u16::from(pixel.g) + u16::from(pixel.b) > 430)
        .count()
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct AxisSpans {
    forward: f32,
    side: f32,
}

fn lit_axis_spans(canvas: &Canvas, heading_x: f32, heading_y: f32) -> Option<AxisSpans> {
    let side_x = -heading_y;
    let side_y = heading_x;
    let mut min_forward = f32::MAX;
    let mut max_forward = f32::MIN;
    let mut min_side = f32::MAX;
    let mut max_side = f32::MIN;
    let mut found = false;

    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            let pixel = canvas.pixel(x, y).expect("pixel in bounds");
            if u16::from(pixel.r) + u16::from(pixel.g) + u16::from(pixel.b) <= 90 {
                continue;
            }

            let forward = f32::from(x) * heading_x + f32::from(y) * heading_y;
            let side = f32::from(x) * side_x + f32::from(y) * side_y;
            min_forward = min_forward.min(forward);
            max_forward = max_forward.max(forward);
            min_side = min_side.min(side);
            max_side = max_side.max(side);
            found = true;
        }
    }

    found.then_some(AxisSpans {
        forward: max_forward - min_forward,
        side: max_side - min_side,
    })
}
