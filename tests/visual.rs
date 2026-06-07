use ecosystem::{
    canvas::{Canvas, Rgba},
    simulation::SceneActivity,
    visual::{
        CafeCanvasConfig, CafeScene, LayeredScene, SceneCanvasConfig, SceneFrame, SceneLayer,
    },
};

const CAFE_WIDTH: u16 = 512;
const CAFE_HEIGHT: u16 = 240;

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
    let mut scene =
        CafeScene::new(CafeCanvasConfig::new(CAFE_WIDTH, CAFE_HEIGHT)).expect("valid cafe scene");

    let canvas = scene.render(0, &SceneActivity::default());

    assert_eq!(canvas.width(), CAFE_WIDTH);
    assert_eq!(canvas.height(), CAFE_HEIGHT);
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
            "warm_light",
            "counter_activity"
        ]
    );
}

#[test]
fn cafe_scene_keeps_quiet_animation_dirty_region_spatially_bounded() {
    let mut scene =
        CafeScene::new(CafeCanvasConfig::new(CAFE_WIDTH, CAFE_HEIGHT)).expect("valid cafe scene");

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
    let mut scene =
        CafeScene::new(CafeCanvasConfig::new(CAFE_WIDTH, CAFE_HEIGHT)).expect("valid cafe scene");
    let canvas = scene.render(0, &SceneActivity::default());

    let window = canvas.pixel(335, 70).expect("window pixel in bounds");
    let counter = canvas.pixel(256, 190).expect("counter pixel in bounds");
    let cat_stage = canvas.pixel(256, 174).expect("cat stage pixel in bounds");
    let lamp = canvas.pixel(100, 54).expect("lamp pixel in bounds");
    let wall = canvas.pixel(52, 132).expect("wall pixel in bounds");

    assert!(window.b > window.r * 2, "window should read as cool night");
    assert!(
        counter.r > counter.b * 2,
        "counter should read as warm wood"
    );
    assert!(
        cat_stage.r < counter.r && cat_stage.b < 40,
        "counter should stage the cat with a darker resting area"
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
    let mut calm_scene =
        CafeScene::new(CafeCanvasConfig::new(CAFE_WIDTH, CAFE_HEIGHT)).expect("valid cafe scene");
    let mut active_scene =
        CafeScene::new(CafeCanvasConfig::new(CAFE_WIDTH, CAFE_HEIGHT)).expect("valid cafe scene");

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
fn cafe_scene_uses_smaller_cat_footprint_on_balanced_canvas() {
    let mut scene =
        CafeScene::new(CafeCanvasConfig::new(CAFE_WIDTH, CAFE_HEIGHT)).expect("valid cafe scene");

    scene.render(0, &SceneActivity::default());
    let canvas = scene.render(30, &SceneActivity::default());
    let dirty_region = canvas
        .dirty_region()
        .expect("cat animation should mark a bounded region");

    assert!(
        dirty_region.width <= 96,
        "cat should not dominate the 512x240 cafe width"
    );
    assert!(
        dirty_region.height <= 96,
        "cat should stay staged around the counter, not fill the scene"
    );
}

#[test]
fn cafe_scene_keeps_walking_state_through_brief_cpu_dip() {
    let mut warmed_scene =
        CafeScene::new(CafeCanvasConfig::new(CAFE_WIDTH, CAFE_HEIGHT)).expect("valid cafe scene");
    let mut fresh_scene =
        CafeScene::new(CafeCanvasConfig::new(CAFE_WIDTH, CAFE_HEIGHT)).expect("valid cafe scene");

    warmed_scene.render(0, &SceneActivity::from_core_loads(vec![1.0]));
    warmed_scene.render(30, &SceneActivity::from_core_loads(vec![1.0]));
    let warmed_after_dip = warmed_scene
        .render(36, &SceneActivity::from_core_loads(vec![0.60]))
        .pixels()
        .to_vec();

    fresh_scene.render(0, &SceneActivity::from_core_loads(vec![0.60]));
    let fresh_moderate = fresh_scene
        .render(36, &SceneActivity::from_core_loads(vec![0.60]))
        .pixels()
        .to_vec();

    let changed_pixels = warmed_after_dip
        .iter()
        .zip(&fresh_moderate)
        .filter(|(warmed, fresh)| warmed != fresh)
        .count();

    assert!(
        changed_pixels > 100,
        "brief CPU dips should not immediately snap the cat out of walking"
    );
}

#[test]
fn cafe_scene_maps_non_cpu_metrics_to_bounded_counter_activity() {
    let mut calm_scene =
        CafeScene::new(CafeCanvasConfig::new(CAFE_WIDTH, CAFE_HEIGHT)).expect("valid cafe scene");
    let mut busy_scene =
        CafeScene::new(CafeCanvasConfig::new(CAFE_WIDTH, CAFE_HEIGHT)).expect("valid cafe scene");
    let busy_activity = SceneActivity::default()
        .with_memory_pressure(0.9)
        .with_network_flow(0.8, 0.5)
        .with_disk_activity(0.7, 1.0);

    calm_scene.render(0, &SceneActivity::default());
    busy_scene.render(0, &busy_activity);

    let calm = calm_scene
        .render(18, &SceneActivity::default())
        .pixels()
        .to_vec();
    let busy_canvas = busy_scene.render(18, &busy_activity);
    let busy = busy_canvas.pixels().to_vec();
    let dirty_region = busy_canvas
        .dirty_region()
        .expect("counter activity should mark dirty pixels");
    let changed_pixels = busy
        .iter()
        .zip(&calm)
        .filter(|(busy, calm)| busy != calm)
        .count();

    assert!(
        changed_pixels > 30,
        "non-CPU metrics should produce visible bounded counter activity"
    );
    assert!(
        dirty_region.width <= 180,
        "counter activity must remain horizontally clustered"
    );
    assert!(
        dirty_region.height <= 96,
        "counter activity must stay near the counter"
    );
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
