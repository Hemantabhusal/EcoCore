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

pub fn build_probe_canvas(
    config: ProbeCanvasConfig,
    tick: u64,
    activity: &SceneActivity,
) -> Result<Canvas, CanvasError> {
    let mut canvas = Canvas::new(config.width, config.height, Rgba::rgb(0, 0, 0))?;
    let cpu_energy = average(activity.core_loads());
    let network_energy = activity.network_download().max(activity.network_upload());
    let memory_energy = activity.memory_pressure();
    let disk_energy = activity.disk_read().max(activity.disk_write());

    for y in 0..config.height {
        for x in 0..config.width {
            let fx = f32::from(x) / f32::from(config.width.max(1));
            let fy = f32::from(y) / f32::from(config.height.max(1));
            let wave = (((f32::from(x) * 0.08) + (tick as f32 * 0.18)).sin() + 1.0) * 0.5;
            let pulse = radial_pulse(config, x, y, tick, cpu_energy);

            let r = scale_channel(12.0 + pulse * 155.0 + disk_energy * 80.0);
            let g = scale_channel(24.0 + fy * 80.0 + memory_energy * 85.0);
            let b = scale_channel(42.0 + fx * 95.0 + wave * network_energy * 90.0);

            canvas.set_pixel(x, y, Rgba::rgb(r, g, b))?;
        }
    }

    canvas.clear_dirty();
    Ok(canvas)
}

fn average(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }

    values.iter().sum::<f32>() / values.len() as f32
}

fn radial_pulse(config: ProbeCanvasConfig, x: u16, y: u16, tick: u64, energy: f32) -> f32 {
    if energy <= 0.0 {
        return 0.0;
    }

    let center_x = f32::from(config.width) * (0.5 + ((tick as f32 * 0.04).sin() * 0.18));
    let center_y = f32::from(config.height) * 0.52;
    let dx = f32::from(x) - center_x;
    let dy = f32::from(y) - center_y;
    let distance = (dx * dx + dy * dy).sqrt();
    let radius = f32::from(config.width.min(config.height)).max(1.0) * (0.18 + energy * 0.35);

    (1.0 - (distance / radius)).clamp(0.0, 1.0) * energy
}

fn scale_channel(value: f32) -> u8 {
    value.clamp(0.0, 255.0).round() as u8
}
