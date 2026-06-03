use ecosystem::{
    layout::{ImagePlacement, centered_image_placement},
    terminal::{TerminalSize, move_cursor_to},
};

#[test]
fn centered_image_placement_uses_one_based_cursor_coordinates() {
    let placement = centered_image_placement(TerminalSize::new(120, 40), 30, 10);

    assert_eq!(
        placement,
        ImagePlacement {
            cursor_column: 46,
            cursor_row: 16,
            columns: 30,
            rows: 10
        }
    );
}

#[test]
fn centered_image_placement_clamps_to_small_terminals() {
    let placement = centered_image_placement(TerminalSize::new(20, 8), 30, 10);

    assert_eq!(
        placement,
        ImagePlacement {
            cursor_column: 1,
            cursor_row: 1,
            columns: 20,
            rows: 8
        }
    );
}

#[test]
fn cursor_movement_uses_ansi_row_then_column_order() {
    let command = String::from_utf8(move_cursor_to(6, 4)).expect("cursor command is utf8");

    assert_eq!(command, "\u{1b}[4;6H");
}
