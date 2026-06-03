use std::io::Write as _;

use base64::{Engine as _, engine::general_purpose::STANDARD};

use crate::canvas::Canvas;

const DEFAULT_CHUNK_SIZE: usize = 4096;

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
                write!(
                    bytes,
                    "\x1b_Ga=T,f=32,i={},s={},v={}",
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

    pub fn encode_delete(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.append_delete(&mut bytes);
        bytes
    }

    pub fn append_delete(&self, bytes: &mut Vec<u8>) {
        write!(bytes, "\x1b_Ga=d,d=i,i={};\x1b\\", self.image_id.value())
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
