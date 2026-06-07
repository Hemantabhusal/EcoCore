use ecosystem::{
    canvas::{Canvas, Rgba},
    simulation::SceneActivity,
    visual::{
        CafeCanvasConfig, CafeScene, LayeredScene, SceneCanvasConfig, SceneFrame, SceneLayer,
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
fn cafe_scene_renders_larger_macro_readable_canvas_with_cat_anchor() {
    let mut scene = CafeScene::new(CafeCanvasConfig::new(400, 192)).expect("valid cafe scene");

    let canvas = scene.render(0, &SceneActivity::default());

    assert_eq!(canvas.width(), 400);
    assert_eq!(canvas.height(), 192);
    assert!(canvas.full_frame_required());
    let cat_pixels = canvas
        .pixels()
        .iter()
        .filter(|pixel| pixel.a == 255 && pixel.r > 150 && pixel.g > 130 && pixel.b > 105)
        .count();
    assert!(cat_pixels > 800, "cat sprite should read as a large anchor");

    assert_eq!(
        scene.layer_names().as_slice(),
        [
            "cafe_background",
            "main_cat_sprite",
            "window_rain",
            "warm_light"
        ]
    );
}

#[test]
fn cafe_scene_keeps_quiet_animation_dirty_region_spatially_bounded() {
    let mut scene = CafeScene::new(CafeCanvasConfig::new(400, 192)).expect("valid cafe scene");

    scene.render(0, &SceneActivity::default());
    let canvas = scene.render(1, &SceneActivity::default());
    let dirty_region = canvas.dirty_region().expect("cat animation changed");
    let dirty_area = u32::from(dirty_region.width) * u32::from(dirty_region.height);
    let canvas_area = u32::from(canvas.width()) * u32::from(canvas.height());

    assert!(!canvas.full_frame_required());
    assert!(
        dirty_area * 4 < canvas_area,
        "quiet frame dirty bounds should stay under 25% of the canvas"
    );
}

#[test]
fn cafe_scene_background_has_readable_window_counter_and_light_regions() {
    let mut scene = CafeScene::new(CafeCanvasConfig::new(400, 192)).expect("valid cafe scene");
    let canvas = scene.render(0, &SceneActivity::default());

    let window = canvas.pixel(260, 60).expect("window pixel in bounds");
    let counter = canvas.pixel(200, 152).expect("counter pixel in bounds");
    let lamp = canvas.pixel(78, 42).expect("lamp pixel in bounds");
    let wall = canvas.pixel(36, 104).expect("wall pixel in bounds");

    assert!(window.b > window.r * 2, "window should read as cool night");
    assert!(
        counter.r > counter.b * 2,
        "counter should read as warm wood"
    );
    assert!(
        lamp.r > 180 && lamp.g > 120,
        "lamp should create warm focal color"
    );
    assert_ne!(wall, window);
    assert_ne!(wall, counter);
}

#[test]
fn cafe_scene_switches_cat_presence_with_cpu_activity_inside_same_area() {
    let mut calm_scene = CafeScene::new(CafeCanvasConfig::new(400, 192)).expect("valid cafe scene");
    let mut active_scene =
        CafeScene::new(CafeCanvasConfig::new(400, 192)).expect("valid cafe scene");

    calm_scene.render(0, &SceneActivity::default());
    active_scene.render(0, &SceneActivity::from_core_loads(vec![1.0]));

    let calm = calm_scene
        .render(30, &SceneActivity::default())
        .pixels()
        .to_vec();
    let active_canvas = active_scene.render(30, &SceneActivity::from_core_loads(vec![1.0]));
    let active = active_canvas.pixels().to_vec();
    let dirty_region = active_canvas
        .dirty_region()
        .expect("active cat sprite marks dirty region");
    let changed_pixels = active
        .iter()
        .zip(&calm)
        .filter(|(active, calm)| active != calm)
        .count();

    assert!(
        changed_pixels > 100,
        "cat state should visibly change with high CPU"
    );
    assert!(dirty_region.width <= 128);
    assert!(dirty_region.height <= 112);
}

#[test]
fn layered_scene_composes_layers_in_order_and_reuses_canvas_storage() {
    let mut scene = LayeredScene::new(
        SceneCanvasConfig::new(4, 3),
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
