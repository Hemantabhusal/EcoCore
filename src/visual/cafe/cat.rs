use crate::{
    assets::{Sprite, SpriteError},
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
const CAT_ALT: &[u8] = include_bytes!("../../../assets/cat_player/Cat_sheets/CAT _alt.png");
const CAT_FRAME_SIZE: u16 = 32;

pub(super) struct CatAnimator {
    asleep_frames: Vec<Sprite>,
    idle_frames: Vec<Sprite>,
    walk_frames: Vec<Sprite>,
    secondary_cat: Sprite,
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
            secondary_cat: Sprite::from_png_bytes(CAT_ALT)?,
            presence: CatPresence::Idle,
        })
    }

    pub(super) fn render(&mut self, canvas: &mut Canvas, tick: u64, activity: &SceneActivity) {
        self.update_presence(activity.average_core_load());
        self.render_secondary_cat(canvas, tick);

        let cat = {
            let (frames, cadence) = self.frames_for_presence();
            let frame_index = ((tick / cadence) as usize) % frames.len();
            frames[frame_index].clone()
        };
        let cat_size = main_cat_size(canvas.width(), canvas.height());
        let counter_top = cafe_counter_top(canvas.height());
        let energy = activity.average_core_load();
        let bob = if energy > 0.55 && tick.is_multiple_of(12) {
            2
        } else {
            0
        };
        let breath = main_cat_breath_offset(self.presence, tick);
        let x = offset_u16(
            canvas.width().saturating_sub(cat_size) / 2,
            walking_pace_offset(self.presence, tick),
        );
        let y = counter_top
            .saturating_sub(cat_size)
            .saturating_add(8)
            .saturating_add(breath)
            .saturating_sub(bob);

        blit_resized(&cat, canvas, x, y, cat_size, cat_size)
            .expect("cat anchor is chosen to fit inside cafe canvas");
    }

    fn render_secondary_cat(&self, canvas: &mut Canvas, tick: u64) {
        let size = main_cat_size(canvas.width(), canvas.height());
        let x = canvas.width() / 4;
        let y = cafe_counter_top(canvas.height())
            .saturating_sub(size)
            .saturating_add(9 + ((tick / 75) % 2) as u16);

        blit_resized(&self.secondary_cat, canvas, x, y, size, size)
            .expect("secondary cat anchor is chosen to fit inside cafe canvas");
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

fn main_cat_breath_offset(presence: CatPresence, tick: u64) -> u16 {
    match presence {
        CatPresence::Idle => ((tick / 30) % 2) as u16,
        CatPresence::Asleep => ((tick / 45) % 2) as u16,
        CatPresence::Walking => 0,
    }
}

fn walking_pace_offset(presence: CatPresence, tick: u64) -> i16 {
    if presence != CatPresence::Walking {
        return 0;
    }

    let phase = ((tick / 6) % 24) as i16;
    let triangle = if phase <= 12 { phase } else { 24 - phase };
    (triangle - 6) * 5
}

fn offset_u16(value: u16, offset: i16) -> u16 {
    if offset.is_negative() {
        value.saturating_sub(offset.unsigned_abs())
    } else {
        value.saturating_add(offset as u16)
    }
}

fn main_cat_size(width: u16, height: u16) -> u16 {
    if width >= 640 && height >= 300 {
        66
    } else if width >= 540 && height >= 252 {
        50
    } else {
        42
    }
}

fn blit_resized(
    sprite: &Sprite,
    canvas: &mut Canvas,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
) -> Result<(), SpriteError> {
    if width == 0 || height == 0 {
        return Err(SpriteError::InvalidSize { width, height });
    }
    if x >= canvas.width()
        || y >= canvas.height()
        || x.saturating_add(width) > canvas.width()
        || y.saturating_add(height) > canvas.height()
    {
        return Err(SpriteError::OutOfCanvas {
            x,
            y,
            width,
            height,
        });
    }

    for dy in 0..height {
        let source_y = dy * sprite.height() / height;
        for dx in 0..width {
            let source_x = dx * sprite.width() / width;
            let source = sprite
                .pixel(source_x, source_y)
                .expect("resized source coordinate is in bounds");
            if source.a == 0 {
                continue;
            }
            let target_x = x + dx;
            let target_y = y + dy;
            let background = canvas
                .pixel(target_x, target_y)
                .expect("validated resize target is in bounds");
            canvas
                .set_pixel(target_x, target_y, source.blend_over(background))
                .map_err(SpriteError::Canvas)?;
        }
    }

    Ok(())
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
