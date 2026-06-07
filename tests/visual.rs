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
        scene.layer_names(),
        ["cafe_background", "main_cat_sprite", "window_rain"]
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
