use crate::{
    assets::{Sprite, SpriteBlit, SpriteError},
    canvas::{Canvas, CanvasError, DirtyRegion, Rgba},
    simulation::SceneActivity,
};

const CAT_IDLE_SHEET: &[u8] = include_bytes!("../../assets/cat_player/Cat_sheets/Cat_idle_1.png");
const CAT_FRAME_SIZE: u16 = 32;
const CAFE_BACKGROUND: &str = "cafe_background";
const MAIN_CAT_SPRITE: &str = "main_cat_sprite";
const WINDOW_RAIN: &str = "window_rain";

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
    cat_idle_frames: Vec<Sprite>,
    previous_dynamic_dirty: Vec<DirtyRegion>,
    first_render: bool,
}

impl CafeScene {
    pub fn new(config: CafeCanvasConfig) -> Result<Self, CafeSceneError> {
        let mut background = Canvas::new(config.width, config.height, Rgba::rgb(10, 8, 16))?;
        draw_cafe_background(&mut background);
        background.clear_dirty();

        Ok(Self {
            canvas: Canvas::new(config.width, config.height, Rgba::rgb(0, 0, 0))?,
            background,
            cat_idle_frames: load_idle_cat_frames()?,
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

    pub const fn layer_names(&self) -> [&'static str; 3] {
        [CAFE_BACKGROUND, MAIN_CAT_SPRITE, WINDOW_RAIN]
    }

    fn render_main_cat(&mut self, tick: u64, activity: &SceneActivity) {
        let frame_index = ((tick / 10) as usize) % self.cat_idle_frames.len();
        let cat = &self.cat_idle_frames[frame_index];
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
}

fn readable_cat_scale(width: u16, height: u16) -> u16 {
    if width >= 380 && height >= 180 { 3 } else { 2 }
}

fn load_idle_cat_frames() -> Result<Vec<Sprite>, CafeSceneError> {
    let sheet = Sprite::from_png_bytes(CAT_IDLE_SHEET)?;
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
    draw_rect(
        canvas,
        0,
        cafe_counter_top(canvas.height()),
        canvas.width(),
        canvas.height() - cafe_counter_top(canvas.height()),
        Rgba::rgb(73, 38, 29),
    );
    draw_rect(
        canvas,
        0,
        cafe_counter_top(canvas.height()),
        canvas.width(),
        8,
        Rgba::rgb(151, 87, 47),
    );
    draw_window(canvas);
    draw_shelves(canvas);
    draw_counter_props(canvas);
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
        Rgba::rgb(13, 33, 65),
        Rgba::rgb(18, 70, 104),
    );
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

fn draw_shelves(canvas: &mut Canvas) {
    let y = canvas.height() / 4;
    let width = canvas.width() / 3;
    draw_rect(canvas, 24, y, width, 5, Rgba::rgb(121, 68, 41));
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
    }
}

fn draw_counter_props(canvas: &mut Canvas) {
    let y = cafe_counter_top(canvas.height()).saturating_add(14);
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
