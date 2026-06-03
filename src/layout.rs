use crate::terminal::TerminalSize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImagePlacement {
    pub cursor_column: u16,
    pub cursor_row: u16,
    pub columns: u16,
    pub rows: u16,
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
