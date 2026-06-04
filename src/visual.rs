use std::{cell::RefCell, rc::Rc};

use crate::{
    canvas::{Canvas, CanvasError, Rgba},
    simulation::SceneActivity,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TidepoolCanvasConfig {
    pub width: u16,
    pub height: u16,
}

impl TidepoolCanvasConfig {
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

pub type ProbeCanvasConfig = TidepoolCanvasConfig;

pub struct TidepoolScene {
    scene: LayeredScene,
}

impl TidepoolScene {
    pub fn new(config: TidepoolCanvasConfig) -> Result<Self, CanvasError> {
        Ok(Self {
            scene: LayeredScene::new(config, tidepool_layers(config))?,
        })
    }

    pub fn render(&mut self, tick: u64, activity: &SceneActivity) -> &Canvas {
        self.scene.render(tick, activity)
    }

    pub fn layer_names(&self) -> Vec<&'static str> {
        self.scene.layer_names()
    }
}

pub type ProbeScene = TidepoolScene;

#[derive(Clone, Copy, Debug)]
pub struct SceneFrame<'a> {
    tick: u64,
    activity: &'a SceneActivity,
}

impl<'a> SceneFrame<'a> {
    pub const fn new(tick: u64, activity: &'a SceneActivity) -> Self {
        Self { tick, activity }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn activity(self) -> &'a SceneActivity {
        self.activity
    }
}

pub trait SceneLayer {
    fn name(&self) -> &'static str {
        "anonymous"
    }

    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>);
}

pub struct LayeredScene {
    canvas: Canvas,
    layers: Vec<Box<dyn SceneLayer>>,
}

impl LayeredScene {
    pub fn new(
        config: TidepoolCanvasConfig,
        layers: Vec<Box<dyn SceneLayer>>,
    ) -> Result<Self, CanvasError> {
        Ok(Self {
            canvas: Canvas::new(config.width, config.height, Rgba::rgb(0, 0, 0))?,
            layers,
        })
    }

    pub fn render(&mut self, tick: u64, activity: &SceneActivity) -> &Canvas {
        let frame = SceneFrame::new(tick, activity);
        for layer in &mut self.layers {
            layer.render(&mut self.canvas, frame);
        }
        self.canvas.clear_dirty();
        &self.canvas
    }

    pub fn layer_names(&self) -> Vec<&'static str> {
        self.layers.iter().map(|layer| layer.name()).collect()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DeepWaterLayer;

impl SceneLayer for DeepWaterLayer {
    fn name(&self) -> &'static str {
        "deep_water"
    }

    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>) {
        draw_deep_water(frame.tick(), frame.activity(), canvas);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ReefGrowthLayer;

impl SceneLayer for ReefGrowthLayer {
    fn name(&self) -> &'static str {
        "reef_growth"
    }

    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>) {
        draw_reef_growth(frame.tick(), frame.activity(), canvas);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CurrentBandsLayer;

impl SceneLayer for CurrentBandsLayer {
    fn name(&self) -> &'static str {
        "current_bands"
    }

    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>) {
        draw_current_bands(frame.tick(), frame.activity(), canvas);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SedimentSparksLayer;

impl SceneLayer for SedimentSparksLayer {
    fn name(&self) -> &'static str {
        "sediment_sparks"
    }

    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>) {
        draw_sediment_sparks(frame.tick(), frame.activity(), canvas);
    }
}

fn tidepool_layers(config: TidepoolCanvasConfig) -> Vec<Box<dyn SceneLayer>> {
    // Trails and seeds are separate visual layers, but they must share motion
    // state so the afterglow follows the actual lifeform positions.
    let lifeforms = Rc::new(RefCell::new(LifeformField::new(14, config)));
    vec![
        Box::new(DeepWaterLayer),
        Box::new(ReefGrowthLayer),
        Box::new(CurrentBandsLayer),
        Box::new(LifeformTrailLayer::new(lifeforms.clone())),
        Box::new(LifeformSeedLayer::new(lifeforms)),
        Box::new(SedimentSparksLayer),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LifeformSnapshot {
    pub x: f32,
    pub y: f32,
    pub energy: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LifeformTrailSnapshot {
    pub x: f32,
    pub y: f32,
    pub intensity: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifeformTrailConfig;

impl LifeformTrailConfig {
    pub const DEFAULT_CAPACITY: usize = 8;
}

#[derive(Clone, Debug, PartialEq)]
pub struct LifeformField {
    bounds: TidepoolCanvasConfig,
    seeds: Vec<LifeformSeed>,
}

impl LifeformField {
    pub fn new(count: usize, bounds: TidepoolCanvasConfig) -> Self {
        let width = f32::from(bounds.width.max(1));
        let height = f32::from(bounds.height.max(1));
        let mut seeds = Vec::with_capacity(count);

        for index in 0..count {
            let phase = index as f32 * 1.618_034;
            let x = ((phase.sin() * 0.5 + 0.5) * (width - 1.0)).clamp(0.0, width - 1.0);
            let y = ((phase.cos() * 0.5 + 0.5) * (height - 1.0)).clamp(0.0, height - 1.0);
            let vx = 0.18 + (index % 3) as f32 * 0.05;
            let vy = 0.10 + (index % 5) as f32 * 0.035;
            let energy = 0.45 + (index % 4) as f32 * 0.12;
            let mut trail = Vec::with_capacity(LifeformTrailConfig::DEFAULT_CAPACITY);
            trail.push(LifeformTrailPoint {
                x,
                y,
                intensity: 1.0,
            });
            seeds.push(LifeformSeed {
                x,
                y,
                vx,
                vy,
                energy,
                trail,
            });
        }

        Self { bounds, seeds }
    }

    pub fn update(&mut self, tick: u64, activity: &SceneActivity) {
        let width = f32::from(self.bounds.width.max(1));
        let height = f32::from(self.bounds.height.max(1));
        let cpu_energy = average(activity.core_loads());
        let flow_energy = activity.network_download().max(activity.network_upload());
        let speed = 0.55 + cpu_energy * 0.9 + flow_energy * 0.35;

        for (index, seed) in self.seeds.iter_mut().enumerate() {
            let drift = ((tick as f32 * 0.035) + index as f32).sin() * 0.08;
            for point in &mut seed.trail {
                point.intensity *= 0.78;
            }
            seed.x = wrap(seed.x + (seed.vx + drift) * speed, width);
            seed.y = wrap(seed.y + (seed.vy - drift * 0.6) * speed, height);
            seed.energy =
                (0.42 + cpu_energy * 0.38 + flow_energy * 0.22 + drift.abs()).clamp(0.2, 1.0);
            seed.trail.insert(
                0,
                LifeformTrailPoint {
                    x: seed.x,
                    y: seed.y,
                    intensity: seed.energy,
                },
            );
            seed.trail.truncate(LifeformTrailConfig::DEFAULT_CAPACITY);
        }
    }

    pub fn render(&self, canvas: &mut Canvas) {
        self.render_trails(canvas);
        self.render_seeds(canvas);
    }

    pub fn render_seeds(&self, canvas: &mut Canvas) {
        for seed in &self.seeds {
            render_lifeform_seed(canvas, seed);
        }
    }

    pub fn render_trails(&self, canvas: &mut Canvas) {
        for seed in &self.seeds {
            for (age, point) in seed.trail.iter().enumerate().skip(1) {
                render_lifeform_trail_point(canvas, point, age);
            }
        }
    }

    pub fn snapshots(&self) -> Vec<LifeformSnapshot> {
        self.seeds
            .iter()
            .map(|seed| LifeformSnapshot {
                x: seed.x,
                y: seed.y,
                energy: seed.energy,
            })
            .collect()
    }

    pub fn trail_snapshots(&self) -> Vec<LifeformTrailSnapshot> {
        self.seeds
            .iter()
            .flat_map(|seed| {
                seed.trail.iter().map(|point| LifeformTrailSnapshot {
                    x: point.x,
                    y: point.y,
                    intensity: point.intensity,
                })
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct LifeformSeed {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    energy: f32,
    trail: Vec<LifeformTrailPoint>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LifeformTrailPoint {
    x: f32,
    y: f32,
    intensity: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct LifeformTrailLayer {
    field: Rc<RefCell<LifeformField>>,
}

impl LifeformTrailLayer {
    fn new(field: Rc<RefCell<LifeformField>>) -> Self {
        Self { field }
    }
}

impl SceneLayer for LifeformTrailLayer {
    fn name(&self) -> &'static str {
        "lifeform_wakes"
    }

    fn render(&mut self, canvas: &mut Canvas, _frame: SceneFrame<'_>) {
        self.field.borrow().render_trails(canvas);
    }
}

#[derive(Clone, Debug, PartialEq)]
struct LifeformSeedLayer {
    field: Rc<RefCell<LifeformField>>,
}

impl LifeformSeedLayer {
    fn new(field: Rc<RefCell<LifeformField>>) -> Self {
        Self { field }
    }
}

impl SceneLayer for LifeformSeedLayer {
    fn name(&self) -> &'static str {
        "glow_lifeforms"
    }

    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>) {
        let mut field = self.field.borrow_mut();
        field.update(frame.tick(), frame.activity());
        field.render_seeds(canvas);
    }
}

fn draw_deep_water(tick: u64, activity: &SceneActivity, canvas: &mut Canvas) {
    let memory_energy = activity.memory_pressure();
    let cpu_energy = average(activity.core_loads());
    let width = canvas.width();
    let height = canvas.height();

    // Whole-frame base layers write through the backing slice to avoid repeated
    // bounds checks and dirty-region updates for every pixel.
    for (index, pixel) in canvas.pixels_mut().iter_mut().enumerate() {
        let x = (index % usize::from(width)) as u16;
        let y = (index / usize::from(width)) as u16;
        let fx = f32::from(x) / f32::from(width.max(1));
        let fy = f32::from(y) / f32::from(height.max(1));
        let drift = tick as f32 * 0.018;
        let slow_wave = wave01(fx * 8.0 + fy * 2.2 + drift);
        let caustic = wave01(fx * 18.0 - fy * 7.0 - drift * 1.7);
        let depth = fy.powf(1.35);
        let glow = slow_wave * 0.18 + caustic * 0.08 + cpu_energy * 0.08;

        let r = scale_channel(2.0 + depth * 9.0 + glow * 18.0);
        let g = scale_channel(12.0 + depth * 34.0 + glow * 52.0 + memory_energy * 10.0);
        let b = scale_channel(32.0 + depth * 78.0 + slow_wave * 38.0 + caustic * 18.0);

        *pixel = Rgba::rgb(r, g, b);
    }
}

fn draw_reef_growth(tick: u64, activity: &SceneActivity, canvas: &mut Canvas) {
    let memory_energy = activity.memory_pressure();
    let width = canvas.width();
    let height = canvas.height();
    let reef_start = f32::from(height) * (0.58 - memory_energy * 0.16);

    for (index, pixel) in canvas.pixels_mut().iter_mut().enumerate() {
        let x = (index % usize::from(width)) as u16;
        let y = (index / usize::from(width)) as u16;
        let fy = f32::from(y);
        if fy < reef_start {
            continue;
        }

        let fx = f32::from(x) / f32::from(width.max(1));
        let bottom_weight =
            ((fy - reef_start) / (f32::from(height).max(1.0) - reef_start)).clamp(0.0, 1.0);
        let branch = wave01(fx * 24.0 + bottom_weight * 7.0 + tick as f32 * 0.01);
        let frond = wave01(fx * 43.0 - bottom_weight * 14.0);
        let density = (branch * 0.65 + frond * 0.35) * bottom_weight;
        if density < 0.38 {
            continue;
        }

        let glow = (density - 0.38) / 0.62 * (0.35 + memory_energy * 1.25);
        pixel.r = add_channel(pixel.r, glow * 8.0);
        pixel.g = add_channel(pixel.g, glow * 82.0);
        pixel.b = add_channel(pixel.b, glow * 54.0);
    }
}

fn draw_current_bands(tick: u64, activity: &SceneActivity, canvas: &mut Canvas) {
    let download = activity.network_download();
    let upload = activity.network_upload();
    let network_energy = download.max(upload);
    let width = canvas.width();
    let height = canvas.height();
    let direction = if upload > download { -1.0 } else { 1.0 };
    let base_motion = tick as f32 * (0.025 + network_energy * 0.16) * direction;

    for (index, pixel) in canvas.pixels_mut().iter_mut().enumerate() {
        let x = (index % usize::from(width)) as u16;
        let y = (index / usize::from(width)) as u16;
        let fx = f32::from(x) / f32::from(width.max(1));
        let fy = f32::from(y) / f32::from(height.max(1));
        let ribbon = wave01(fx * 9.0 + fy * 16.0 + base_motion);
        let shimmer = wave01(fx * 31.0 - fy * 11.0 - base_motion * 1.8);
        let band = (ribbon * 0.75 + shimmer * 0.25).powf(2.2);
        let energy = 0.12 + network_energy * 1.15;

        pixel.g = add_channel(pixel.g, band * energy * 18.0);
        pixel.b = add_channel(pixel.b, band * energy * 44.0);
    }
}

fn render_lifeform_seed(canvas: &mut Canvas, seed: &LifeformSeed) {
    let center_x = seed.x.round() as i32;
    let center_y = seed.y.round() as i32;
    let radius = 4_i32;

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let distance = ((dx * dx + dy * dy) as f32).sqrt();
            let influence = (1.0 - distance / (radius as f32 + 0.75)).clamp(0.0, 1.0);
            if influence <= 0.0 {
                continue;
            }

            let x = center_x + dx;
            let y = center_y + dy;
            if x < 0 || y < 0 {
                continue;
            }

            let x = x as u16;
            let y = y as u16;
            let Some(current) = canvas.pixel(x, y) else {
                continue;
            };
            let halo = influence.powf(2.0);
            let core = if distance <= 1.0 { 1.0 } else { 0.0 };
            let energy = seed.energy * (halo + core * 0.65);
            let next = Rgba::rgb(
                add_channel(current.r, 20.0 * energy),
                add_channel(current.g, 145.0 * energy),
                add_channel(current.b, 125.0 * energy),
            );
            let _ = canvas.set_pixel(x, y, next);
        }
    }
}

fn render_lifeform_trail_point(canvas: &mut Canvas, point: &LifeformTrailPoint, age: usize) {
    let center_x = point.x.round() as i32;
    let center_y = point.y.round() as i32;
    let radius = 2_i32;
    let age_falloff = 1.0 / (age as f32 + 1.0);

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let x = center_x + dx;
            let y = center_y + dy;
            if x < 0 || y < 0 {
                continue;
            }

            let x = x as u16;
            let y = y as u16;
            let Some(current) = canvas.pixel(x, y) else {
                continue;
            };

            let distance = ((dx * dx + dy * dy) as f32).sqrt();
            let influence = (1.0 - distance / 2.0).clamp(0.0, 1.0);
            if influence <= 0.0 {
                continue;
            }

            let energy = point.intensity * age_falloff * influence;
            let next = Rgba::rgb(
                add_channel(current.r, 8.0 * energy),
                add_channel(current.g, 72.0 * energy),
                add_channel(current.b, 74.0 * energy),
            );
            let _ = canvas.set_pixel(x, y, next);
        }
    }
}

fn draw_sediment_sparks(tick: u64, activity: &SceneActivity, canvas: &mut Canvas) {
    let disk_energy = activity.disk_read().max(activity.disk_write());
    if disk_energy <= 0.0 {
        return;
    }

    let width = canvas.width().max(1);
    let height = canvas.height().max(1);
    let spark_count = (4.0 + disk_energy * 18.0).round() as u16;

    for index in 0..spark_count {
        let seed = tick
            .wrapping_mul(37)
            .wrapping_add(u64::from(index) * 91)
            .wrapping_add(17);
        let x = (seed.wrapping_mul(53) % u64::from(width)) as u16;
        let lower_band = u64::from((height / 3).max(1));
        let y = (u64::from(height) - 1 - (seed.wrapping_mul(29) % lower_band)) as u16;
        let intensity = disk_energy * (0.55 + f32::from(index % 5) * 0.1);
        add_glow_point(canvas, x, y, 2, intensity);
    }
}

fn average(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }

    values.iter().sum::<f32>() / values.len() as f32
}

fn add_glow_point(canvas: &mut Canvas, center_x: u16, center_y: u16, radius: i32, energy: f32) {
    let center_x = i32::from(center_x);
    let center_y = i32::from(center_y);

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let distance = ((dx * dx + dy * dy) as f32).sqrt();
            let influence = (1.0 - distance / (radius as f32 + 0.5)).clamp(0.0, 1.0);
            if influence <= 0.0 {
                continue;
            }

            let x = center_x + dx;
            let y = center_y + dy;
            if x < 0 || y < 0 {
                continue;
            }

            let x = x as u16;
            let y = y as u16;
            let Some(current) = canvas.pixel(x, y) else {
                continue;
            };

            let glow = energy * influence.powf(1.6);
            let next = Rgba::rgb(
                add_channel(current.r, glow * 190.0),
                add_channel(current.g, glow * 126.0),
                add_channel(current.b, glow * 42.0),
            );
            let _ = canvas.set_pixel(x, y, next);
        }
    }
}

fn wave01(phase: f32) -> f32 {
    (phase.sin() + 1.0) * 0.5
}

fn scale_channel(value: f32) -> u8 {
    value.clamp(0.0, 255.0).round() as u8
}

fn add_channel(base: u8, value: f32) -> u8 {
    scale_channel(f32::from(base) + value)
}

fn wrap(value: f32, limit: f32) -> f32 {
    if limit <= 1.0 {
        return 0.0;
    }

    value.rem_euclid(limit)
}
