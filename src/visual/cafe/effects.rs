use crate::{
    canvas::{Canvas, Rgba},
    simulation::SceneActivity,
};

use super::background::{WindowGeometry, cafe_counter_top, draw_rect, window_geometry};

pub(super) fn render_counter_activity(canvas: &mut Canvas, tick: u64, activity: &SceneActivity) {
    let intensity = non_cpu_activity(activity);
    if intensity <= 0.05 {
        return;
    }

    // Keep non-CPU signals clustered near the counter. A wider particle field
    // would look lively, but it would dirty too many Kitty tiles.
    let counter_top = cafe_counter_top(canvas.height());
    let center_x = canvas.width() / 2;
    let cup_x = center_x.saturating_add(canvas.width() / 12);
    let cup_y = counter_top.saturating_add(12);
    let steam_puffs = 2 + (intensity * 4.0).round() as u16;
    let drift = ((activity.network_download() + activity.network_upload()) * 8.0).round() as i16;

    for puff in 0..steam_puffs.min(6) {
        let phase = ((tick / 4 + u64::from(puff) * 3) % 14) as u16;
        let x = offset_u16(
            cup_x,
            i16::try_from((puff % 3) * 7).unwrap_or(0) - 7 + drift / 2,
        );
        let y = cup_y.saturating_sub(10 + phase + puff * 4);
        draw_rect(canvas, x, y, 3 + puff % 2, 2, Rgba::rgb(207, 164, 122));
    }

    let disk_energy = activity.disk_read().max(activity.disk_write());
    let bubbles = (disk_energy * 5.0).round() as u16;
    for bubble in 0..bubbles.min(5) {
        let x = cup_x.saturating_add(16 + bubble * 4);
        let y = cup_y.saturating_sub(4 + ((tick as u16 + bubble * 3) % 10));
        draw_rect(canvas, x, y, 2, 2, Rgba::rgb(235, 178, 89));
    }

    let clutter = (activity.memory_pressure() * 3.0).round() as u16;
    for item in 0..clutter.min(3) {
        let x = cup_x.saturating_sub(24 + item * 9);
        draw_rect(
            canvas,
            x,
            cup_y.saturating_add(5),
            7,
            5,
            Rgba::rgb(188, 118, 58),
        );
    }
}

pub(super) fn render_window_rain(canvas: &mut Canvas, tick: u64, activity: &SceneActivity) {
    let WindowGeometry {
        x,
        y,
        width,
        height,
    } = window_geometry(canvas.width(), canvas.height());
    let network = activity.network_download().max(activity.network_upload());
    let streaks = 5 + (network * 5.0).round() as u16;
    let speed = 2 + (network * 4.0).round() as u16;

    // Keep rain sparse. Dense full-window weather looks better in a still
    // image, but it would dirty most of the window every frame.
    for index in 0..streaks.min(10) {
        let rain_x = x + 10 + ((index * 29 + (tick as u16 % 11)) % width.saturating_sub(20).max(1));
        let rain_y =
            y + 6 + (((index * 37) + (tick as u16 * speed)) % height.saturating_sub(16).max(1));
        draw_slanted_rain(canvas, rain_x, rain_y, 7, Rgba::rgb(96, 151, 190));
    }
}

fn non_cpu_activity(activity: &SceneActivity) -> f32 {
    activity
        .memory_pressure()
        .max(activity.network_download())
        .max(activity.network_upload())
        .max(activity.disk_read())
        .max(activity.disk_write())
}

fn offset_u16(value: u16, offset: i16) -> u16 {
    if offset.is_negative() {
        value.saturating_sub(offset.unsigned_abs())
    } else {
        value.saturating_add(offset as u16)
    }
}

fn draw_slanted_rain(canvas: &mut Canvas, x: u16, y: u16, length: u16, color: Rgba) {
    for offset in 0..length {
        let py = y + offset;
        let px = x.saturating_add(offset / 3);
        if px < canvas.width() && py < canvas.height() {
            canvas
                .set_pixel(px, py, color)
                .expect("rain pixel in bounds");
        }
    }
}
