use crate::{
    canvas::{Canvas, CanvasError, Rgba},
    simulation::SceneActivity,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProbeCanvasConfig {
    pub width: u16,
    pub height: u16,
}

impl ProbeCanvasConfig {
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

pub struct ProbeScene {
    scene: LayeredScene,
}

impl ProbeScene {
    pub fn new(config: ProbeCanvasConfig) -> Result<Self, CanvasError> {
        Ok(Self {
            scene: LayeredScene::new(config, probe_layers())?,
        })
    }

    pub fn render(&mut self, tick: u64, activity: &SceneActivity) -> &Canvas {
        self.scene.render(tick, activity)
    }

    pub fn layer_names(&self) -> Vec<&'static str> {
        self.scene.layer_names()
    }
}

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
        config: ProbeCanvasConfig,
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
struct BackgroundFieldLayer;

impl SceneLayer for BackgroundFieldLayer {
    fn name(&self) -> &'static str {
        "background_field"
    }

    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>) {
        draw_background_field(frame.activity(), canvas);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ActivityPulseLayer;

impl SceneLayer for ActivityPulseLayer {
    fn name(&self) -> &'static str {
        "activity_pulse"
    }

    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>) {
        draw_activity_pulse(frame.tick(), frame.activity(), canvas);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FlowTintLayer;

impl SceneLayer for FlowTintLayer {
    fn name(&self) -> &'static str {
        "flow_tint"
    }

    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>) {
        draw_flow_tint(frame.tick(), frame.activity(), canvas);
    }
}

pub fn build_probe_canvas(
    config: ProbeCanvasConfig,
    tick: u64,
    activity: &SceneActivity,
) -> Result<Canvas, CanvasError> {
    let mut canvas = Canvas::new(config.width, config.height, Rgba::rgb(0, 0, 0))?;
    for mut layer in probe_layers() {
        layer.render(&mut canvas, SceneFrame::new(tick, activity));
    }
    canvas.clear_dirty();
    Ok(canvas)
}

fn probe_layers() -> Vec<Box<dyn SceneLayer>> {
    vec![
        Box::new(BackgroundFieldLayer),
        Box::new(ActivityPulseLayer),
        Box::new(FlowTintLayer),
    ]
}

fn draw_background_field(activity: &SceneActivity, canvas: &mut Canvas) {
    let memory_energy = activity.memory_pressure();
    let width = canvas.width();
    let height = canvas.height();

    // Whole-frame base layers write through the backing slice to avoid repeated
    // bounds checks and dirty-region updates for every pixel.
    for (index, pixel) in canvas.pixels_mut().iter_mut().enumerate() {
        let x = (index % usize::from(width)) as u16;
        let y = (index / usize::from(width)) as u16;
        let fx = f32::from(x) / f32::from(width.max(1));
        let fy = f32::from(y) / f32::from(height.max(1));

        let r = scale_channel(12.0);
        let g = scale_channel(24.0 + fy * 80.0 + memory_energy * 85.0);
        let b = scale_channel(42.0 + fx * 95.0);

        *pixel = Rgba::rgb(r, g, b);
    }
}

fn draw_activity_pulse(tick: u64, activity: &SceneActivity, canvas: &mut Canvas) {
    let cpu_energy = average(activity.core_loads());
    let disk_energy = activity.disk_read().max(activity.disk_write());
    let width = canvas.width();
    let height = canvas.height();

    for (index, pixel) in canvas.pixels_mut().iter_mut().enumerate() {
        let x = (index % usize::from(width)) as u16;
        let y = (index / usize::from(width)) as u16;
        let pulse = radial_pulse(width, height, x, y, tick, cpu_energy);
        pixel.r = add_channel(pixel.r, pulse * 155.0 + disk_energy * 80.0);
    }
}

fn draw_flow_tint(tick: u64, activity: &SceneActivity, canvas: &mut Canvas) {
    let network_energy = activity.network_download().max(activity.network_upload());
    let width = canvas.width();

    if network_energy <= 0.0 {
        return;
    }

    for (index, pixel) in canvas.pixels_mut().iter_mut().enumerate() {
        let x = (index % usize::from(width)) as u16;
        let wave = (((f32::from(x) * 0.08) + (tick as f32 * 0.18)).sin() + 1.0) * 0.5;
        pixel.b = add_channel(pixel.b, wave * network_energy * 90.0);
    }
}

fn average(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }

    values.iter().sum::<f32>() / values.len() as f32
}

fn radial_pulse(width: u16, height: u16, x: u16, y: u16, tick: u64, energy: f32) -> f32 {
    if energy <= 0.0 {
        return 0.0;
    }

    let center_x = f32::from(width) * (0.5 + ((tick as f32 * 0.04).sin() * 0.18));
    let center_y = f32::from(height) * 0.52;
    let dx = f32::from(x) - center_x;
    let dy = f32::from(y) - center_y;
    let distance = (dx * dx + dy * dy).sqrt();
    let radius = f32::from(width.min(height)).max(1.0) * (0.18 + energy * 0.35);

    (1.0 - (distance / radius)).clamp(0.0, 1.0) * energy
}

fn scale_channel(value: f32) -> u8 {
    value.clamp(0.0, 255.0).round() as u8
}

fn add_channel(base: u8, value: f32) -> u8 {
    scale_channel(f32::from(base) + value)
}
