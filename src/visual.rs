use std::{cell::RefCell, rc::Rc};

use crate::{
    canvas::{Canvas, CanvasError, DirtyRegion, Rgba},
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
    canvas: Canvas,
    environment: EnvironmentLayer,
    sparse_layers: Vec<Box<dyn SceneLayer>>,
    previous_dynamic_dirty: Option<DirtyRegion>,
}

impl TidepoolScene {
    pub fn new(config: TidepoolCanvasConfig) -> Result<Self, CanvasError> {
        Ok(Self {
            canvas: Canvas::new(config.width, config.height, Rgba::rgb(0, 0, 0))?,
            environment: EnvironmentLayer::new(config)?,
            sparse_layers: tidepool_sparse_layers(config),
            previous_dynamic_dirty: None,
        })
    }

    pub fn render(&mut self, tick: u64, activity: &SceneActivity) -> &Canvas {
        let frame = SceneFrame::new(tick, activity);
        self.canvas.clear_dirty();

        let environment_refreshed = self.environment.render_environment(&mut self.canvas, frame);
        if !environment_refreshed && let Some(region) = self.previous_dynamic_dirty {
            self.environment.restore_region(&mut self.canvas, region);
        }

        for layer in &mut self.sparse_layers {
            layer.render(&mut self.canvas, frame);
        }

        self.previous_dynamic_dirty = self.canvas.dirty_region();
        if environment_refreshed {
            self.canvas.mark_full_frame_required();
        }

        &self.canvas
    }

    pub fn layer_names(&self) -> Vec<&'static str> {
        self.environment
            .layer_names()
            .into_iter()
            .chain(
                self.sparse_layers
                    .iter()
                    .flat_map(|layer| layer.layer_names()),
            )
            .collect()
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

    fn layer_names(&self) -> Vec<&'static str> {
        vec![self.name()]
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
        self.canvas.clear_dirty();
        let frame = SceneFrame::new(tick, activity);
        for layer in &mut self.layers {
            layer.render(&mut self.canvas, frame);
        }
        &self.canvas
    }

    pub fn layer_names(&self) -> Vec<&'static str> {
        self.layers
            .iter()
            .flat_map(|layer| layer.layer_names())
            .collect()
    }
}

const ENVIRONMENT_REFRESH_TICKS: u64 = 8;

#[derive(Clone, Debug, PartialEq)]
struct EnvironmentLayer {
    cache: Canvas,
    surface_light: SurfaceLightCache,
    refresh_tick: Option<u64>,
}

impl EnvironmentLayer {
    fn new(config: TidepoolCanvasConfig) -> Result<Self, CanvasError> {
        Ok(Self {
            cache: Canvas::new(config.width, config.height, Rgba::rgb(0, 0, 0))?,
            surface_light: SurfaceLightCache::default(),
            refresh_tick: None,
        })
    }

    fn render_environment(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>) -> bool {
        draw_environment(frame.tick(), frame.activity(), canvas, self)
    }

    fn restore_region(&self, canvas: &mut Canvas, region: DirtyRegion) {
        canvas
            .copy_region_from(&self.cache, region)
            .expect("environment cache matches render canvas dimensions");
    }
}

impl SceneLayer for EnvironmentLayer {
    fn name(&self) -> &'static str {
        "environment"
    }

    fn layer_names(&self) -> Vec<&'static str> {
        vec![
            "deep_water",
            "surface_light",
            "reef_growth",
            "current_bands",
        ]
    }

    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>) {
        draw_environment(frame.tick(), frame.activity(), canvas, self);
    }
}

const SURFACE_LIGHT_REFRESH_TICKS: u64 = ENVIRONMENT_REFRESH_TICKS;

#[derive(Clone, Debug, Default, PartialEq)]
struct SurfaceLightCache {
    width: u16,
    height: u16,
    refresh_tick: Option<u64>,
    samples: Vec<SurfaceLightSample>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct SurfaceLightSample {
    r_mul: u16,
    g_mul: u16,
    b_mul: u16,
    r_add: u8,
    g_add: u8,
    b_add: u8,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DriftMotesLayer;

impl SceneLayer for DriftMotesLayer {
    fn name(&self) -> &'static str {
        "drift_motes"
    }

    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>) {
        draw_drift_motes(frame.tick(), frame.activity(), canvas);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ReefPolypsLayer;

impl SceneLayer for ReefPolypsLayer {
    fn name(&self) -> &'static str {
        "reef_polyps"
    }

    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>) {
        draw_reef_polyps(frame.tick(), frame.activity(), canvas);
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

fn tidepool_sparse_layers(config: TidepoolCanvasConfig) -> Vec<Box<dyn SceneLayer>> {
    // Trails and seeds are separate visual layers, but they must share motion
    // state so the afterglow follows the actual lifeform positions.
    let lifeforms = Rc::new(RefCell::new(LifeformField::new(14, config)));
    vec![
        Box::new(DriftMotesLayer),
        Box::new(ReefPolypsLayer),
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
    pub heading_x: f32,
    pub heading_y: f32,
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
            let (heading_x, heading_y) = normalize_direction(vx, vy);
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
                heading_x,
                heading_y,
                energy,
                pulse: 0.0,
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
            let move_x = (seed.vx + drift) * speed;
            let move_y = (seed.vy - drift * 0.6) * speed;
            let (heading_x, heading_y) = normalize_direction(move_x, move_y);
            for point in &mut seed.trail {
                point.intensity *= 0.78;
            }
            seed.x = wrap(seed.x + move_x, width);
            seed.y = wrap(seed.y + move_y, height);
            seed.heading_x = blend(seed.heading_x, heading_x, 0.28);
            seed.heading_y = blend(seed.heading_y, heading_y, 0.28);
            let (heading_x, heading_y) = normalize_direction(seed.heading_x, seed.heading_y);
            seed.heading_x = heading_x;
            seed.heading_y = heading_y;
            seed.energy =
                (0.42 + cpu_energy * 0.38 + flow_energy * 0.22 + drift.abs()).clamp(0.2, 1.0);
            seed.pulse = wave01(tick as f32 * (0.13 + cpu_energy * 0.16) + index as f32 * 0.9);
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
                heading_x: seed.heading_x,
                heading_y: seed.heading_y,
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
    heading_x: f32,
    heading_y: f32,
    energy: f32,
    pulse: f32,
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

fn draw_environment(
    tick: u64,
    activity: &SceneActivity,
    canvas: &mut Canvas,
    layer: &mut EnvironmentLayer,
) -> bool {
    let refresh_tick = tick / ENVIRONMENT_REFRESH_TICKS * ENVIRONMENT_REFRESH_TICKS;

    if layer.cache.width() != canvas.width() || layer.cache.height() != canvas.height() {
        layer.cache = Canvas::new(canvas.width(), canvas.height(), Rgba::rgb(0, 0, 0))
            .expect("render target canvas has non-zero dimensions");
        layer.surface_light = SurfaceLightCache::default();
        layer.refresh_tick = None;
    }

    if layer.refresh_tick != Some(refresh_tick) {
        draw_deep_water(refresh_tick, activity, &mut layer.cache);
        draw_surface_light(
            refresh_tick,
            activity,
            &mut layer.cache,
            &mut layer.surface_light,
        );
        draw_reef_growth(refresh_tick, activity, &mut layer.cache);
        draw_current_bands(refresh_tick, activity, &mut layer.cache);
        layer.cache.clear_dirty();
        layer.refresh_tick = Some(refresh_tick);
        canvas.pixels_mut().copy_from_slice(layer.cache.pixels());
        return true;
    }

    false
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

fn draw_surface_light(
    tick: u64,
    activity: &SceneActivity,
    canvas: &mut Canvas,
    cache: &mut SurfaceLightCache,
) {
    let cpu_energy = average(activity.core_loads());
    let flow_energy = activity.network_download().max(activity.network_upload());
    let width = canvas.width();
    let height = canvas.height();
    let refresh_tick = tick / SURFACE_LIGHT_REFRESH_TICKS * SURFACE_LIGHT_REFRESH_TICKS;

    if cache.width != width || cache.height != height || cache.refresh_tick != Some(refresh_tick) {
        refresh_surface_light_cache(refresh_tick, flow_energy, width, height, cache);
    }

    for (pixel, sample) in canvas.pixels_mut().iter_mut().zip(cache.samples.iter()) {
        pixel.r = apply_light_sample_channel(pixel.r, sample.r_mul, sample.r_add);
        pixel.g = apply_light_sample_channel(pixel.g, sample.g_mul, sample.g_add);
        pixel.b = apply_light_sample_channel(pixel.b, sample.b_mul, sample.b_add);
    }

    draw_surface_glints(refresh_tick, cpu_energy.max(flow_energy), canvas);
}

fn refresh_surface_light_cache(
    tick: u64,
    flow_energy: f32,
    width: u16,
    height: u16,
    cache: &mut SurfaceLightCache,
) {
    cache.width = width;
    cache.height = height;
    cache.refresh_tick = Some(tick);
    cache.samples.resize(
        usize::from(width) * usize::from(height),
        SurfaceLightSample::default(),
    );

    let drift = tick as f32 * (0.012 + flow_energy * 0.028);

    // Expensive shaft/shimmer math runs at the environment cadence. Sparse
    // life layers carry frame-rate motion between these heavier refreshes.
    for (index, sample) in cache.samples.iter_mut().enumerate() {
        let x = (index % usize::from(width)) as u16;
        let y = (index / usize::from(width)) as u16;
        let fx = f32::from(x) / f32::from(width.max(1));
        let fy = f32::from(y) / f32::from(height.max(1));
        let center_x = (1.0 - ((fx - 0.5).abs() * 1.65)).clamp(0.0, 1.0);
        let center_y = (1.0 - ((fy - 0.42).abs() * 1.05)).clamp(0.0, 1.0);
        let focus = (center_x * center_y).clamp(0.0, 1.0);
        let surface = (1.0 - fy * 1.7).clamp(0.0, 1.0);
        let top_glow = surface * surface;
        let shaft_a = wave01(fx * 7.2 + fy * 4.4 + drift);
        let shaft_b = wave01(fx * 15.0 - fy * 8.5 - drift * 1.8);
        let shaft = (shaft_a * 0.7 + shaft_b * 0.3).powf(2.4) * (1.0 - fy).clamp(0.0, 1.0);
        let shimmer = wave01(fx * 48.0 + drift * 8.0) * top_glow;
        let shade = 0.76 + focus * 0.18 + top_glow * 0.12;

        *sample = SurfaceLightSample {
            r_mul: fixed_light_mul(shade),
            g_mul: fixed_light_mul(shade),
            b_mul: fixed_light_mul(shade + top_glow * 0.04),
            r_add: scale_channel(shaft * 10.0 + shimmer * 8.0),
            g_add: scale_channel(shaft * 34.0 + shimmer * 28.0),
            b_add: scale_channel(shaft * 48.0 + shimmer * 34.0),
        };
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

fn draw_drift_motes(tick: u64, activity: &SceneActivity, canvas: &mut Canvas) {
    let width = canvas.width().max(1);
    let height = canvas.height().max(1);
    let area = u32::from(width) * u32::from(height);
    let mote_count = (area / 1_200).clamp(10, 40);
    let download = activity.network_download();
    let upload = activity.network_upload();
    let flow = download.max(upload);
    let direction = if upload > download { -1.0 } else { 1.0 };
    let travel = tick as f32 * (0.09 + flow * 0.24) * direction;
    let water_top = f32::from(height) * 0.12;
    let water_span = f32::from(height) * 0.58;

    // Motes are intentionally sparse entity-like marks, not a fourth full-frame
    // scan, so the layer adds motion without changing the terminal bandwidth.
    for index in 0..mote_count {
        let seed = u64::from(index)
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(0xA511_E9B3);
        let phase = seed as f32 * 0.000_031 + tick as f32 * 0.035;
        let base_x = (seed.wrapping_mul(47) % u64::from(width)) as f32;
        let base_y = water_top
            + (seed.wrapping_mul(29) % u64::from(height)) as f32 / f32::from(height) * water_span;
        let lane = (index % 5) as f32;
        let x = wrap(base_x + travel * (0.45 + lane * 0.09), f32::from(width));
        let y = (base_y + phase.sin() * (1.2 + flow * 2.4)).clamp(0.0, f32::from(height - 1));
        let pulse = wave01(phase * 1.7 + lane);
        let energy = 0.18 + pulse * 0.24 + flow * 0.22;

        add_cyan_glow_point(canvas, x.round() as u16, y.round() as u16, 1, energy);
    }
}

fn draw_reef_polyps(tick: u64, activity: &SceneActivity, canvas: &mut Canvas) {
    let width = canvas.width().max(1);
    let height = canvas.height().max(1);
    let area = u32::from(width) * u32::from(height);
    let polyp_count = (area / 1_700).clamp(10, 32);
    let memory_energy = activity.memory_pressure();
    let cpu_energy = average(activity.core_loads());
    let lower_band = (u64::from(height) / 4).max(1);

    for index in 0..polyp_count {
        let seed = u64::from(index)
            .wrapping_mul(0x85EB_CA6B)
            .wrapping_add(0xC2B2_AE35);
        let base_x = (seed.wrapping_mul(41) % u64::from(width)) as f32;
        let base_y = f32::from(height - 1) - (seed.wrapping_mul(17) % lower_band) as f32;
        let height_seed = (seed.wrapping_mul(13) % 7) as f32;
        let stalk_height = 4.0 + height_seed * 0.7 + memory_energy * 4.2;
        let sway_phase = tick as f32 * (0.035 + cpu_energy * 0.025) + seed as f32 * 0.000_021;
        let segments = stalk_height.round() as i32;

        for segment in 0..=segments {
            let progress = segment as f32 / stalk_height.max(1.0);
            let sway = (sway_phase + progress * 1.9).sin() * progress * (0.6 + cpu_energy * 1.4);
            let x = base_x + sway;
            let y = base_y - segment as f32;
            let energy = (0.22 + memory_energy * 0.38) * (1.0 - progress * 0.35);

            add_reef_polyp_pixel(canvas, x.round() as i32, y.round() as i32, energy);
        }

        let tip_progress = segments as f32 / stalk_height.max(1.0);
        let tip_sway =
            (sway_phase + tip_progress * 1.9).sin() * tip_progress * (0.6 + cpu_energy * 1.4);
        let tip_x = (base_x + tip_sway).round() as i32;
        let tip_y = (base_y - segments as f32).round() as i32;
        if tip_x >= 0 && tip_y >= 0 {
            let pulse = wave01(sway_phase * 1.8 + height_seed);
            let energy = 0.26 + memory_energy * 0.34 + cpu_energy * 0.18 + pulse * 0.16;
            add_polyp_tip_glow(canvas, tip_x as u16, tip_y as u16, energy);
        }
    }
}

fn render_lifeform_seed(canvas: &mut Canvas, seed: &LifeformSeed) {
    let center_x = seed.x.round() as i32;
    let center_y = seed.y.round() as i32;
    let radius = 6_i32;
    let side_x = -seed.heading_y;
    let side_y = seed.heading_x;
    let body_length = 5.8 + seed.energy * 2.4;
    let body_width = 1.2 + seed.pulse * 0.45;

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let local_x = dx as f32 * seed.heading_x + dy as f32 * seed.heading_y;
            let local_y = dx as f32 * side_x + dy as f32 * side_y;
            let body = directional_body_influence(local_x, local_y, body_length, body_width);
            let halo_distance = ((dx * dx + dy * dy) as f32).sqrt();
            let halo = (1.0 - halo_distance / (radius as f32 + 1.0)).clamp(0.0, 1.0) * 0.22;
            let influence = body.max(halo);
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
            let head = head_influence(local_x, local_y, body_length, body_width);
            let tail = tail_influence(local_x, local_y, body_length);
            let energy = seed.energy * (influence.powf(1.35) + head * 0.85 + tail * 0.28);
            let next = Rgba::rgb(
                add_channel(current.r, (16.0 + head * 18.0 + tail * 10.0) * energy),
                add_channel(
                    current.g,
                    (118.0 + seed.pulse * 46.0 + head * 32.0) * energy,
                ),
                add_channel(
                    current.b,
                    (110.0 + seed.pulse * 42.0 + tail * 20.0) * energy,
                ),
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

fn directional_body_influence(local_x: f32, local_y: f32, length: f32, width: f32) -> f32 {
    let forward = (1.0 - (local_x.abs() / length)).clamp(0.0, 1.0);
    let side = (1.0 - (local_y.abs() / width)).clamp(0.0, 1.0);
    (forward * side).powf(0.85)
}

fn head_influence(local_x: f32, local_y: f32, length: f32, width: f32) -> f32 {
    let head_x = length * 0.36;
    let dx = (local_x - head_x) / (width * 1.15);
    let dy = local_y / (width * 0.95);
    (1.0 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0)
}

fn tail_influence(local_x: f32, local_y: f32, length: f32) -> f32 {
    if local_x >= 0.0 {
        return 0.0;
    }

    let taper = (1.0 - ((local_x + length * 0.45).abs() / (length * 0.55))).clamp(0.0, 1.0);
    let centerline = (1.0 - local_y.abs() / 0.9).clamp(0.0, 1.0);
    taper * centerline
}

fn normalize_direction(x: f32, y: f32) -> (f32, f32) {
    let magnitude = (x * x + y * y).sqrt();
    if magnitude <= f32::EPSILON {
        return (1.0, 0.0);
    }

    (x / magnitude, y / magnitude)
}

fn blend(current: f32, target: f32, response: f32) -> f32 {
    current + (target - current) * response.clamp(0.0, 1.0)
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

fn add_cyan_glow_point(
    canvas: &mut Canvas,
    center_x: u16,
    center_y: u16,
    radius: i32,
    energy: f32,
) {
    let center_x = i32::from(center_x);
    let center_y = i32::from(center_y);

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let distance = ((dx * dx + dy * dy) as f32).sqrt();
            let influence = (1.0 - distance / (radius as f32 + 0.35)).clamp(0.0, 1.0);
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

            let glow = energy * influence.powf(1.45);
            let next = Rgba::rgb(
                add_channel(current.r, glow * 18.0),
                add_channel(current.g, glow * 118.0),
                add_channel(current.b, glow * 142.0),
            );
            let _ = canvas.set_pixel(x, y, next);
        }
    }
}

fn add_reef_polyp_pixel(canvas: &mut Canvas, x: i32, y: i32, energy: f32) {
    if x < 0 || y < 0 {
        return;
    }

    let x = x as u16;
    let y = y as u16;
    let Some(current) = canvas.pixel(x, y) else {
        return;
    };

    let next = Rgba::rgb(
        add_channel(current.r, energy * 16.0),
        add_channel(current.g, energy * 92.0),
        add_channel(current.b, energy * 58.0),
    );
    let _ = canvas.set_pixel(x, y, next);
}

fn add_polyp_tip_glow(canvas: &mut Canvas, center_x: u16, center_y: u16, energy: f32) {
    let center_x = i32::from(center_x);
    let center_y = i32::from(center_y);

    for dy in -1..=1 {
        for dx in -1..=1 {
            let distance = ((dx * dx + dy * dy) as f32).sqrt();
            let influence = (1.0 - distance / 1.45).clamp(0.0, 1.0);
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

            let glow = energy * influence.powf(1.35);
            let next = Rgba::rgb(
                add_channel(current.r, glow * 82.0),
                add_channel(current.g, glow * 142.0),
                add_channel(current.b, glow * 78.0),
            );
            let _ = canvas.set_pixel(x, y, next);
        }
    }
}

fn draw_surface_glints(tick: u64, energy: f32, canvas: &mut Canvas) {
    let width = canvas.width().max(1);
    let height = canvas.height().max(1);
    let glint_count = (6.0 + energy * 10.0).round() as u16;
    let upper_band = (height / 7).max(1);

    for index in 0..glint_count {
        let seed = u64::from(index)
            .wrapping_mul(0x27D4_EB2D)
            .wrapping_add(tick / 3);
        let x = (seed.wrapping_mul(59) % u64::from(width)) as u16;
        let y = (seed.wrapping_mul(23) % u64::from(upper_band)) as u16;
        let pulse = wave01(tick as f32 * 0.11 + index as f32 * 1.7);
        add_surface_glint(canvas, x, y, 0.22 + pulse * 0.24 + energy * 0.16);
    }
}

fn add_surface_glint(canvas: &mut Canvas, center_x: u16, center_y: u16, energy: f32) {
    let center_x = i32::from(center_x);
    let center_y = i32::from(center_y);

    for dx in -2..=2 {
        let x = center_x + dx;
        if x < 0 || center_y < 0 {
            continue;
        }

        let x = x as u16;
        let y = center_y as u16;
        let Some(current) = canvas.pixel(x, y) else {
            continue;
        };

        let influence = (1.0 - (dx.abs() as f32 / 2.4)).clamp(0.0, 1.0);
        let glow = energy * influence.powf(1.2);
        let next = Rgba::rgb(
            add_channel(current.r, glow * 60.0),
            add_channel(current.g, glow * 128.0),
            add_channel(current.b, glow * 148.0),
        );
        let _ = canvas.set_pixel(x, y, next);
    }
}

fn wave01(phase: f32) -> f32 {
    (phase.sin() + 1.0) * 0.5
}

fn scale_channel(value: f32) -> u8 {
    value.clamp(0.0, 255.0).round() as u8
}

fn fixed_light_mul(value: f32) -> u16 {
    (value.clamp(0.0, 1.25) * 256.0).round() as u16
}

fn apply_light_sample_channel(base: u8, multiplier: u16, addition: u8) -> u8 {
    let multiplied = (u32::from(base) * u32::from(multiplier) + 128) / 256;
    let value = multiplied + u32::from(addition);
    value.min(255) as u8
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_light_reuses_cached_field_between_refresh_ticks() {
        let activity = SceneActivity::from_core_loads(vec![0.45]).with_network_flow(0.35, 0.0);
        let fill = Rgba::rgb(8, 24, 56);
        let mut cache = SurfaceLightCache::default();

        let mut first = Canvas::new(32, 18, fill).expect("valid canvas");
        draw_surface_light(0, &activity, &mut first, &mut cache);
        let first_pixels = first.pixels().to_vec();

        let mut same_refresh_window = Canvas::new(32, 18, fill).expect("valid canvas");
        draw_surface_light(7, &activity, &mut same_refresh_window, &mut cache);

        assert_eq!(same_refresh_window.pixels(), first_pixels.as_slice());

        let mut next_refresh_window = Canvas::new(32, 18, fill).expect("valid canvas");
        draw_surface_light(8, &activity, &mut next_refresh_window, &mut cache);

        assert_ne!(next_refresh_window.pixels(), first_pixels.as_slice());
    }

    #[test]
    fn environment_layer_reuses_cached_background_between_refresh_ticks() {
        let activity = SceneActivity::from_core_loads(vec![0.45])
            .with_memory_pressure(0.55)
            .with_network_flow(0.35, 0.0);
        let mut layer =
            EnvironmentLayer::new(TidepoolCanvasConfig::new(32, 18)).expect("valid layer");

        let mut first = Canvas::new(32, 18, Rgba::rgb(0, 0, 0)).expect("valid canvas");
        assert!(layer.render_environment(&mut first, SceneFrame::new(0, &activity)));
        let first_pixels = first.pixels().to_vec();

        let mut same_refresh_window =
            Canvas::new(32, 18, Rgba::rgb(0, 0, 0)).expect("valid canvas");
        assert!(!layer.render_environment(&mut same_refresh_window, SceneFrame::new(7, &activity)));

        assert!(
            same_refresh_window
                .pixels()
                .iter()
                .all(|pixel| *pixel == Rgba::rgb(0, 0, 0))
        );
        assert_eq!(layer.cache.pixels(), first_pixels.as_slice());

        let mut next_refresh_window =
            Canvas::new(32, 18, Rgba::rgb(0, 0, 0)).expect("valid canvas");
        assert!(layer.render_environment(&mut next_refresh_window, SceneFrame::new(8, &activity)));

        assert_ne!(next_refresh_window.pixels(), first_pixels.as_slice());
    }
}
