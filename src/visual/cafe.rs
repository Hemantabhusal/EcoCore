use crate::{
    assets::{Sprite, SpriteBlit, SpriteError},
    canvas::{Canvas, CanvasError, DirtyRegion, Rgba},
    simulation::SceneActivity,
};

const CAT_ASLEEP_SHEET: &[u8] =
    include_bytes!("../../assets/cat_player/Cat_sheets/Cat_asleep_1.png");
const CAT_IDLE_SHEET: &[u8] = include_bytes!("../../assets/cat_player/Cat_sheets/Cat_idle_1.png");
const CAT_WALK_SHEET: &[u8] = include_bytes!("../../assets/cat_player/Cat_sheets/Cat_walk_1.png");
const CAT_FRAME_SIZE: u16 = 32;
const CAFE_BACKGROUND: &str = "cafe_background";
const MAIN_CAT_SPRITE: &str = "main_cat_sprite";
const WINDOW_RAIN: &str = "window_rain";
const WARM_LIGHT: &str = "warm_light";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CafeCanvasConfig {
    pub width: u16,
    pub height: u16,
}

impl CafeCanvasConfig {
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

pub struct CafeScene {
    canvas: Canvas,
    background: Canvas,
    cat_asleep_frames: Vec<Sprite>,
    cat_idle_frames: Vec<Sprite>,
    cat_walk_frames: Vec<Sprite>,
    cat_presence: CatPresence,
    previous_dynamic_dirty: Vec<DirtyRegion>,
    first_render: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CatPresence {
    Asleep,
    Idle,
    Walking,
}

impl CafeScene {
    pub fn new(config: CafeCanvasConfig) -> Result<Self, CafeSceneError> {
        let mut background = Canvas::new(config.width, config.height, Rgba::rgb(10, 8, 16))?;
        draw_cafe_background(&mut background);
        background.clear_dirty();

        Ok(Self {
            canvas: Canvas::new(config.width, config.height, Rgba::rgb(0, 0, 0))?,
            background,
            cat_asleep_frames: load_cat_frames(CAT_ASLEEP_SHEET)?,
            cat_idle_frames: load_cat_frames(CAT_IDLE_SHEET)?,
            cat_walk_frames: load_cat_frames(CAT_WALK_SHEET)?,
            cat_presence: CatPresence::Idle,
            previous_dynamic_dirty: Vec::new(),
            first_render: true,
        })
    }

    pub fn render(&mut self, tick: u64, activity: &SceneActivity) -> &Canvas {
        self.canvas.clear_dirty();

        if self.first_render {
            self.canvas
                .pixels_mut()
                .copy_from_slice(self.background.pixels());
            self.canvas.clear_dirty();
        } else {
            for region in &self.previous_dynamic_dirty {
                self.canvas
                    .copy_region_from(&self.background, *region)
                    .expect("previous dirty region came from this canvas");
            }
        }

        self.render_main_cat(tick, activity);
        self.previous_dynamic_dirty = self.canvas.dirty_regions();

        if self.first_render {
            self.canvas.mark_full_frame_required();
            self.first_render = false;
        }

        &self.canvas
    }

    pub const fn layer_names(&self) -> [&'static str; 4] {
        [CAFE_BACKGROUND, MAIN_CAT_SPRITE, WINDOW_RAIN, WARM_LIGHT]
    }

    fn render_main_cat(&mut self, tick: u64, activity: &SceneActivity) {
        self.update_cat_presence(activity.average_core_load());
        let cat = {
            let (frames, cadence) = self.cat_frames_for_presence();
            let frame_index = ((tick / cadence) as usize) % frames.len();
            frames[frame_index].clone()
        };
        let scale = readable_cat_scale(self.canvas.width(), self.canvas.height());
        let cat_width = cat.width() * scale;
        let cat_height = cat.height() * scale;
        let counter_top = cafe_counter_top(self.canvas.height());
        let energy = activity.average_core_load();
        let bob = if energy > 0.55 && tick.is_multiple_of(12) {
            2
        } else {
            0
        };
        let x = self.canvas.width().saturating_sub(cat_width) / 2;
        let y = counter_top
            .saturating_sub(cat_height)
            .saturating_add(8)
            .saturating_sub(bob);

        cat.blit_scaled(&mut self.canvas, SpriteBlit { x, y, scale })
            .expect("cat anchor is chosen to fit inside cafe canvas");
    }

    fn update_cat_presence(&mut self, average_core_load: f32) {
        self.cat_presence = match self.cat_presence {
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

    fn cat_frames_for_presence(&self) -> (&[Sprite], u64) {
        match self.cat_presence {
            CatPresence::Asleep => (&self.cat_asleep_frames, 18),
            CatPresence::Idle => (&self.cat_idle_frames, 10),
            CatPresence::Walking => (&self.cat_walk_frames, 6),
        }
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

fn load_cat_frames(sheet_bytes: &[u8]) -> Result<Vec<Sprite>, CafeSceneError> {
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

fn draw_cafe_background(canvas: &mut Canvas) {
    draw_vertical_gradient(
        canvas,
        0,
        0,
        canvas.width(),
        canvas.height(),
        Rgba::rgb(16, 12, 24),
        Rgba::rgb(54, 31, 31),
    );
    draw_warm_light(canvas);
    draw_rect(
        canvas,
        0,
        cafe_counter_top(canvas.height()),
        canvas.width(),
        canvas.height() - cafe_counter_top(canvas.height()),
        Rgba::rgb(85, 43, 30),
    );
    draw_rect(
        canvas,
        0,
        cafe_counter_top(canvas.height()),
        canvas.width(),
        8,
        Rgba::rgb(169, 96, 48),
    );
    draw_rect(
        canvas,
        0,
        cafe_counter_top(canvas.height()) + 8,
        canvas.width(),
        3,
        Rgba::rgb(48, 26, 26),
    );
    draw_window(canvas);
    draw_shelves(canvas);
    draw_counter_props(canvas);
}

fn draw_warm_light(canvas: &mut Canvas) {
    let lamp_x = canvas.width() / 5;
    let lamp_y = canvas.height() / 5;
    draw_rect(
        canvas,
        lamp_x.saturating_sub(2),
        0,
        4,
        lamp_y.saturating_add(5),
        Rgba::rgb(58, 34, 31),
    );
    draw_rect(
        canvas,
        lamp_x.saturating_sub(18),
        lamp_y,
        36,
        11,
        Rgba::rgb(219, 149, 68),
    );
    draw_rect(
        canvas,
        lamp_x.saturating_sub(12),
        lamp_y + 11,
        24,
        7,
        Rgba::rgb(255, 190, 91),
    );

    // The glow is blocky on purpose: it reads at normal terminal distance and
    // remains part of the cached background instead of becoming a per-frame cost.
    draw_rect(
        canvas,
        lamp_x.saturating_sub(52),
        lamp_y + 18,
        104,
        24,
        Rgba::rgb(91, 52, 38),
    );
    draw_rect(
        canvas,
        lamp_x.saturating_sub(36),
        lamp_y + 18,
        72,
        18,
        Rgba::rgb(120, 72, 45),
    );
    draw_rect(
        canvas,
        lamp_x.saturating_sub(20),
        lamp_y + 18,
        40,
        12,
        Rgba::rgb(188, 123, 61),
    );
}

fn draw_window(canvas: &mut Canvas) {
    let window_width = canvas.width() / 3;
    let window_height = canvas.height() / 2;
    let x = canvas.width() - window_width - canvas.width() / 12;
    let y = canvas.height() / 8;

    draw_rect(
        canvas,
        x.saturating_sub(4),
        y.saturating_sub(4),
        window_width + 8,
        window_height + 8,
        Rgba::rgb(93, 55, 40),
    );
    draw_vertical_gradient(
        canvas,
        x,
        y,
        window_width,
        window_height,
        Rgba::rgb(7, 22, 68),
        Rgba::rgb(13, 77, 128),
    );
    draw_city_silhouette(canvas, x, y, window_width, window_height);
    draw_rect(
        canvas,
        x + window_width / 2 - 2,
        y,
        4,
        window_height,
        Rgba::rgb(36, 27, 38),
    );
    draw_rect(
        canvas,
        x,
        y + window_height / 2 - 2,
        window_width,
        4,
        Rgba::rgb(36, 27, 38),
    );

    // Rain is static in 4A so the first sprite milestone does not accidentally
    // spread dirty tiles across the whole scene.
    for index in 0..18 {
        let rain_x = x + 8 + ((index * 17) % window_width.max(1));
        let rain_y = y + 4 + ((index * 23) % window_height.max(1));
        draw_line_down(canvas, rain_x, rain_y, 5, Rgba::rgb(95, 151, 184));
    }
}

fn draw_city_silhouette(canvas: &mut Canvas, x: u16, y: u16, width: u16, height: u16) {
    let base_y = y + height.saturating_sub(26);
    let buildings = [
        (6, 19, 14),
        (23, 28, 18),
        (45, 16, 16),
        (65, 34, 20),
        (91, 23, 17),
        (113, 30, 15),
    ];

    for (offset_x, building_height, building_width) in buildings {
        if offset_x >= width {
            continue;
        }
        let building_x = x + offset_x;
        let building_y = base_y.saturating_sub(building_height);
        draw_rect(
            canvas,
            building_x,
            building_y,
            building_width.min(width - offset_x),
            building_height,
            Rgba::rgb(9, 19, 42),
        );
        for window in 0..3 {
            let light_x = building_x + 3 + window * 5;
            let light_y = building_y + 5 + (window % 2) * 8;
            if light_x + 2 < x + width && light_y + 3 < y + height {
                draw_rect(canvas, light_x, light_y, 2, 3, Rgba::rgb(227, 169, 76));
            }
        }
    }
}

fn draw_shelves(canvas: &mut Canvas) {
    let y = canvas.height() / 4;
    let width = canvas.width() / 3;
    draw_rect(canvas, 24, y, width, 6, Rgba::rgb(132, 73, 42));
    draw_rect(canvas, 24, y + 6, width, 3, Rgba::rgb(70, 38, 31));
    for cup in 0..5 {
        let x = 34 + cup * 22;
        draw_rect(
            canvas,
            x,
            y.saturating_sub(12),
            10,
            12,
            Rgba::rgb(218, 166, 91),
        );
        draw_rect(
            canvas,
            x + 2,
            y.saturating_sub(10),
            6,
            3,
            Rgba::rgb(246, 205, 126),
        );
    }
    draw_rect(
        canvas,
        42,
        y + 34,
        width.saturating_sub(20),
        5,
        Rgba::rgb(112, 61, 40),
    );
    for jar in 0..4 {
        let x = 52 + jar * 25;
        draw_rect(canvas, x, y + 18, 12, 16, Rgba::rgb(83, 61, 70));
        draw_rect(canvas, x + 2, y + 20, 8, 9, Rgba::rgb(197, 135, 80));
    }
}

fn draw_counter_props(canvas: &mut Canvas) {
    let y = cafe_counter_top(canvas.height()).saturating_add(14);
    let stage_width = canvas.width() / 5;
    let stage_x = canvas.width().saturating_sub(stage_width) / 2;
    draw_rect(
        canvas,
        stage_x,
        cafe_counter_top(canvas.height()).saturating_add(12),
        stage_width,
        10,
        Rgba::rgb(46, 28, 28),
    );
    draw_rect(
        canvas,
        stage_x.saturating_add(6),
        cafe_counter_top(canvas.height()).saturating_add(12),
        stage_width.saturating_sub(12),
        2,
        Rgba::rgb(121, 64, 40),
    );
    draw_rect(
        canvas,
        canvas.width() / 5,
        y,
        18,
        10,
        Rgba::rgb(204, 139, 74),
    );
    draw_rect(
        canvas,
        canvas.width() / 5 + 22,
        y.saturating_sub(8),
        8,
        18,
        Rgba::rgb(182, 112, 61),
    );
    draw_rect(
        canvas,
        canvas.width().saturating_sub(90),
        y,
        40,
        14,
        Rgba::rgb(45, 30, 32),
    );
    draw_rect(
        canvas,
        canvas.width().saturating_sub(78),
        y.saturating_sub(14),
        15,
        14,
        Rgba::rgb(108, 70, 52),
    );
    draw_rect(
        canvas,
        canvas.width().saturating_sub(73),
        y.saturating_sub(24),
        5,
        10,
        Rgba::rgb(214, 156, 88),
    );
    for panel in 0..5 {
        let x = 24 + panel * 72;
        draw_rect(canvas, x, y + 22, 48, 22, Rgba::rgb(103, 53, 33));
        draw_rect(canvas, x + 3, y + 25, 42, 3, Rgba::rgb(149, 82, 44));
    }
}

fn cafe_counter_top(height: u16) -> u16 {
    height.saturating_sub(height / 3)
}

fn draw_rect(canvas: &mut Canvas, x: u16, y: u16, width: u16, height: u16, color: Rgba) {
    let max_y = y.saturating_add(height).min(canvas.height());
    let max_x = x.saturating_add(width).min(canvas.width());
    for py in y..max_y {
        for px in x..max_x {
            canvas.set_pixel(px, py, color).expect("rect is clipped");
        }
    }
}

fn draw_vertical_gradient(
    canvas: &mut Canvas,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    top: Rgba,
    bottom: Rgba,
) {
    let max_y = y.saturating_add(height).min(canvas.height());
    let max_x = x.saturating_add(width).min(canvas.width());
    for py in y..max_y {
        let t = (py - y) as f32 / height.max(1) as f32;
        let color = lerp_color(top, bottom, t);
        for px in x..max_x {
            canvas
                .set_pixel(px, py, color)
                .expect("gradient is clipped");
        }
    }
}

fn draw_line_down(canvas: &mut Canvas, x: u16, y: u16, length: u16, color: Rgba) {
    for offset in 0..length {
        let py = y + offset;
        if x < canvas.width() && py < canvas.height() {
            canvas
                .set_pixel(x, py, color)
                .expect("rain pixel in bounds");
        }
    }
}

fn lerp_color(start: Rgba, end: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    Rgba::rgb(
        lerp_channel(start.r, end.r, t),
        lerp_channel(start.g, end.g, t),
        lerp_channel(start.b, end.b, t),
    )
}

fn lerp_channel(start: u8, end: u8, t: f32) -> u8 {
    (f32::from(start) + (f32::from(end) - f32::from(start)) * t).round() as u8
}

#[derive(Debug)]
pub enum CafeSceneError {
    Canvas(CanvasError),
    Sprite(SpriteError),
}

impl From<CanvasError> for CafeSceneError {
    fn from(error: CanvasError) -> Self {
        Self::Canvas(error)
    }
}

impl From<SpriteError> for CafeSceneError {
    fn from(error: SpriteError) -> Self {
        Self::Sprite(error)
    }
}

impl std::fmt::Display for CafeSceneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Canvas(error) => write!(f, "cafe canvas failed: {error}"),
            Self::Sprite(error) => write!(f, "cafe sprite failed: {error}"),
        }
    }
}

impl std::error::Error for CafeSceneError {}
