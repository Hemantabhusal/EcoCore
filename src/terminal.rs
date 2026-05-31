use std::fmt::Write as _;

use crate::framebuffer::{Cell, Color, Framebuffer, FramebufferError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSize {
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodeOutput {
    pub bytes: Vec<u8>,
    pub changed_cells: usize,
}

#[derive(Clone, Debug, Default)]
pub struct AnsiDiffEncoder;

impl AnsiDiffEncoder {
    pub const fn new() -> Self {
        Self
    }

    pub fn encode_diff(
        &self,
        previous: &Framebuffer,
        current: &Framebuffer,
    ) -> Result<EncodeOutput, FramebufferError> {
        if previous.width() != current.width() || previous.height() != current.height() {
            return Err(FramebufferError::SizeMismatch {
                previous_width: previous.width(),
                previous_height: previous.height(),
                current_width: current.width(),
                current_height: current.height(),
            });
        }

        let mut bytes = Vec::new();
        let mut changed_cells = 0;

        for y in 0..current.height() {
            for x in 0..current.width() {
                let previous_cell = previous
                    .get(x, y)
                    .expect("coordinates were already bounded by framebuffer size");
                let current_cell = current
                    .get(x, y)
                    .expect("coordinates were already bounded by framebuffer size");

                if previous_cell == current_cell {
                    continue;
                }

                changed_cells += 1;
                push_cell(&mut bytes, x, y, current_cell);
            }
        }

        if changed_cells > 0 {
            bytes.extend_from_slice(b"\x1b[0m");
        }

        Ok(EncodeOutput {
            bytes,
            changed_cells,
        })
    }
}

fn push_cell(bytes: &mut Vec<u8>, x: u16, y: u16, cell: &Cell) {
    let mut sequence = String::new();
    // ANSI cursor coordinates are 1-based, while framebuffer coordinates are 0-based.
    let _ = write!(
        sequence,
        "\x1b[{};{}H{}{}{}",
        y + 1,
        x + 1,
        truecolor_fg(cell.fg),
        truecolor_bg(cell.bg),
        cell.glyph
    );
    bytes.extend_from_slice(sequence.as_bytes());
}

fn truecolor_fg(color: Color) -> String {
    format!("\x1b[38;2;{};{};{}m", color.r, color.g, color.b)
}

fn truecolor_bg(color: Color) -> String {
    format!("\x1b[48;2;{};{};{}m", color.r, color.g, color.b)
}
