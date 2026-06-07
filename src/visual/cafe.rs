mod background;
mod cat;
mod effects;

use crate::{
    assets::SpriteError,
    canvas::{Canvas, CanvasError, DirtyRegion, Rgba},
    simulation::SceneActivity,
};

use background::draw_cafe_background;
use cat::CatAnimator;
use effects::{render_counter_activity, render_window_rain};

const CAFE_BACKGROUND: &str = "cafe_background";
const MAIN_CAT_SPRITE: &str = "main_cat_sprite";
const SECONDARY_CAT_SPRITE: &str = "secondary_cat_sprite";
const WINDOW_RAIN: &str = "window_rain";
const WARM_LIGHT: &str = "warm_light";
const COUNTER_ACTIVITY: &str = "counter_activity";

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
    cat: CatAnimator,
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
            cat: CatAnimator::new()?,
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

        render_window_rain(&mut self.canvas, tick, activity);
        render_counter_activity(&mut self.canvas, tick, activity);
        self.cat.render(&mut self.canvas, tick, activity);
        self.previous_dynamic_dirty = self.canvas.dirty_regions();

        if self.first_render {
            self.canvas.mark_full_frame_required();
            self.first_render = false;
        }

        &self.canvas
    }

    pub const fn layer_names(&self) -> [&'static str; 6] {
        [
            CAFE_BACKGROUND,
            MAIN_CAT_SPRITE,
            SECONDARY_CAT_SPRITE,
            WINDOW_RAIN,
            WARM_LIGHT,
            COUNTER_ACTIVITY,
        ]
    }
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
