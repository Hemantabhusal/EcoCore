use std::{cell::RefCell, rc::Rc};

use crate::{canvas::Canvas, simulation::SceneActivity};

use super::{
    lifeforms::{LifeformField, LifeformSeedLayer, LifeformTrailLayer},
    math::{
        add_cyan_glow_point, add_glow_point, add_polyp_tip_glow, add_reef_polyp_pixel, average,
        wave01, wrap,
    },
    scene::{SceneFrame, SceneLayer, TidepoolCanvasConfig},
};

struct DriftMotesLayer;

impl SceneLayer for DriftMotesLayer {
    fn name(&self) -> &'static str {
        "drift_motes"
    }

    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>) {
        draw_drift_motes(frame.tick(), frame.activity(), canvas);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ReefPolypsLayer;

impl SceneLayer for ReefPolypsLayer {
    fn name(&self) -> &'static str {
        "reef_polyps"
    }

    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>) {
        draw_reef_polyps(frame.tick(), frame.activity(), canvas);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SedimentSparksLayer;

impl SceneLayer for SedimentSparksLayer {
    fn name(&self) -> &'static str {
        "sediment_sparks"
    }

    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>) {
        draw_sediment_sparks(frame.tick(), frame.activity(), canvas);
    }
}

pub(super) fn tidepool_sparse_layers(config: TidepoolCanvasConfig) -> Vec<Box<dyn SceneLayer>> {
    // Trails and seeds are separate visual layers, but they must share motion
    // state so the afterglow follows the actual lifeform positions.
    let lifeforms = Rc::new(RefCell::new(LifeformField::new(14, config)));
    vec![
        Box::new(DriftMotesLayer),
        Box::new(ReefPolypsLayer),
        Box::new(LifeformTrailLayer::new(lifeforms.clone())),
        Box::new(LifeformSeedLayer::new(lifeforms)),
        Box::new(SedimentSparksLayer),
    ]
}

fn draw_drift_motes(tick: u64, activity: &SceneActivity, canvas: &mut Canvas) {
    let width = canvas.width().max(1);
    let height = canvas.height().max(1);
    let area = u32::from(width) * u32::from(height);
    let mote_count = (area / 1_200).clamp(10, 40);
    let download = activity.network_download();
    let upload = activity.network_upload();
    let flow = download.max(upload);
    let direction = if upload > download { -1.0 } else { 1.0 };
    let travel = tick as f32 * (0.09 + flow * 0.24) * direction;
    let water_top = f32::from(height) * 0.12;
    let water_span = f32::from(height) * 0.58;

    // Motes are intentionally sparse entity-like marks, not a fourth full-frame
    // scan, so the layer adds motion without changing the terminal bandwidth.
    for index in 0..mote_count {
        let seed = u64::from(index)
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(0xA511_E9B3);
        let phase = seed as f32 * 0.000_031 + tick as f32 * 0.035;
        let base_x = (seed.wrapping_mul(47) % u64::from(width)) as f32;
        let base_y = water_top
            + (seed.wrapping_mul(29) % u64::from(height)) as f32 / f32::from(height) * water_span;
        let lane = (index % 5) as f32;
        let x = wrap(base_x + travel * (0.45 + lane * 0.09), f32::from(width));
        let y = (base_y + phase.sin() * (1.2 + flow * 2.4)).clamp(0.0, f32::from(height - 1));
        let pulse = wave01(phase * 1.7 + lane);
        let energy = 0.18 + pulse * 0.24 + flow * 0.22;

        add_cyan_glow_point(canvas, x.round() as u16, y.round() as u16, 1, energy);
    }
}

fn draw_reef_polyps(tick: u64, activity: &SceneActivity, canvas: &mut Canvas) {
    let width = canvas.width().max(1);
    let height = canvas.height().max(1);
    let area = u32::from(width) * u32::from(height);
    let polyp_count = (area / 1_700).clamp(10, 32);
    let memory_energy = activity.memory_pressure();
    let cpu_energy = average(activity.core_loads());
    let lower_band = (u64::from(height) / 4).max(1);

    for index in 0..polyp_count {
        let seed = u64::from(index)
            .wrapping_mul(0x85EB_CA6B)
            .wrapping_add(0xC2B2_AE35);
        let base_x = (seed.wrapping_mul(41) % u64::from(width)) as f32;
        let base_y = f32::from(height - 1) - (seed.wrapping_mul(17) % lower_band) as f32;
        let height_seed = (seed.wrapping_mul(13) % 7) as f32;
        let stalk_height = 4.0 + height_seed * 0.7 + memory_energy * 4.2;
        let sway_phase = tick as f32 * (0.035 + cpu_energy * 0.025) + seed as f32 * 0.000_021;
        let segments = stalk_height.round() as i32;

        for segment in 0..=segments {
            let progress = segment as f32 / stalk_height.max(1.0);
            let sway = (sway_phase + progress * 1.9).sin() * progress * (0.6 + cpu_energy * 1.4);
            let x = base_x + sway;
            let y = base_y - segment as f32;
            let energy = (0.22 + memory_energy * 0.38) * (1.0 - progress * 0.35);

            add_reef_polyp_pixel(canvas, x.round() as i32, y.round() as i32, energy);
        }

        let tip_progress = segments as f32 / stalk_height.max(1.0);
        let tip_sway =
            (sway_phase + tip_progress * 1.9).sin() * tip_progress * (0.6 + cpu_energy * 1.4);
        let tip_x = (base_x + tip_sway).round() as i32;
        let tip_y = (base_y - segments as f32).round() as i32;
        if tip_x >= 0 && tip_y >= 0 {
            let pulse = wave01(sway_phase * 1.8 + height_seed);
            let energy = 0.26 + memory_energy * 0.34 + cpu_energy * 0.18 + pulse * 0.16;
            add_polyp_tip_glow(canvas, tip_x as u16, tip_y as u16, energy);
        }
    }
}

fn draw_sediment_sparks(tick: u64, activity: &SceneActivity, canvas: &mut Canvas) {
    let disk_energy = activity.disk_read().max(activity.disk_write());
    if disk_energy <= 0.0 {
        return;
    }

    let width = canvas.width().max(1);
    let height = canvas.height().max(1);
    let spark_count = (4.0 + disk_energy * 18.0).round() as u16;

    for index in 0..spark_count {
        let seed = tick
            .wrapping_mul(37)
            .wrapping_add(u64::from(index) * 91)
            .wrapping_add(17);
        let x = (seed.wrapping_mul(53) % u64::from(width)) as u16;
        let lower_band = u64::from((height / 3).max(1));
        let y = (u64::from(height) - 1 - (seed.wrapping_mul(29) % lower_band)) as u16;
        let intensity = disk_energy * (0.55 + f32::from(index % 5) * 0.1);
        add_glow_point(canvas, x, y, 2, intensity);
    }
}
