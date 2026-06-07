use crate::{
    assets::{Sprite, SpriteBlit, SpriteError},
    canvas::Canvas,
    simulation::SceneActivity,
};

use super::background::cafe_counter_top;

const CAT_ASLEEP_SHEET: &[u8] =
    include_bytes!("../../../assets/cat_player/Cat_sheets/Cat_asleep_1.png");
const CAT_IDLE_SHEET: &[u8] =
    include_bytes!("../../../assets/cat_player/Cat_sheets/Cat_idle_1.png");
const CAT_WALK_SHEET: &[u8] =
    include_bytes!("../../../assets/cat_player/Cat_sheets/Cat_walk_1.png");
const CAT_FRAME_SIZE: u16 = 32;

pub(super) struct CatAnimator {
    asleep_frames: Vec<Sprite>,
    idle_frames: Vec<Sprite>,
    walk_frames: Vec<Sprite>,
    presence: CatPresence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CatPresence {
    Asleep,
    Idle,
    Walking,
}

impl CatAnimator {
    pub(super) fn new() -> Result<Self, SpriteError> {
        Ok(Self {
            asleep_frames: load_cat_frames(CAT_ASLEEP_SHEET)?,
            idle_frames: load_cat_frames(CAT_IDLE_SHEET)?,
            walk_frames: load_cat_frames(CAT_WALK_SHEET)?,
            presence: CatPresence::Idle,
        })
    }

    pub(super) fn render(&mut self, canvas: &mut Canvas, tick: u64, activity: &SceneActivity) {
        self.update_presence(activity.average_core_load());
        let cat = {
            let (frames, cadence) = self.frames_for_presence();
            let frame_index = ((tick / cadence) as usize) % frames.len();
            frames[frame_index].clone()
        };
        let scale = readable_cat_scale(canvas.width(), canvas.height());
        let cat_width = cat.width() * scale;
        let cat_height = cat.height() * scale;
        let counter_top = cafe_counter_top(canvas.height());
        let energy = activity.average_core_load();
        let bob = if energy > 0.55 && tick.is_multiple_of(12) {
            2
        } else {
            0
        };
        let x = offset_u16(
            canvas.width().saturating_sub(cat_width) / 2,
            walking_pace_offset(self.presence, tick),
        );
        let y = counter_top
            .saturating_sub(cat_height)
            .saturating_add(8)
            .saturating_sub(bob);

        cat.blit_scaled(canvas, SpriteBlit { x, y, scale })
            .expect("cat anchor is chosen to fit inside cafe canvas");
    }

    fn update_presence(&mut self, average_core_load: f32) {
        self.presence = match self.presence {
            CatPresence::Walking if average_core_load >= 0.45 => CatPresence::Walking,
            CatPresence::Walking if average_core_load <= 0.12 => CatPresence::Asleep,
            CatPresence::Walking => CatPresence::Idle,
            CatPresence::Asleep if average_core_load <= 0.25 => CatPresence::Asleep,
            CatPresence::Asleep if average_core_load >= 0.72 => CatPresence::Walking,
            CatPresence::Asleep => CatPresence::Idle,
            CatPresence::Idle if average_core_load >= 0.72 => CatPresence::Walking,
            CatPresence::Idle if average_core_load <= 0.08 => CatPresence::Asleep,
            CatPresence::Idle => CatPresence::Idle,
        };
    }

    fn frames_for_presence(&self) -> (&[Sprite], u64) {
        match self.presence {
            CatPresence::Asleep => (&self.asleep_frames, 18),
            CatPresence::Idle => (&self.idle_frames, 10),
            CatPresence::Walking => (&self.walk_frames, 6),
        }
    }
}

fn walking_pace_offset(presence: CatPresence, tick: u64) -> i16 {
    if presence != CatPresence::Walking {
        return 0;
    }

    let phase = ((tick / 6) % 12) as i16;
    let triangle = if phase <= 6 { phase } else { 12 - phase };
    (triangle - 3) * 4
}

fn offset_u16(value: u16, offset: i16) -> u16 {
    if offset.is_negative() {
        value.saturating_sub(offset.unsigned_abs())
    } else {
        value.saturating_add(offset as u16)
    }
}

fn readable_cat_scale(width: u16, height: u16) -> u16 {
    if width >= 640 && height >= 300 {
        4
    } else if width >= 540 && height >= 252 {
        3
    } else {
        2
    }
}

fn load_cat_frames(sheet_bytes: &[u8]) -> Result<Vec<Sprite>, SpriteError> {
    let sheet = Sprite::from_png_bytes(sheet_bytes)?;
    let frames = sheet.width() / CAT_FRAME_SIZE;
    let mut sprites = Vec::new();
    for frame in 0..frames.max(1) {
        sprites.push(crop_sprite(
            &sheet,
            frame * CAT_FRAME_SIZE,
            0,
            CAT_FRAME_SIZE.min(sheet.width()),
            CAT_FRAME_SIZE.min(sheet.height()),
        )?);
    }
    Ok(sprites)
}

fn crop_sprite(
    source: &Sprite,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
) -> Result<Sprite, SpriteError> {
    let mut pixels = Vec::with_capacity(usize::from(width) * usize::from(height));
    for source_y in y..y + height {
        for source_x in x..x + width {
            pixels.push(
                source
                    .pixel(source_x, source_y)
                    .expect("cropped sprite region is in bounds"),
            );
        }
    }
    Sprite::from_rgba_pixels(width, height, pixels)
}
