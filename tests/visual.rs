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
    assert!(cat_pixels > 350, "cat sprite should read as a clear anchor");

    assert_eq!(
        scene.layer_names().as_slice(),
        [
            "cafe_background",
            "main_cat_sprite",
            "secondary_cat_sprite",
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
    let dirty_area: u32 = canvas
        .dirty_regions()
        .iter()
        .map(|region| u32::from(region.width) * u32::from(region.height))
        .sum();
    let canvas_area = u32::from(canvas.width()) * u32::from(canvas.height());

    assert!(!canvas.full_frame_required());
    assert!(
        dirty_area * 4 < canvas_area,
        "quiet frame dirty tiles should stay under 25% of the canvas"
    );
}

#[test]
fn cafe_scene_background_has_readable_window_counter_and_light_regions() {
    let mut scene =
        CafeScene::new(CafeCanvasConfig::new(CAFE_WIDTH, CAFE_HEIGHT)).expect("valid cafe scene");
    let canvas = scene.render(0, &SceneActivity::default());

    let window = canvas.pixel(335, 70).expect("window pixel in bounds");
    let clean_night = canvas.pixel(370, 78).expect("window night pixel in bounds");
    let skyline = canvas
        .pixel(342, 132)
        .expect("window skyline pixel in bounds");
    let counter = canvas.pixel(256, 190).expect("counter pixel in bounds");
    let cat_stage = canvas.pixel(256, 174).expect("cat stage pixel in bounds");
    let lamp = canvas.pixel(100, 54).expect("lamp pixel in bounds");
    let wall = canvas.pixel(52, 132).expect("wall pixel in bounds");
    let wall_panel = canvas.pixel(92, 132).expect("wall panel pixel in bounds");
    let counter_lip = canvas.pixel(440, 162).expect("counter lip pixel in bounds");

    assert!(window.b > window.r * 2, "window should read as cool night");
    assert!(
        clean_night.b > clean_night.r * 2 && clean_night.b > 60,
        "upper window should read as clean night glass"
    );
    assert!(
        skyline.b < clean_night.b && skyline.r < 25,
        "window scenery should sit low as an intentional distant skyline"
    );
    assert!(
        counter.r > counter.b * 2,
        "counter should read as warm wood"
    );
    assert!(
        cat_stage.r < counter.r && cat_stage.b < 40,
        "counter should stage the cat with a darker resting area"
    );
    assert!(
        wall_panel.r + 8 < wall.r,
        "wall should have deliberate depth bands instead of one flat rectangle"
    );
    assert!(
        counter_lip.r > counter.r,
        "counter lip should read brighter than the lower counter face"
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
    let changed_pixels = active
        .iter()
        .zip(&calm)
        .filter(|(active, calm)| active != calm)
        .count();

    assert!(
        changed_pixels > 100,
        "cat state should visibly change with high CPU"
    );
    assert_dirty_tiles_under_fraction(active_canvas, 3, "cat plus window rain");
}

#[test]
fn cafe_scene_uses_smaller_cat_footprint_on_balanced_canvas() {
    let mut scene =
        CafeScene::new(CafeCanvasConfig::new(CAFE_WIDTH, CAFE_HEIGHT)).expect("valid cafe scene");

    scene.render(0, &SceneActivity::default());
    let canvas = scene.render(30, &SceneActivity::default());

    assert_dirty_tiles_under_fraction(canvas, 4, "cat and window rain");
    let white_bounds = main_cat_white_bounds(canvas).expect("main cat should be visible");
    assert!(
        white_bounds.1 - white_bounds.0 <= 48,
        "main cat should be smaller than the previous 2x sprite footprint"
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
fn cafe_scene_walking_cat_paces_horizontally_under_high_cpu() {
    let mut scene =
        CafeScene::new(CafeCanvasConfig::new(CAFE_WIDTH, CAFE_HEIGHT)).expect("valid cafe scene");
    let high_cpu = SceneActivity::from_core_loads(vec![1.0]);

    scene.render(0, &high_cpu);
    let first_bounds = main_cat_white_bounds(scene.render(30, &high_cpu))
        .expect("walking cat should have visible white pixels");
    let second_bounds = main_cat_white_bounds(scene.render(54, &high_cpu))
        .expect("walking cat should remain visible");

    assert!(
        first_bounds.0.abs_diff(second_bounds.0) >= 4,
        "walking state should pace horizontally instead of animating in place"
    );
}

#[test]
fn cafe_scene_idle_cat_breathes_subtly_in_place() {
    let mut scene =
        CafeScene::new(CafeCanvasConfig::new(CAFE_WIDTH, CAFE_HEIGHT)).expect("valid cafe scene");
    let idle = SceneActivity::from_core_loads(vec![0.35]);

    scene.render(0, &idle);
    let first_bounds = main_cat_white_bounds(scene.render(20, &idle))
        .expect("idle cat should have visible pixels");
    let second_bounds =
        main_cat_white_bounds(scene.render(50, &idle)).expect("idle cat should remain visible");

    assert!(
        first_bounds.2.abs_diff(second_bounds.2) <= 2,
        "idle breathing should be subtle, not a jump"
    );
    assert_ne!(
        first_bounds.2, second_bounds.2,
        "idle cat should breathe instead of freezing in place"
    );
}

#[test]
fn cafe_scene_renders_secondary_black_cat_for_variety() {
    let mut scene =
        CafeScene::new(CafeCanvasConfig::new(CAFE_WIDTH, CAFE_HEIGHT)).expect("valid cafe scene");

    let canvas = scene.render(0, &SceneActivity::default());
    let accent_pixels = count_secondary_cat_accent_pixels(canvas);
    let white_bounds = main_cat_white_bounds(canvas).expect("main cat should be visible");
    let secondary_bounds = secondary_cat_accent_bounds(canvas)
        .expect("secondary black cat should have colored accent pixels");

    assert!(accent_pixels > 12, "black alternate cat should be visible");
    assert!(
        secondary_bounds.1 - secondary_bounds.0 <= white_bounds.1 - white_bounds.0,
        "black customer cat should not be larger than the main cat"
    );
    assert!(
        secondary_bounds.0 < 220,
        "black customer cat should sit on the counter bench, not against the window"
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
    let changed_pixels = busy
        .iter()
        .zip(&calm)
        .filter(|(busy, calm)| busy != calm)
        .count();

    assert!(
        changed_pixels > 30,
        "non-CPU metrics should produce visible bounded counter activity"
    );
    assert_dirty_tiles_under_fraction(busy_canvas, 3, "counter activity plus window rain");
}

fn assert_dirty_tiles_under_fraction(canvas: &Canvas, denominator: u32, label: &str) {
    let dirty_area = dirty_tile_area(canvas);
    let canvas_area = u32::from(canvas.width()) * u32::from(canvas.height());

    assert!(
        dirty_area * denominator < canvas_area,
        "{label} should stay bounded in dirty tile area"
    );
}

fn dirty_tile_area(canvas: &Canvas) -> u32 {
    canvas
        .dirty_regions()
        .iter()
        .map(|region| u32::from(region.width) * u32::from(region.height))
        .sum()
}

fn main_cat_white_bounds(canvas: &Canvas) -> Option<(u16, u16, u16)> {
    let mut min_x = u16::MAX;
    let mut max_x = 0;
    let mut min_y = u16::MAX;
    let mut found = false;
    for y in 100..190 {
        for x in 200..320 {
            let pixel = canvas.pixel(x, y).expect("test scans in-bounds pixels");
            if pixel.a == 255 && pixel.r > 210 && pixel.g > 210 && pixel.b > 210 {
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                found = true;
            }
        }
    }

    found.then_some((min_x, max_x, min_y))
}

fn count_secondary_cat_accent_pixels(canvas: &Canvas) -> usize {
    (116..178)
        .flat_map(|y| (120..220).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let pixel = canvas
                .pixel(*x, *y)
                .expect("customer cat scan stays in-bounds");
            pixel.a == 255 && pixel.r > 200 && pixel.g > 80 && pixel.g < 150 && pixel.b > 100
        })
        .count()
}

fn secondary_cat_accent_bounds(canvas: &Canvas) -> Option<(u16, u16)> {
    let mut min_x = u16::MAX;
    let mut max_x = 0;
    let mut found = false;
    for y in 116..178 {
        for x in 120..220 {
            let pixel = canvas
                .pixel(x, y)
                .expect("customer cat scan stays in-bounds");
            if pixel.a == 255 && pixel.r > 200 && pixel.g > 80 && pixel.g < 150 && pixel.b > 100 {
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                found = true;
            }
        }
    }

    found.then_some((min_x, max_x))
}

#[test]
fn cafe_scene_animates_window_rain_without_dirtying_the_whole_window() {
    let mut scene =
        CafeScene::new(CafeCanvasConfig::new(CAFE_WIDTH, CAFE_HEIGHT)).expect("valid cafe scene");

    scene.render(0, &SceneActivity::default());
    let first_rain_frame = scene.render(1, &SceneActivity::default()).pixels().to_vec();
    let second_canvas = scene.render(6, &SceneActivity::default());
    let second_rain_frame = second_canvas.pixels().to_vec();
    let changed_pixels = first_rain_frame
        .iter()
        .zip(&second_rain_frame)
        .filter(|(first, second)| first != second)
        .count();

    assert!(changed_pixels > 20, "window rain should animate");
    assert_dirty_tiles_under_fraction(second_canvas, 3, "rain plus cat");
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
