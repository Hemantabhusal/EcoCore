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
    const fn single_pixel(x: u16, y: u16) -> Self {
        Self {
            x,
            y,
            width: 1,
            height: 1,
        }
    }

    const fn full(width: u16, height: u16) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Canvas {
    width: u16,
    height: u16,
    pixels: Vec<Rgba>,
    dirty_region: Option<DirtyRegion>,
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
    }

    pub const fn dirty_region(&self) -> Option<DirtyRegion> {
        self.dirty_region
    }

    pub fn clear_dirty(&mut self) {
        self.dirty_region = None;
    }

    fn mark_dirty(&mut self, x: u16, y: u16) {
        self.dirty_region = Some(match self.dirty_region {
            Some(region) => region.include(x, y),
            None => DirtyRegion::single_pixel(x, y),
        });
    }

    fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }

        Some(usize::from(y) * usize::from(self.width) + usize::from(x))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanvasError {
    InvalidSize { width: u16, height: u16 },
    OutOfBounds { x: u16, y: u16 },
}

impl fmt::Display for CanvasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize { width, height } => {
                write!(f, "canvas size must be non-zero, got {width}x{height}")
            }
            Self::OutOfBounds { x, y } => {
                write!(f, "canvas coordinate out of bounds: ({x}, {y})")
            }
        }
    }
}

impl Error for CanvasError {}
