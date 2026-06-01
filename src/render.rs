use crate::framebuffer::{Cell, Color, Framebuffer, FramebufferError};
use crate::simulation::SceneActivity;

const SKY: Color = Color::rgb(8, 18, 34);
const GROUND: Color = Color::rgb(35, 50, 35);
const WATER: Color = Color::rgb(35, 120, 210);
const CREATURE: Color = Color::rgb(255, 180, 80);
const BUSY_CREATURE: Color = Color::rgb(255, 95, 90);
const VEGETATION: Color = Color::rgb(95, 190, 105);

#[derive(Clone, Debug, PartialEq)]
pub struct VisualTheme {
    pub sky_text: Color,
    pub sky: Color,
    pub ground_text: Color,
    pub ground_background: Color,
    pub water_color: Color,
    pub creature_color: Color,
    pub creature_busy_color: Color,
    pub vegetation_color: Color,
    pub weather_color: Color,
    pub ground: char,
    pub water_idle: char,
    pub water_download: char,
    pub water_upload: char,
    pub water_bidirectional: char,
    pub weather_read: char,
    pub weather_write: char,
    pub weather_mixed: char,
    pub vegetation_low: char,
    pub vegetation_mid: char,
    pub vegetation_high: char,
    pub creature_idle: char,
    pub creature_active: char,
    pub creature_busy: char,
}

impl Default for VisualTheme {
    fn default() -> Self {
        Self {
            sky_text: Color::rgb(126, 164, 190),
            sky: SKY,
            ground_text: Color::rgb(103, 160, 94),
            ground_background: GROUND,
            water_color: WATER,
            creature_color: CREATURE,
            creature_busy_color: BUSY_CREATURE,
            vegetation_color: VEGETATION,
            weather_color: Color::rgb(150, 190, 220),
            ground: '▄',
            water_idle: '≈',
            water_download: '›',
            water_upload: '‹',
            water_bidirectional: '≋',
            weather_read: '∙',
            weather_write: '✦',
            weather_mixed: '✶',
            vegetation_low: '╷',
            vegetation_mid: '♧',
            vegetation_high: '♣',
            creature_idle: '◦',
            creature_active: '●',
            creature_busy: '◆',
        }
    }
}

pub fn build_static_landscape_frame(
    width: u16,
    height: u16,
) -> Result<Framebuffer, FramebufferError> {
    build_landscape_frame(width, height, 0)
}

pub fn build_landscape_frame(
    width: u16,
    height: u16,
    tick: u64,
) -> Result<Framebuffer, FramebufferError> {
    build_landscape_frame_with_activity(width, height, tick, &SceneActivity::default())
}

pub fn build_landscape_frame_with_activity(
    width: u16,
    height: u16,
    tick: u64,
    activity: &SceneActivity,
) -> Result<Framebuffer, FramebufferError> {
    build_landscape_frame_with_theme(width, height, tick, activity, &VisualTheme::default())
}

pub fn build_landscape_frame_with_theme(
    width: u16,
    height: u16,
    tick: u64,
    activity: &SceneActivity,
    theme: &VisualTheme,
) -> Result<Framebuffer, FramebufferError> {
    let mut frame = Framebuffer::new(width, height, Cell::new(' ', theme.sky_text, theme.sky))?;

    if height < 3 {
        return Ok(frame);
    }

    let ground_y = height - 1;
    let water_y = height - 2;
    let creature_origin_y = height / 2;

    for x in 0..width {
        frame.set(
            x,
            ground_y,
            Cell::new(theme.ground, theme.ground_text, theme.ground_background),
        )?;

        let water_glyph = water_glyph(x, tick, activity, theme);
        frame.set(
            x,
            water_y,
            Cell::new(water_glyph, theme.water_color, theme.sky),
        )?;
    }

    draw_weather(&mut frame, activity, theme)?;
    draw_vegetation(&mut frame, activity, theme)?;
    draw_creatures(&mut frame, creature_origin_y, tick, activity, theme)?;

    Ok(frame)
}

fn draw_weather(
    frame: &mut Framebuffer,
    activity: &SceneActivity,
    theme: &VisualTheme,
) -> Result<(), FramebufferError> {
    if frame.height() < 5 || frame.width() == 0 {
        return Ok(());
    }

    let intensity = activity.disk_read().max(activity.disk_write());
    if intensity < 0.35 {
        return Ok(());
    }

    // Disk activity becomes sparse sky weather. Keeping it to one bounded row
    // makes reads/writes visible without turning heavy I/O into a full-screen
    // redraw source.
    let max_particles = usize::from(frame.width()).div_ceil(4).max(1);
    let particle_count = (max_particles as f32 * intensity).round() as usize;
    let weather_y = (frame.height() / 4).max(1);
    let glyph = weather_glyph(activity, theme);

    for index in 0..particle_count {
        let base_x = ((index + 1) * usize::from(frame.width())) / (particle_count + 1);
        let x = base_x.min(usize::from(frame.width().saturating_sub(1))) as u16;
        frame.set(
            x,
            weather_y,
            Cell::new(glyph, theme.weather_color, theme.sky),
        )?;
    }

    Ok(())
}

fn draw_vegetation(
    frame: &mut Framebuffer,
    activity: &SceneActivity,
    theme: &VisualTheme,
) -> Result<(), FramebufferError> {
    if frame.height() < 4 || frame.width() == 0 {
        return Ok(());
    }

    // Memory pressure is mapped to sparse density instead of per-cell noise so
    // it remains readable and cheap to diff on every terminal frame.
    let max_plants = usize::from(frame.width()).div_ceil(4).max(1);
    let plant_count = (max_plants as f32 * activity.memory_pressure()).round() as usize;
    let vegetation_y = frame.height() - 3;
    let glyph = vegetation_glyph(activity.memory_pressure(), theme);

    for index in 0..plant_count {
        let x = ((index + 1) * usize::from(frame.width())) / (plant_count + 1);
        frame.set(
            x.min(usize::from(frame.width().saturating_sub(1))) as u16,
            vegetation_y,
            Cell::new(glyph, theme.vegetation_color, theme.sky),
        )?;
    }

    Ok(())
}

fn draw_creatures(
    frame: &mut Framebuffer,
    origin_y: u16,
    tick: u64,
    activity: &SceneActivity,
    theme: &VisualTheme,
) -> Result<(), FramebufferError> {
    let width = frame.width();
    let loads = activity.core_loads();
    let creature_count = if loads.is_empty() {
        1
    } else {
        loads.len().min(usize::from(width.saturating_sub(1)).max(1))
    };
    let lane_count = creature_lane_count(creature_count, frame.height());
    let creatures_per_lane = creature_count.div_ceil(lane_count);
    let lane_start_y = origin_y.saturating_sub((lane_count / 2) as u16);

    for index in 0..creature_count {
        let load = loads.get(index).copied().unwrap_or(0.0);
        let lane = index / creatures_per_lane;
        let lane_slot = index % creatures_per_lane;
        let lane_size = creature_count
            .saturating_sub(lane * creatures_per_lane)
            .min(creatures_per_lane);
        let x = creature_x(lane_slot, lane_size, width, load, tick);
        let y = (lane_start_y + lane as u16).min(frame.height().saturating_sub(3));
        frame.set(x, y, creature_cell(load, tick, theme))?;
    }

    Ok(())
}

fn water_glyph(x: u16, tick: u64, activity: &SceneActivity, theme: &VisualTheme) -> char {
    let download = activity.network_download();
    let upload = activity.network_upload();
    let ambient_phase = u64::from(x) + tick;
    let activity_phase = u64::from(x) + (tick / 2);

    if download >= 0.35 && upload >= 0.35 {
        if activity_phase.is_multiple_of(3) {
            theme.water_idle
        } else {
            theme.water_bidirectional
        }
    } else if download >= 0.35 && download >= upload {
        if activity_phase.is_multiple_of(5) {
            theme.water_idle
        } else {
            theme.water_download
        }
    } else if upload >= 0.35 {
        if activity_phase.is_multiple_of(5) {
            theme.water_idle
        } else {
            theme.water_upload
        }
    } else if ambient_phase.is_multiple_of(4) {
        theme.water_download
    } else {
        theme.water_idle
    }
}

fn weather_glyph(activity: &SceneActivity, theme: &VisualTheme) -> char {
    let read = activity.disk_read();
    let write = activity.disk_write();

    if read >= 0.35 && write >= 0.35 {
        theme.weather_mixed
    } else if write >= read {
        theme.weather_write
    } else {
        theme.weather_read
    }
}

fn vegetation_glyph(memory_pressure: f32, theme: &VisualTheme) -> char {
    if memory_pressure >= 0.80 {
        theme.vegetation_high
    } else if memory_pressure >= 0.50 {
        theme.vegetation_mid
    } else {
        theme.vegetation_low
    }
}

fn creature_lane_count(creature_count: usize, height: u16) -> usize {
    let max_lanes = if height >= 12 {
        3
    } else if height >= 8 {
        2
    } else {
        1
    };

    if creature_count > 12 {
        max_lanes
    } else if creature_count > 4 {
        max_lanes.min(2)
    } else {
        1
    }
}

fn creature_x(index: usize, creature_count: usize, width: u16, load: f32, tick: u64) -> u16 {
    if creature_count == 1 {
        return drifted_x(width / 2, width, load, tick);
    }

    let width = usize::from(width);
    let x = ((index + 1) * width) / (creature_count + 1);
    drifted_x(
        x.min(width.saturating_sub(1)) as u16,
        width as u16,
        load,
        tick,
    )
}

fn drifted_x(base_x: u16, width: u16, load: f32, tick: u64) -> u16 {
    if load < 0.35 {
        return base_x;
    }

    // Movement is deliberately capped to one cell. It gives active cores life
    // while keeping ANSI diffs small and preventing layout jitter.
    let offset = match tick % 4 {
        1 => 1_i16,
        3 => -1_i16,
        _ => 0_i16,
    };
    let max_x = width.saturating_sub(1) as i16;
    (base_x as i16 + offset).clamp(0, max_x) as u16
}

fn creature_cell(load: f32, tick: u64, theme: &VisualTheme) -> Cell {
    // CPU load is intentionally reduced to three visual states. This keeps the
    // ambient scene readable and avoids noisy one-frame glyph changes.
    let glyph = if load >= 0.75 {
        theme.creature_busy
    } else if load >= 0.35 {
        theme.creature_active
    } else if tick.is_multiple_of(2) {
        theme.creature_idle
    } else {
        theme.creature_active
    };

    let color = if load >= 0.75 {
        theme.creature_busy_color
    } else {
        theme.creature_color
    };
    Cell::new(glyph, color, theme.sky)
}
