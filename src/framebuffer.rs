use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cell {
    pub glyph: char,
    pub fg: Color,
    pub bg: Color,
}

impl Cell {
    pub const fn new(glyph: char, fg: Color, bg: Color) -> Self {
        Self { glyph, fg, bg }
    }

    pub const fn blank() -> Self {
        Self::new(' ', Color::rgb(180, 190, 200), Color::rgb(0, 0, 0))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Framebuffer {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
}

impl Framebuffer {
    pub fn new(width: u16, height: u16, fill: Cell) -> Result<Self, FramebufferError> {
        if width == 0 || height == 0 {
            return Err(FramebufferError::InvalidSize { width, height });
        }

        let len = usize::from(width) * usize::from(height);
        Ok(Self {
            width,
            height,
            cells: vec![fill; len],
        })
    }

    pub const fn width(&self) -> u16 {
        self.width
    }

    pub const fn height(&self) -> u16 {
        self.height
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn get(&self, x: u16, y: u16) -> Option<&Cell> {
        self.index(x, y).map(|index| &self.cells[index])
    }

    pub fn set(&mut self, x: u16, y: u16, cell: Cell) -> Result<(), FramebufferError> {
        let index = self
            .index(x, y)
            .ok_or(FramebufferError::OutOfBounds { x, y })?;
        self.cells[index] = cell;
        Ok(())
    }

    fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }

        Some(usize::from(y) * usize::from(self.width) + usize::from(x))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FramebufferError {
    InvalidSize {
        width: u16,
        height: u16,
    },
    OutOfBounds {
        x: u16,
        y: u16,
    },
    SizeMismatch {
        previous_width: u16,
        previous_height: u16,
        current_width: u16,
        current_height: u16,
    },
}

impl fmt::Display for FramebufferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize { width, height } => {
                write!(f, "framebuffer size must be non-zero, got {width}x{height}")
            }
            Self::OutOfBounds { x, y } => {
                write!(f, "framebuffer coordinate out of bounds: ({x}, {y})")
            }
            Self::SizeMismatch {
                previous_width,
                previous_height,
                current_width,
                current_height,
            } => write!(
                f,
                "framebuffer sizes differ: previous {previous_width}x{previous_height}, current {current_width}x{current_height}"
            ),
        }
    }
}

impl Error for FramebufferError {}
