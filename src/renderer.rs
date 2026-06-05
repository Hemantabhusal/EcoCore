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
    pub partial_update: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KittyRenderer {
    config: KittyRendererConfig,
    visible_image_id: Option<KittyImageId>,
    visible_placement: Option<ImagePlacement>,
    stats: KittyRendererStats,
    encode_scratch: KittyEncodeScratch,
}

impl KittyRenderer {
    pub fn new(config: KittyRendererConfig) -> Self {
        Self {
            config,
            visible_image_id: None,
            visible_placement: None,
            stats: KittyRendererStats::new(),
            encode_scratch: KittyEncodeScratch::default(),
        }
    }

    pub fn render_frame(
        &mut self,
        terminal_size: TerminalSize,
        canvas: &Canvas,
    ) -> RenderedKittyFrame {
        let image_id = self.config.image_ids[0];
        let placement = centered_image_placement(
            terminal_size,
            self.config.image_columns,
            self.config.image_rows,
        );
        let dirty_region = canvas.dirty_region();
        let full_update_required = self.visible_image_id.is_none()
            || self.visible_placement != Some(placement)
            || canvas.full_frame_required()
            || dirty_region.is_none();

        let mut bytes = Vec::new();
        if full_update_required {
            bytes.extend_from_slice(&move_cursor_to(
                placement.cursor_column,
                placement.cursor_row,
            ));
            KittyGraphicsEncoder::new(image_id)
                .with_placement(KittyPlacement::new(placement.columns, placement.rows))
                .append_canvas_with_scratch(canvas, &mut bytes, &mut self.encode_scratch);
        } else if let Some(region) = dirty_region {
            KittyGraphicsEncoder::new(image_id).append_frame_region_with_scratch(
                canvas,
                region,
                &mut bytes,
                &mut self.encode_scratch,
            );
        }

        self.visible_image_id = Some(image_id);
        self.visible_placement = Some(placement);
        self.stats
            .record_frame(bytes.len(), full_update_required, image_id, None, placement);

        RenderedKittyFrame {
            bytes,
            placement,
            image_id,
            deleted_image_id: None,
            partial_update: !full_update_required,
        }
    }

    pub fn reset(&mut self) -> Vec<u8> {
        self.visible_image_id = None;
        self.visible_placement = None;

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
    full_frame_bytes: u64,
    partial_frame_bytes: u64,
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
            full_frame_bytes: 0,
            partial_frame_bytes: 0,
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
        full_update: bool,
        image_id: KittyImageId,
        deleted_image_id: Option<KittyImageId>,
        placement: ImagePlacement,
    ) {
        self.frames_rendered += 1;
        self.frame_bytes += bytes as u64;
        if full_update {
            self.full_frame_bytes += bytes as u64;
        } else {
            self.partial_frame_bytes += bytes as u64;
        }
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

    pub const fn full_frame_bytes(self) -> u64 {
        self.full_frame_bytes
    }

    pub const fn partial_frame_bytes(self) -> u64 {
        self.partial_frame_bytes
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
