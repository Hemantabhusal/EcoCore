use crate::{
    canvas::{Canvas, CanvasError, DirtyRegion, Rgba},
    simulation::SceneActivity,
};

use super::{
    math::{
        add_channel, apply_light_sample_channel, average, draw_surface_glints, fixed_light_mul,
        scale_channel, wave01,
    },
    scene::{SceneFrame, SceneLayer, TidepoolCanvasConfig},
};

pub(super) const ENVIRONMENT_REFRESH_TICKS: u64 = 8;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct EnvironmentLayer {
    cache: Canvas,
    surface_light: SurfaceLightCache,
    refresh_tick: Option<u64>,
}

impl EnvironmentLayer {
    pub(super) fn new(config: TidepoolCanvasConfig) -> Result<Self, CanvasError> {
        Ok(Self {
            cache: Canvas::new(config.width, config.height, Rgba::rgb(0, 0, 0))?,
            surface_light: SurfaceLightCache::default(),
            refresh_tick: None,
        })
    }

    pub(super) fn render_environment(
        &mut self,
        canvas: &mut Canvas,
        frame: SceneFrame<'_>,
    ) -> bool {
        draw_environment(frame.tick(), frame.activity(), canvas, self)
    }

    pub(super) fn restore_region(&self, canvas: &mut Canvas, region: DirtyRegion) {
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
        canvas.clear_dirty();
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

    #[test]
    fn environment_refresh_leaves_canvas_clean_for_sparse_dirty_tracking() {
        let activity = SceneActivity::from_core_loads(vec![0.45])
            .with_memory_pressure(0.55)
            .with_network_flow(0.35, 0.0);
        let mut layer =
            EnvironmentLayer::new(TidepoolCanvasConfig::new(32, 18)).expect("valid layer");
        let mut canvas = Canvas::new(32, 18, Rgba::rgb(0, 0, 0)).expect("valid canvas");

        assert!(layer.render_environment(&mut canvas, SceneFrame::new(0, &activity)));

        assert_eq!(canvas.dirty_region(), None);
        assert!(!canvas.full_frame_required());
    }
}
