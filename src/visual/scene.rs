use crate::{
    canvas::{Canvas, CanvasError, DirtyRegion, Rgba},
    simulation::SceneActivity,
};

use super::{environment::EnvironmentLayer, sparse::tidepool_sparse_layers};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TidepoolCanvasConfig {
    pub width: u16,
    pub height: u16,
}

impl TidepoolCanvasConfig {
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

pub type ProbeCanvasConfig = TidepoolCanvasConfig;

pub struct TidepoolScene {
    canvas: Canvas,
    environment: EnvironmentLayer,
    sparse_layers: Vec<Box<dyn SceneLayer>>,
    previous_dynamic_dirty: Vec<DirtyRegion>,
}

impl TidepoolScene {
    pub fn new(config: TidepoolCanvasConfig) -> Result<Self, CanvasError> {
        Ok(Self {
            canvas: Canvas::new(config.width, config.height, Rgba::rgb(0, 0, 0))?,
            environment: EnvironmentLayer::new(config)?,
            sparse_layers: tidepool_sparse_layers(config),
            previous_dynamic_dirty: Vec::new(),
        })
    }

    pub fn render(&mut self, tick: u64, activity: &SceneActivity) -> &Canvas {
        let frame = SceneFrame::new(tick, activity);
        self.canvas.clear_dirty();

        let environment_refreshed = self.environment.render_environment(&mut self.canvas, frame);
        if !environment_refreshed {
            for region in &self.previous_dynamic_dirty {
                self.environment.restore_region(&mut self.canvas, *region);
            }
        }

        for layer in &mut self.sparse_layers {
            layer.render(&mut self.canvas, frame);
        }

        self.previous_dynamic_dirty = self.canvas.dirty_regions();
        if environment_refreshed {
            self.canvas.mark_full_frame_required();
        }

        &self.canvas
    }

    pub fn layer_names(&self) -> Vec<&'static str> {
        self.environment
            .layer_names()
            .into_iter()
            .chain(
                self.sparse_layers
                    .iter()
                    .flat_map(|layer| layer.layer_names()),
            )
            .collect()
    }
}

pub type ProbeScene = TidepoolScene;

#[derive(Clone, Copy, Debug)]
pub struct SceneFrame<'a> {
    tick: u64,
    activity: &'a SceneActivity,
}

impl<'a> SceneFrame<'a> {
    pub const fn new(tick: u64, activity: &'a SceneActivity) -> Self {
        Self { tick, activity }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn activity(self) -> &'a SceneActivity {
        self.activity
    }
}

pub trait SceneLayer {
    fn name(&self) -> &'static str {
        "anonymous"
    }

    fn layer_names(&self) -> Vec<&'static str> {
        vec![self.name()]
    }

    fn render(&mut self, canvas: &mut Canvas, frame: SceneFrame<'_>);
}

pub struct LayeredScene {
    canvas: Canvas,
    layers: Vec<Box<dyn SceneLayer>>,
}

impl LayeredScene {
    pub fn new(
        config: TidepoolCanvasConfig,
        layers: Vec<Box<dyn SceneLayer>>,
    ) -> Result<Self, CanvasError> {
        Ok(Self {
            canvas: Canvas::new(config.width, config.height, Rgba::rgb(0, 0, 0))?,
            layers,
        })
    }

    pub fn render(&mut self, tick: u64, activity: &SceneActivity) -> &Canvas {
        self.canvas.clear_dirty();
        let frame = SceneFrame::new(tick, activity);
        for layer in &mut self.layers {
            layer.render(&mut self.canvas, frame);
        }
        &self.canvas
    }

    pub fn layer_names(&self) -> Vec<&'static str> {
        self.layers
            .iter()
            .flat_map(|layer| layer.layer_names())
            .collect()
    }
}
