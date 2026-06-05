use std::{cell::RefCell, rc::Rc};

use crate::{
    canvas::{Canvas, Rgba},
    simulation::SceneActivity,
};

use super::{
    math::{
        add_channel, average, blend, directional_body_influence, head_influence,
        normalize_direction, tail_influence, wave01, wrap,
    },
    scene::{SceneFrame, SceneLayer, TidepoolCanvasConfig},
};

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
pub(super) struct LifeformTrailLayer {
    field: Rc<RefCell<LifeformField>>,
}

impl LifeformTrailLayer {
    pub(super) fn new(field: Rc<RefCell<LifeformField>>) -> Self {
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
pub(super) struct LifeformSeedLayer {
    field: Rc<RefCell<LifeformField>>,
}

impl LifeformSeedLayer {
    pub(super) fn new(field: Rc<RefCell<LifeformField>>) -> Self {
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
