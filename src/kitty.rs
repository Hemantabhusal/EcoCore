use std::io::Write as _;

use base64::{Engine as _, engine::general_purpose::STANDARD};

use crate::canvas::{Canvas, DirtyRegion};

const DEFAULT_CHUNK_SIZE: usize = 4096;
const QUIET_RESPONSE_MODE: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KittyImageId(u32);

impl KittyImageId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KittyGraphicsEncoder {
    image_id: KittyImageId,
    chunk_size: usize,
    placement: Option<KittyPlacement>,
}

impl KittyGraphicsEncoder {
    pub const fn new(image_id: KittyImageId) -> Self {
        Self {
            image_id,
            chunk_size: DEFAULT_CHUNK_SIZE,
            placement: None,
        }
    }

    pub const fn with_chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size;
        self
    }

    pub const fn with_placement(mut self, placement: KittyPlacement) -> Self {
        self.placement = Some(placement);
        self
    }

    pub fn encode_canvas(&self, canvas: &Canvas) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.append_canvas(canvas, &mut bytes);
        bytes
    }

    pub fn encode_frame_region(&self, canvas: &Canvas, region: DirtyRegion) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.append_frame_region(canvas, region, &mut bytes);
        bytes
    }

    pub fn append_canvas(&self, canvas: &Canvas, bytes: &mut Vec<u8>) {
        let mut scratch = KittyEncodeScratch::default();
        self.append_canvas_with_scratch(canvas, bytes, &mut scratch);
    }

    pub fn append_canvas_with_scratch(
        &self,
        canvas: &Canvas,
        bytes: &mut Vec<u8>,
        scratch: &mut KittyEncodeScratch,
    ) {
        scratch.load_canvas(canvas);
        STANDARD.encode_string(&scratch.rgba, &mut scratch.payload);
        let chunk_size = self.chunk_size.max(1);
        let chunk_count = scratch.payload.len().div_ceil(chunk_size).max(1);
        bytes.reserve(scratch.payload.len() + chunk_count * 64);

        for (index, chunk) in scratch.payload.as_bytes().chunks(chunk_size).enumerate() {
            let more_chunks = usize::from(index + 1 < chunk_count);
            if index == 0 {
                // The renderer does not currently consume Kitty response ACKs.
                // Quiet mode prevents success responses from leaking into the
                // terminal input stream and contaminating trace output.
                write!(
                    bytes,
                    "\x1b_Ga=T,q={QUIET_RESPONSE_MODE},f=32,i={},s={},v={}",
                    self.image_id.value(),
                    canvas.width(),
                    canvas.height()
                )
                .expect("writing to an in-memory byte buffer cannot fail");
                if let Some(placement) = self.placement {
                    write!(bytes, ",c={},r={},C=1", placement.columns, placement.rows)
                        .expect("writing to an in-memory byte buffer cannot fail");
                }
                write!(bytes, ",m={more_chunks};")
                    .expect("writing to an in-memory byte buffer cannot fail");
            } else {
                write!(bytes, "\x1b_Gm={more_chunks};")
                    .expect("writing to an in-memory byte buffer cannot fail");
            }
            bytes.extend_from_slice(chunk);
            bytes.extend_from_slice(b"\x1b\\");
        }
    }

    pub fn append_frame_region(&self, canvas: &Canvas, region: DirtyRegion, bytes: &mut Vec<u8>) {
        let mut scratch = KittyEncodeScratch::default();
        self.append_frame_region_with_scratch(canvas, region, bytes, &mut scratch);
    }

    pub fn append_frame_region_with_scratch(
        &self,
        canvas: &Canvas,
        region: DirtyRegion,
        bytes: &mut Vec<u8>,
        scratch: &mut KittyEncodeScratch,
    ) {
        scratch.load_canvas_region(canvas, region);
        STANDARD.encode_string(&scratch.rgba, &mut scratch.payload);
        let chunk_size = self.chunk_size.max(1);
        let chunk_count = scratch.payload.len().div_ceil(chunk_size).max(1);
        bytes.reserve(scratch.payload.len() + chunk_count * 80);

        for (index, chunk) in scratch.payload.as_bytes().chunks(chunk_size).enumerate() {
            let more_chunks = usize::from(index + 1 < chunk_count);
            if index == 0 {
                // `a=f` edits the root frame of the already displayed image.
                // This is the protocol path for pixel payload updates; `a=p`
                // only places image data that has already been transmitted.
                write!(
                    bytes,
                    "\x1b_Ga=f,q={QUIET_RESPONSE_MODE},f=32,i={},r=1,x={},y={},s={},v={},X=1,m={more_chunks};",
                    self.image_id.value(),
                    region.x,
                    region.y,
                    region.width,
                    region.height
                )
                .expect("writing to an in-memory byte buffer cannot fail");
            } else {
                write!(bytes, "\x1b_Gm={more_chunks};")
                    .expect("writing to an in-memory byte buffer cannot fail");
            }
            bytes.extend_from_slice(chunk);
            bytes.extend_from_slice(b"\x1b\\");
        }
    }

    pub fn encode_delete(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.append_delete(&mut bytes);
        bytes
    }

    pub fn append_delete(&self, bytes: &mut Vec<u8>) {
        write!(
            bytes,
            "\x1b_Ga=d,q={QUIET_RESPONSE_MODE},d=i,i={};\x1b\\",
            self.image_id.value()
        )
        .expect("writing to an in-memory byte buffer cannot fail");
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct KittyEncodeScratch {
    rgba: Vec<u8>,
    payload: String,
}

impl KittyEncodeScratch {
    fn load_canvas(&mut self, canvas: &Canvas) {
        self.rgba.clear();
        self.payload.clear();
        self.rgba
            .reserve(usize::from(canvas.width()) * usize::from(canvas.height()) * 4);
        for pixel in canvas.pixels() {
            self.rgba
                .extend_from_slice(&[pixel.r, pixel.g, pixel.b, pixel.a]);
        }
    }

    fn load_canvas_region(&mut self, canvas: &Canvas, region: DirtyRegion) {
        self.rgba.clear();
        self.payload.clear();
        self.rgba
            .reserve(usize::from(region.width) * usize::from(region.height) * 4);

        let canvas_width = usize::from(canvas.width());
        let x = usize::from(region.x);
        let region_width = usize::from(region.width);
        for y in region.y..region.y + region.height {
            let start = usize::from(y) * canvas_width + x;
            let end = start + region_width;
            for pixel in &canvas.pixels()[start..end] {
                self.rgba
                    .extend_from_slice(&[pixel.r, pixel.g, pixel.b, pixel.a]);
            }
        }
    }

    pub fn capacities(&self) -> (usize, usize) {
        (self.rgba.capacity(), self.payload.capacity())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KittyPlacement {
    pub columns: u16,
    pub rows: u16,
}

impl KittyPlacement {
    pub const fn new(columns: u16, rows: u16) -> Self {
        let columns = if columns == 0 { 1 } else { columns };
        let rows = if rows == 0 { 1 } else { rows };

        Self { columns, rows }
    }
}
