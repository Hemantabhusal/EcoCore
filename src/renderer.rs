use crate::{
    canvas::Canvas,
    kitty::{KittyGraphicsEncoder, KittyImageId, KittyPlacement},
    layout::{ImagePlacement, centered_image_placement},
    terminal::{TerminalSize, move_cursor_to},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KittyRendererConfig {
    pub image_ids: [KittyImageId; 2],
    pub image_columns: u16,
    pub image_rows: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedKittyFrame {
    pub bytes: Vec<u8>,
    pub placement: ImagePlacement,
    pub image_id: KittyImageId,
    pub deleted_image_id: Option<KittyImageId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KittyRenderer {
    config: KittyRendererConfig,
    next_buffer_index: usize,
    visible_image_id: Option<KittyImageId>,
}

impl KittyRenderer {
    pub const fn new(config: KittyRendererConfig) -> Self {
        Self {
            config,
            next_buffer_index: 0,
            visible_image_id: None,
        }
    }

    pub fn render_frame(
        &mut self,
        terminal_size: TerminalSize,
        canvas: &Canvas,
    ) -> RenderedKittyFrame {
        let image_id = self.config.image_ids[self.next_buffer_index];
        let previous_image_id = self
            .visible_image_id
            .filter(|previous| *previous != image_id);
        let placement = centered_image_placement(
            terminal_size,
            self.config.image_columns,
            self.config.image_rows,
        );

        let mut bytes = move_cursor_to(placement.cursor_column, placement.cursor_row);
        bytes.extend_from_slice(
            &KittyGraphicsEncoder::new(image_id)
                .with_placement(KittyPlacement::new(placement.columns, placement.rows))
                .encode_canvas(canvas),
        );

        // Draw the new image before deleting the previous buffer so the
        // terminal is less likely to show an empty placement between frames.
        if let Some(previous) = previous_image_id {
            bytes.extend_from_slice(&KittyGraphicsEncoder::new(previous).encode_delete());
        }

        self.visible_image_id = Some(image_id);
        self.next_buffer_index = (self.next_buffer_index + 1) % self.config.image_ids.len();

        RenderedKittyFrame {
            bytes,
            placement,
            image_id,
            deleted_image_id: previous_image_id,
        }
    }

    pub fn reset(&mut self) -> Vec<u8> {
        self.next_buffer_index = 0;
        self.visible_image_id = None;

        let mut bytes = Vec::new();
        for image_id in self.config.image_ids {
            bytes.extend_from_slice(&KittyGraphicsEncoder::new(image_id).encode_delete());
        }
        bytes
    }
}
