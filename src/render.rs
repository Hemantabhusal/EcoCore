use crate::framebuffer::{Cell, Color, Framebuffer, FramebufferError};

const SKY: Color = Color::rgb(8, 18, 34);
const GROUND: Color = Color::rgb(35, 50, 35);
const WATER: Color = Color::rgb(35, 120, 210);
const CREATURE: Color = Color::rgb(255, 180, 80);
const BUSY_CREATURE: Color = Color::rgb(255, 95, 90);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneActivity {
    core_loads: Vec<f32>,
}

impl SceneActivity {
    pub fn from_core_loads(core_loads: Vec<f32>) -> Self {
        let core_loads = core_loads
            .into_iter()
            .map(|load| {
                if load.is_finite() {
                    load.clamp(0.0, 1.0)
                } else {
                    0.0
                }
            })
            .collect();

        Self { core_loads }
    }

    pub fn core_loads(&self) -> &[f32] {
        &self.core_loads
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

        let water_glyph = if (u64::from(x) + tick).is_multiple_of(4) {
            '>'
        } else {
            '~'
        };
        frame.set(x, water_y, Cell::new(water_glyph, WATER, SKY))?;
    }

    draw_creatures(&mut frame, creature_origin_y, tick, activity)?;

    Ok(frame)
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
