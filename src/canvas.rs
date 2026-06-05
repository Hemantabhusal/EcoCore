use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const TRANSPARENT: Self = Self::new(0, 0, 0, 0);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn blend_over(self, background: Self) -> Self {
        let alpha = u16::from(self.a);
        let inverse_alpha = 255 - alpha;

        Self {
            r: blend_channel(self.r, background.r, alpha, inverse_alpha),
            g: blend_channel(self.g, background.g, alpha, inverse_alpha),
            b: blend_channel(self.b, background.b, alpha, inverse_alpha),
            a: blend_alpha(self.a, background.a),
        }
    }
}

fn blend_channel(foreground: u8, background: u8, alpha: u16, inverse_alpha: u16) -> u8 {
    let value = u16::from(foreground) * alpha + u16::from(background) * inverse_alpha;
    ((value + 127) / 255) as u8
}

fn blend_alpha(foreground: u8, background: u8) -> u8 {
    let foreground = u16::from(foreground);
    let background = u16::from(background);
    (foreground + (background * (255 - foreground)) / 255) as u8
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirtyRegion {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl DirtyRegion {
    pub const fn full(width: u16, height: u16) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    const fn single_pixel(x: u16, y: u16) -> Self {
        Self {
            x,
            y,
            width: 1,
            height: 1,
        }
    }

    fn include(self, x: u16, y: u16) -> Self {
        let min_x = self.x.min(x);
        let min_y = self.y.min(y);
        let max_x = (self.x + self.width - 1).max(x);
        let max_y = (self.y + self.height - 1).max(y);

        Self {
            x: min_x,
            y: min_y,
            width: max_x - min_x + 1,
            height: max_y - min_y + 1,
        }
    }

    fn include_region(self, region: Self) -> Self {
        let max_x = region.x.saturating_add(region.width.saturating_sub(1));
        let max_y = region.y.saturating_add(region.height.saturating_sub(1));
        self.include(region.x, region.y).include(max_x, max_y)
    }

    pub fn covers(self, width: u16, height: u16) -> bool {
        self.x == 0 && self.y == 0 && self.width >= width && self.height >= height
    }
}

const DIRTY_TILE_SIZE: u16 = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
struct DirtyTiles {
    columns: u16,
    rows: u16,
    dirty: Vec<bool>,
}

impl DirtyTiles {
    fn new(width: u16, height: u16) -> Self {
        let columns = width.div_ceil(DIRTY_TILE_SIZE);
        let rows = height.div_ceil(DIRTY_TILE_SIZE);
        Self {
            columns,
            rows,
            dirty: vec![false; usize::from(columns) * usize::from(rows)],
        }
    }

    fn clear(&mut self) {
        self.dirty.fill(false);
    }

    fn mark_pixel(&mut self, x: u16, y: u16) {
        let tile_x = x / DIRTY_TILE_SIZE;
        let tile_y = y / DIRTY_TILE_SIZE;
        self.mark_tile(tile_x, tile_y);
    }

    fn mark_region(&mut self, region: DirtyRegion) {
        let first_tile_x = region.x / DIRTY_TILE_SIZE;
        let first_tile_y = region.y / DIRTY_TILE_SIZE;
        let last_tile_x = (region.x + region.width - 1) / DIRTY_TILE_SIZE;
        let last_tile_y = (region.y + region.height - 1) / DIRTY_TILE_SIZE;

        for tile_y in first_tile_y..=last_tile_y {
            for tile_x in first_tile_x..=last_tile_x {
                self.mark_tile(tile_x, tile_y);
            }
        }
    }

    fn mark_all(&mut self) {
        self.dirty.fill(true);
    }

    fn regions(&self, canvas_width: u16, canvas_height: u16) -> Vec<DirtyRegion> {
        let mut regions = Vec::new();
        for tile_y in 0..self.rows {
            for tile_x in 0..self.columns {
                let index = usize::from(tile_y) * usize::from(self.columns) + usize::from(tile_x);
                if !self.dirty[index] {
                    continue;
                }
                let x = tile_x * DIRTY_TILE_SIZE;
                let y = tile_y * DIRTY_TILE_SIZE;
                regions.push(DirtyRegion {
                    x,
                    y,
                    width: DIRTY_TILE_SIZE.min(canvas_width - x),
                    height: DIRTY_TILE_SIZE.min(canvas_height - y),
                });
            }
        }
        regions
    }

    fn mark_tile(&mut self, tile_x: u16, tile_y: u16) {
        let index = usize::from(tile_y) * usize::from(self.columns) + usize::from(tile_x);
        if let Some(dirty) = self.dirty.get_mut(index) {
            *dirty = true;
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Canvas {
    width: u16,
    height: u16,
    pixels: Vec<Rgba>,
    dirty_region: Option<DirtyRegion>,
    dirty_tiles: DirtyTiles,
    full_frame_required: bool,
}

impl Canvas {
    pub fn new(width: u16, height: u16, fill: Rgba) -> Result<Self, CanvasError> {
        if width == 0 || height == 0 {
            return Err(CanvasError::InvalidSize { width, height });
        }

        let len = usize::from(width) * usize::from(height);
        Ok(Self {
            width,
            height,
            pixels: vec![fill; len],
            dirty_region: None,
            dirty_tiles: DirtyTiles::new(width, height),
            full_frame_required: false,
        })
    }

    pub const fn width(&self) -> u16 {
        self.width
    }

    pub const fn height(&self) -> u16 {
        self.height
    }

    pub fn pixels(&self) -> &[Rgba] {
        &self.pixels
    }

    pub fn pixels_mut(&mut self) -> &mut [Rgba] {
        self.dirty_region = Some(DirtyRegion::full(self.width, self.height));
        self.dirty_tiles.mark_all();
        self.full_frame_required = true;
        &mut self.pixels
    }

    pub fn pixel(&self, x: u16, y: u16) -> Option<Rgba> {
        self.index(x, y).map(|index| self.pixels[index])
    }

    pub fn set_pixel(&mut self, x: u16, y: u16, pixel: Rgba) -> Result<(), CanvasError> {
        let index = self.index(x, y).ok_or(CanvasError::OutOfBounds { x, y })?;
        if self.pixels[index] != pixel {
            self.pixels[index] = pixel;
            self.mark_dirty(x, y);
        }
        Ok(())
    }

    pub fn fill(&mut self, pixel: Rgba) {
        if self.pixels.iter().all(|current| *current == pixel) {
            return;
        }

        self.pixels.fill(pixel);
        self.dirty_region = Some(DirtyRegion::full(self.width, self.height));
        self.dirty_tiles.mark_all();
        self.full_frame_required = true;
    }

    pub fn copy_region_from(
        &mut self,
        source: &Canvas,
        region: DirtyRegion,
    ) -> Result<(), CanvasError> {
        self.validate_region(region)?;
        source.validate_region(region)?;

        let mut changed = false;
        let width = usize::from(self.width);
        let region_width = usize::from(region.width);
        for y in region.y..region.y + region.height {
            let start = usize::from(y) * width + usize::from(region.x);
            let end = start + region_width;
            if self.pixels[start..end] != source.pixels[start..end] {
                self.pixels[start..end].copy_from_slice(&source.pixels[start..end]);
                changed = true;
            }
        }

        if changed {
            self.mark_dirty_region(region);
        }

        Ok(())
    }

    pub const fn dirty_region(&self) -> Option<DirtyRegion> {
        self.dirty_region
    }

    pub fn dirty_regions(&self) -> Vec<DirtyRegion> {
        self.dirty_tiles.regions(self.width, self.height)
    }

    pub const fn full_frame_required(&self) -> bool {
        self.full_frame_required
    }

    pub fn clear_dirty(&mut self) {
        self.dirty_region = None;
        self.dirty_tiles.clear();
        self.full_frame_required = false;
    }

    pub fn mark_dirty_region(&mut self, region: DirtyRegion) {
        self.dirty_region = Some(match self.dirty_region {
            Some(current) => current.include_region(region),
            None => region,
        });
        self.dirty_tiles.mark_region(region);
    }

    pub fn mark_full_frame_required(&mut self) {
        self.dirty_region = Some(DirtyRegion::full(self.width, self.height));
        self.dirty_tiles.mark_all();
        self.full_frame_required = true;
    }

    fn mark_dirty(&mut self, x: u16, y: u16) {
        self.dirty_region = Some(match self.dirty_region {
            Some(region) => region.include(x, y),
            None => DirtyRegion::single_pixel(x, y),
        });
        self.dirty_tiles.mark_pixel(x, y);
    }

    fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }

        Some(usize::from(y) * usize::from(self.width) + usize::from(x))
    }

    fn validate_region(&self, region: DirtyRegion) -> Result<(), CanvasError> {
        if region.width == 0 || region.height == 0 {
            return Err(CanvasError::InvalidRegion { region });
        }
        let right = u32::from(region.x) + u32::from(region.width);
        let bottom = u32::from(region.y) + u32::from(region.height);
        if region.x >= self.width
            || region.y >= self.height
            || right > u32::from(self.width)
            || bottom > u32::from(self.height)
        {
            return Err(CanvasError::InvalidRegion { region });
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanvasError {
    InvalidSize { width: u16, height: u16 },
    InvalidRegion { region: DirtyRegion },
    OutOfBounds { x: u16, y: u16 },
}

impl fmt::Display for CanvasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize { width, height } => {
                write!(f, "canvas size must be non-zero, got {width}x{height}")
            }
            Self::InvalidRegion { region } => {
                write!(
                    f,
                    "canvas dirty region out of bounds: {},{} {}x{}",
                    region.x, region.y, region.width, region.height
                )
            }
            Self::OutOfBounds { x, y } => {
                write!(f, "canvas coordinate out of bounds: ({x}, {y})")
            }
        }
    }
}

impl Error for CanvasError {}
