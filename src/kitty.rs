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
}

impl KittyGraphicsEncoder {
    pub const fn new(image_id: KittyImageId) -> Self {
        Self {
            image_id,
            chunk_size: DEFAULT_CHUNK_SIZE,
        }
    }

    pub const fn with_chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size;
        self
    }

    pub fn encode_canvas(&self, canvas: &Canvas) -> Vec<u8> {
        let rgba = canvas_rgba_bytes(canvas);
        let payload = STANDARD.encode(rgba);
        let chunk_size = self.chunk_size.max(1);
        let chunk_count = payload.len().div_ceil(chunk_size).max(1);
        let mut bytes = Vec::with_capacity(payload.len() + chunk_count * 64);

        for (index, chunk) in payload.as_bytes().chunks(chunk_size).enumerate() {
            let more_chunks = usize::from(index + 1 < chunk_count);
            if index == 0 {
                bytes.extend_from_slice(
                    format!(
                        "\x1b_Ga=T,f=32,i={},s={},v={},m={more_chunks};",
                        self.image_id.value(),
                        canvas.width(),
                        canvas.height()
                    )
                    .as_bytes(),
                );
            } else {
                bytes.extend_from_slice(format!("\x1b_Gm={more_chunks};").as_bytes());
            }
            bytes.extend_from_slice(chunk);
            bytes.extend_from_slice(b"\x1b\\");
        }

        bytes
    }

    pub fn encode_delete(&self) -> Vec<u8> {
        format!("\x1b_Ga=d,i={};\x1b\\", self.image_id.value()).into_bytes()
    }
}

fn canvas_rgba_bytes(canvas: &Canvas) -> Vec<u8> {
    let mut bytes =
        Vec::with_capacity(usize::from(canvas.width()) * usize::from(canvas.height()) * 4);
    for pixel in canvas.pixels() {
        bytes.extend_from_slice(&[pixel.r, pixel.g, pixel.b, pixel.a]);
    }
    bytes
}
