use crate::terminal::TerminalSize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellSize {
    pub width_pixels: u16,
    pub height_pixels: u16,
}

impl CellSize {
    pub const fn new(width_pixels: u16, height_pixels: u16) -> Self {
        Self {
            width_pixels,
            height_pixels,
        }
    }

    fn sanitized(self) -> Self {
        Self {
            width_pixels: self.width_pixels.max(1),
            height_pixels: self.height_pixels.max(1),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImagePlacement {
    pub cursor_column: u16,
    pub cursor_row: u16,
    pub columns: u16,
    pub rows: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphicsLayout {
    pub placement: ImagePlacement,
    pub canvas_width: u16,
    pub canvas_height: u16,
}

pub fn graphics_layout(
    terminal: TerminalSize,
    requested_columns: u16,
    requested_rows: u16,
    cell_size: CellSize,
) -> GraphicsLayout {
    let placement = centered_image_placement(terminal, requested_columns, requested_rows);
    let cell_size = cell_size.sanitized();

    GraphicsLayout {
        placement,
        canvas_width: placement.columns.saturating_mul(cell_size.width_pixels),
        canvas_height: placement.rows.saturating_mul(cell_size.height_pixels),
    }
}

pub fn centered_image_placement(
    terminal: TerminalSize,
    requested_columns: u16,
    requested_rows: u16,
) -> ImagePlacement {
    let columns = requested_columns.max(1).min(terminal.width.max(1));
    let rows = requested_rows.max(1).min(terminal.height.max(1));

    ImagePlacement {
        cursor_column: ((terminal.width - columns) / 2) + 1,
        cursor_row: ((terminal.height - rows) / 2) + 1,
        columns,
        rows,
    }
}
