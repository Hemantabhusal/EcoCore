use std::{
    error::Error,
    fmt, fs,
    io::{self, Cursor},
    path::Path,
};

use crate::canvas::{Canvas, CanvasError, Rgba};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sprite {
    width: u16,
    height: u16,
    pixels: Vec<Rgba>,
}

impl Sprite {
    pub fn from_png_file(path: impl AsRef<Path>) -> Result<Self, SpriteError> {
        let bytes = fs::read(path).map_err(SpriteError::Io)?;
        Self::from_png_bytes(&bytes)
    }

    pub fn from_png_bytes(bytes: &[u8]) -> Result<Self, SpriteError> {
        let decoder = png::Decoder::new(Cursor::new(bytes));
        let mut reader = decoder.read_info().map_err(SpriteError::PngDecode)?;
        let output_size = reader
            .output_buffer_size()
            .ok_or(SpriteError::UnsupportedOutputSize)?;
        let mut buffer = vec![0; output_size];
        let info = reader
            .next_frame(&mut buffer)
            .map_err(SpriteError::PngDecode)?;
        if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
            return Err(SpriteError::UnsupportedFormat {
                color_type: info.color_type,
                bit_depth: info.bit_depth,
            });
        }

        let width = u16::try_from(info.width).map_err(|_| SpriteError::ImageTooLarge {
            width: info.width,
            height: info.height,
        })?;
        let height = u16::try_from(info.height).map_err(|_| SpriteError::ImageTooLarge {
            width: info.width,
            height: info.height,
        })?;
        let pixels = buffer[..info.buffer_size()]
            .chunks_exact(4)
            .map(|rgba| Rgba::new(rgba[0], rgba[1], rgba[2], rgba[3]))
            .collect();

        Self::from_rgba_pixels(width, height, pixels)
    }

    pub fn from_rgba_pixels(
        width: u16,
        height: u16,
        pixels: Vec<Rgba>,
    ) -> Result<Self, SpriteError> {
        if width == 0 || height == 0 {
            return Err(SpriteError::InvalidSize { width, height });
        }
        let expected = usize::from(width) * usize::from(height);
        if pixels.len() != expected {
            return Err(SpriteError::PixelCountMismatch {
                expected,
                actual: pixels.len(),
            });
        }

        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub const fn width(&self) -> u16 {
        self.width
    }

    pub const fn height(&self) -> u16 {
        self.height
    }

    pub fn pixel(&self, x: u16, y: u16) -> Option<Rgba> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(self.pixels[usize::from(y) * usize::from(self.width) + usize::from(x)])
    }

    pub fn blit_scaled(&self, canvas: &mut Canvas, blit: SpriteBlit) -> Result<(), SpriteError> {
        if blit.scale == 0 {
            return Err(SpriteError::InvalidScale);
        }
        let scale = blit.scale;
        let scaled_width = self.width.saturating_mul(scale);
        let scaled_height = self.height.saturating_mul(scale);

        if blit.x >= canvas.width()
            || blit.y >= canvas.height()
            || blit.x.saturating_add(scaled_width) > canvas.width()
            || blit.y.saturating_add(scaled_height) > canvas.height()
        {
            return Err(SpriteError::OutOfCanvas {
                x: blit.x,
                y: blit.y,
                width: scaled_width,
                height: scaled_height,
            });
        }

        for source_y in 0..self.height {
            for source_x in 0..self.width {
                let source = self
                    .pixel(source_x, source_y)
                    .expect("source coordinate is in bounds");
                if source.a == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let target_x = blit.x + source_x * scale + dx;
                        let target_y = blit.y + source_y * scale + dy;
                        let background = canvas
                            .pixel(target_x, target_y)
                            .expect("validated blit target is in bounds");
                        canvas
                            .set_pixel(target_x, target_y, source.blend_over(background))
                            .map_err(SpriteError::Canvas)?;
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpriteBlit {
    pub x: u16,
    pub y: u16,
    pub scale: u16,
}

#[derive(Debug)]
pub enum SpriteError {
    Io(io::Error),
    PngDecode(png::DecodingError),
    UnsupportedOutputSize,
    UnsupportedFormat {
        color_type: png::ColorType,
        bit_depth: png::BitDepth,
    },
    ImageTooLarge {
        width: u32,
        height: u32,
    },
    InvalidSize {
        width: u16,
        height: u16,
    },
    PixelCountMismatch {
        expected: usize,
        actual: usize,
    },
    InvalidScale,
    OutOfCanvas {
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    },
    Canvas(CanvasError),
}

impl fmt::Display for SpriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "sprite file read failed: {error}"),
            Self::PngDecode(error) => write!(f, "sprite png decode failed: {error}"),
            Self::UnsupportedOutputSize => write!(f, "sprite png output size is unknown"),
            Self::UnsupportedFormat {
                color_type,
                bit_depth,
            } => write!(
                f,
                "sprite png must be 8-bit RGBA, got {color_type:?} {bit_depth:?}"
            ),
            Self::ImageTooLarge { width, height } => {
                write!(
                    f,
                    "sprite image is too large for canvas coordinates: {width}x{height}"
                )
            }
            Self::InvalidSize { width, height } => {
                write!(f, "sprite size must be non-zero, got {width}x{height}")
            }
            Self::PixelCountMismatch { expected, actual } => {
                write!(f, "sprite expected {expected} pixels, got {actual}")
            }
            Self::InvalidScale => write!(f, "sprite scale must be at least 1"),
            Self::OutOfCanvas {
                x,
                y,
                width,
                height,
            } => write!(
                f,
                "sprite blit out of canvas bounds: {x},{y} {width}x{height}"
            ),
            Self::Canvas(error) => write!(f, "sprite canvas write failed: {error}"),
        }
    }
}

impl Error for SpriteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::PngDecode(error) => Some(error),
            Self::Canvas(error) => Some(error),
            Self::UnsupportedOutputSize
            | Self::UnsupportedFormat { .. }
            | Self::ImageTooLarge { .. }
            | Self::InvalidSize { .. }
            | Self::PixelCountMismatch { .. }
            | Self::InvalidScale
            | Self::OutOfCanvas { .. } => None,
        }
    }
}
