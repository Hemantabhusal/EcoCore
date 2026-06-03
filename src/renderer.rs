use crate::{
    canvas::Canvas,
    kitty::{KittyEncodeScratch, KittyGraphicsEncoder, KittyImageId, KittyPlacement},
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
    stats: KittyRendererStats,
    encode_scratch: KittyEncodeScratch,
}

impl KittyRenderer {
    pub fn new(config: KittyRendererConfig) -> Self {
        Self {
            config,
            next_buffer_index: 0,
            visible_image_id: None,
            stats: KittyRendererStats::new(),
            encode_scratch: KittyEncodeScratch::default(),
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
        KittyGraphicsEncoder::new(image_id)
            .with_placement(KittyPlacement::new(placement.columns, placement.rows))
            .append_canvas_with_scratch(canvas, &mut bytes, &mut self.encode_scratch);

        // Draw the new image before deleting the previous buffer so the
        // terminal is less likely to show an empty placement between frames.
        if let Some(previous) = previous_image_id {
            KittyGraphicsEncoder::new(previous).append_delete(&mut bytes);
        }

        self.visible_image_id = Some(image_id);
        self.next_buffer_index = (self.next_buffer_index + 1) % self.config.image_ids.len();
        self.stats
            .record_frame(bytes.len(), image_id, previous_image_id, placement);

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
            KittyGraphicsEncoder::new(image_id).append_delete(&mut bytes);
        }
        self.stats.record_reset(bytes.len());
        bytes
    }

    pub const fn stats(&self) -> KittyRendererStats {
        self.stats
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KittyRendererStats {
    frames_rendered: u64,
    frame_bytes: u64,
    resets: u64,
    cleanup_bytes: u64,
    latest_image_id: Option<KittyImageId>,
    latest_deleted_image_id: Option<KittyImageId>,
    latest_placement: Option<ImagePlacement>,
}

impl KittyRendererStats {
    const fn new() -> Self {
        Self {
            frames_rendered: 0,
            frame_bytes: 0,
            resets: 0,
            cleanup_bytes: 0,
            latest_image_id: None,
            latest_deleted_image_id: None,
            latest_placement: None,
        }
    }

    fn record_frame(
        &mut self,
        bytes: usize,
        image_id: KittyImageId,
        deleted_image_id: Option<KittyImageId>,
        placement: ImagePlacement,
    ) {
        self.frames_rendered += 1;
        self.frame_bytes += bytes as u64;
        self.latest_image_id = Some(image_id);
        self.latest_deleted_image_id = deleted_image_id;
        self.latest_placement = Some(placement);
    }

    fn record_reset(&mut self, bytes: usize) {
        self.resets += 1;
        self.cleanup_bytes += bytes as u64;
        self.latest_image_id = None;
        self.latest_deleted_image_id = None;
        self.latest_placement = None;
    }

    pub const fn frames_rendered(self) -> u64 {
        self.frames_rendered
    }

    pub const fn frame_bytes(self) -> u64 {
        self.frame_bytes
    }

    pub fn average_frame_bytes(self) -> u64 {
        average(self.frame_bytes, self.frames_rendered)
    }

    pub const fn resets(self) -> u64 {
        self.resets
    }

    pub const fn cleanup_bytes(self) -> u64 {
        self.cleanup_bytes
    }

    pub const fn total_protocol_bytes(self) -> u64 {
        self.frame_bytes + self.cleanup_bytes
    }

    pub const fn latest_image_id(self) -> Option<KittyImageId> {
        self.latest_image_id
    }

    pub const fn latest_deleted_image_id(self) -> Option<KittyImageId> {
        self.latest_deleted_image_id
    }

    pub const fn latest_placement(self) -> Option<ImagePlacement> {
        self.latest_placement
    }
}

const fn average(total: u64, count: u64) -> u64 {
    match count {
        0 => 0,
        count => total / count,
    }
}
