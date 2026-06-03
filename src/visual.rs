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
            scene: LayeredScene::new(config, vec![Box::new(ProbeLayer)])?,
        })
    }

    pub fn render(&mut self, tick: u64, activity: &SceneActivity) -> &Canvas {
        self.scene.render(tick, activity)
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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ProbeLayer;

impl SceneLayer for ProbeLayer {
    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>) {
        draw_probe_canvas(frame.tick(), frame.activity(), canvas);
    }
}

pub fn build_probe_canvas(
    config: ProbeCanvasConfig,
    tick: u64,
    activity: &SceneActivity,
) -> Result<Canvas, CanvasError> {
    let mut canvas = Canvas::new(config.width, config.height, Rgba::rgb(0, 0, 0))?;
    draw_probe_canvas(tick, activity, &mut canvas);
    canvas.clear_dirty();
    Ok(canvas)
}

fn draw_probe_canvas(tick: u64, activity: &SceneActivity, canvas: &mut Canvas) {
    let cpu_energy = average(activity.core_loads());
    let network_energy = activity.network_download().max(activity.network_upload());
    let memory_energy = activity.memory_pressure();
    let disk_energy = activity.disk_read().max(activity.disk_write());
    let width = canvas.width();
    let height = canvas.height();

    // Whole-frame renderers write through the backing slice to avoid repeated
    // bounds checks and dirty-region updates for every pixel.
    for (index, pixel) in canvas.pixels_mut().iter_mut().enumerate() {
        let x = (index % usize::from(width)) as u16;
        let y = (index / usize::from(width)) as u16;
        let fx = f32::from(x) / f32::from(width.max(1));
        let fy = f32::from(y) / f32::from(height.max(1));
        let wave = (((f32::from(x) * 0.08) + (tick as f32 * 0.18)).sin() + 1.0) * 0.5;
        let pulse = radial_pulse(width, height, x, y, tick, cpu_energy);

        let r = scale_channel(12.0 + pulse * 155.0 + disk_energy * 80.0);
        let g = scale_channel(24.0 + fy * 80.0 + memory_energy * 85.0);
        let b = scale_channel(42.0 + fx * 95.0 + wave * network_energy * 90.0);

        *pixel = Rgba::rgb(r, g, b);
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
