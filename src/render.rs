use crate::framebuffer::{Cell, Color, Framebuffer, FramebufferError};

const SKY: Color = Color::rgb(8, 18, 34);
const GROUND: Color = Color::rgb(35, 50, 35);
const WATER: Color = Color::rgb(35, 120, 210);
const CREATURE: Color = Color::rgb(255, 180, 80);
const BUSY_CREATURE: Color = Color::rgb(255, 95, 90);
const VEGETATION: Color = Color::rgb(95, 190, 105);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneActivity {
    core_loads: Vec<f32>,
    memory_pressure: f32,
    network_download: f32,
    network_upload: f32,
    disk_read: f32,
    disk_write: f32,
}

impl SceneActivity {
    pub fn from_core_loads(core_loads: Vec<f32>) -> Self {
        Self::default().with_core_loads(core_loads)
    }

    pub fn with_core_loads(mut self, core_loads: Vec<f32>) -> Self {
        let core_loads = core_loads
            .into_iter()
            .map(normalize_unit_interval)
            .collect();

        self.core_loads = core_loads;
        self
    }

    pub fn with_memory_pressure(mut self, memory_pressure: f32) -> Self {
        self.memory_pressure = normalize_unit_interval(memory_pressure);
        self
    }

    pub fn with_network_flow(mut self, download: f32, upload: f32) -> Self {
        self.network_download = normalize_unit_interval(download);
        self.network_upload = normalize_unit_interval(upload);
        self
    }

    pub fn with_disk_activity(mut self, read: f32, write: f32) -> Self {
        self.disk_read = normalize_unit_interval(read);
        self.disk_write = normalize_unit_interval(write);
        self
    }

    pub fn core_loads(&self) -> &[f32] {
        &self.core_loads
    }

    pub fn memory_pressure(&self) -> f32 {
        self.memory_pressure
    }

    pub fn network_download(&self) -> f32 {
        self.network_download
    }

    pub fn network_upload(&self) -> f32 {
        self.network_upload
    }

    pub fn disk_read(&self) -> f32 {
        self.disk_read
    }

    pub fn disk_write(&self) -> f32 {
        self.disk_write
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
    let mut frame = Framebuffer::new(
        width,
        height,
        Cell::new(' ', Color::rgb(120, 150, 190), SKY),
    )?;

    if height < 3 {
        return Ok(frame);
    }

    let ground_y = height - 1;
    let water_y = height - 2;
    let creature_origin_y = height / 2;

    for x in 0..width {
        frame.set(x, ground_y, Cell::new('.', Color::rgb(90, 150, 85), GROUND))?;

        let water_glyph = water_glyph(x, tick, activity);
        frame.set(x, water_y, Cell::new(water_glyph, WATER, SKY))?;
    }

    draw_weather(&mut frame, tick, activity)?;
    draw_vegetation(&mut frame, activity)?;
    draw_creatures(&mut frame, creature_origin_y, tick, activity)?;

    Ok(frame)
}

fn draw_weather(
    frame: &mut Framebuffer,
    tick: u64,
    activity: &SceneActivity,
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
    let glyph = weather_glyph(activity);

    for index in 0..particle_count {
        let base_x = ((index + 1) * usize::from(frame.width())) / (particle_count + 1);
        let drift = if tick.is_multiple_of(2) { 0 } else { index % 2 };
        let x = (base_x + drift).min(usize::from(frame.width().saturating_sub(1))) as u16;
        frame.set(x, weather_y, Cell::new(glyph, WATER, SKY))?;
    }

    Ok(())
}

fn draw_vegetation(
    frame: &mut Framebuffer,
    activity: &SceneActivity,
) -> Result<(), FramebufferError> {
    if frame.height() < 4 || frame.width() == 0 {
        return Ok(());
    }

    // Memory pressure is mapped to sparse density instead of per-cell noise so
    // it remains readable and cheap to diff on every terminal frame.
    let max_plants = usize::from(frame.width()).div_ceil(4).max(1);
    let plant_count = (max_plants as f32 * activity.memory_pressure()).round() as usize;
    let vegetation_y = frame.height() - 3;

    for index in 0..plant_count {
        let x = ((index + 1) * usize::from(frame.width())) / (plant_count + 1);
        frame.set(
            x.min(usize::from(frame.width().saturating_sub(1))) as u16,
            vegetation_y,
            Cell::new('^', VEGETATION, SKY),
        )?;
    }

    Ok(())
}

fn draw_creatures(
    frame: &mut Framebuffer,
    origin_y: u16,
    tick: u64,
    activity: &SceneActivity,
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
        frame.set(x, y, creature_cell(load, tick))?;
    }

    Ok(())
}

fn water_glyph(x: u16, tick: u64, activity: &SceneActivity) -> char {
    let download = activity.network_download();
    let upload = activity.network_upload();
    let wave_phase = u64::from(x) + tick;

    if download >= 0.35 && upload >= 0.35 {
        if wave_phase.is_multiple_of(3) {
            '~'
        } else {
            '='
        }
    } else if download >= 0.35 && download >= upload {
        if wave_phase.is_multiple_of(5) {
            '~'
        } else {
            '>'
        }
    } else if upload >= 0.35 {
        if wave_phase.is_multiple_of(5) {
            '~'
        } else {
            '<'
        }
    } else if wave_phase.is_multiple_of(4) {
        '>'
    } else {
        '~'
    }
}

fn weather_glyph(activity: &SceneActivity) -> char {
    let read = activity.disk_read();
    let write = activity.disk_write();

    if read >= 0.35 && write >= 0.35 {
        '#'
    } else if write >= read {
        '*'
    } else {
        ','
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

fn creature_cell(load: f32, tick: u64) -> Cell {
    // CPU load is intentionally reduced to three visual states. This keeps the
    // ambient scene readable and avoids noisy one-frame glyph changes.
    let glyph = if load >= 0.75 {
        '@'
    } else if load >= 0.35 {
        'O'
    } else if tick.is_multiple_of(2) {
        'o'
    } else {
        'O'
    };

    let color = if load >= 0.75 {
        BUSY_CREATURE
    } else {
        CREATURE
    };
    Cell::new(glyph, color, SKY)
}

fn normalize_unit_interval(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}
