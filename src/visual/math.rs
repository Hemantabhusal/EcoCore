use crate::canvas::{Canvas, Rgba};

pub(super) fn average(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }

    values.iter().sum::<f32>() / values.len() as f32
}

pub(super) fn directional_body_influence(
    local_x: f32,
    local_y: f32,
    length: f32,
    width: f32,
) -> f32 {
    let forward = (1.0 - (local_x.abs() / length)).clamp(0.0, 1.0);
    let side = (1.0 - (local_y.abs() / width)).clamp(0.0, 1.0);
    (forward * side).powf(0.85)
}

pub(super) fn head_influence(local_x: f32, local_y: f32, length: f32, width: f32) -> f32 {
    let head_x = length * 0.36;
    let dx = (local_x - head_x) / (width * 1.15);
    let dy = local_y / (width * 0.95);
    (1.0 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0)
}

pub(super) fn tail_influence(local_x: f32, local_y: f32, length: f32) -> f32 {
    if local_x >= 0.0 {
        return 0.0;
    }

    let taper = (1.0 - ((local_x + length * 0.45).abs() / (length * 0.55))).clamp(0.0, 1.0);
    let centerline = (1.0 - local_y.abs() / 0.9).clamp(0.0, 1.0);
    taper * centerline
}

pub(super) fn normalize_direction(x: f32, y: f32) -> (f32, f32) {
    let magnitude = (x * x + y * y).sqrt();
    if magnitude <= f32::EPSILON {
        return (1.0, 0.0);
    }

    (x / magnitude, y / magnitude)
}

pub(super) fn blend(current: f32, target: f32, response: f32) -> f32 {
    current + (target - current) * response.clamp(0.0, 1.0)
}

pub(super) fn add_glow_point(
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

pub(super) fn add_cyan_glow_point(
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

pub(super) fn add_reef_polyp_pixel(canvas: &mut Canvas, x: i32, y: i32, energy: f32) {
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

pub(super) fn add_polyp_tip_glow(canvas: &mut Canvas, center_x: u16, center_y: u16, energy: f32) {
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

pub(super) fn draw_surface_glints(tick: u64, energy: f32, canvas: &mut Canvas) {
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

pub(super) fn wave01(phase: f32) -> f32 {
    (phase.sin() + 1.0) * 0.5
}

pub(super) fn scale_channel(value: f32) -> u8 {
    value.clamp(0.0, 255.0).round() as u8
}

pub(super) fn fixed_light_mul(value: f32) -> u16 {
    (value.clamp(0.0, 1.25) * 256.0).round() as u16
}

pub(super) fn apply_light_sample_channel(base: u8, multiplier: u16, addition: u8) -> u8 {
    let multiplied = (u32::from(base) * u32::from(multiplier) + 128) / 256;
    let value = multiplied + u32::from(addition);
    value.min(255) as u8
}

pub(super) fn add_channel(base: u8, value: f32) -> u8 {
    scale_channel(f32::from(base) + value)
}

pub(super) fn wrap(value: f32, limit: f32) -> f32 {
    if limit <= 1.0 {
        return 0.0;
    }

    value.rem_euclid(limit)
}
