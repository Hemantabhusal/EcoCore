use std::io::Cursor;

use ecosystem::{
    assets::{Sprite, SpriteBlit},
    canvas::{Canvas, Rgba},
};

#[test]
fn sprite_decodes_rgba_png_bytes() {
    let png = tiny_rgba_png(2, 1, &[255, 0, 0, 255, 0, 80, 255, 128]);

    let sprite = Sprite::from_png_bytes(&png).expect("valid rgba png");

    assert_eq!(sprite.width(), 2);
    assert_eq!(sprite.height(), 1);
    assert_eq!(sprite.pixel(0, 0), Some(Rgba::new(255, 0, 0, 255)));
    assert_eq!(sprite.pixel(1, 0), Some(Rgba::new(0, 80, 255, 128)));
}

#[test]
fn sprite_blit_scales_with_nearest_neighbor_and_alpha_blends() {
    let sprite = Sprite::from_rgba_pixels(
        2,
        1,
        vec![Rgba::new(255, 0, 0, 255), Rgba::new(0, 0, 255, 128)],
    )
    .expect("valid sprite");
    let mut canvas = Canvas::new(6, 3, Rgba::rgb(10, 20, 30)).expect("valid canvas");

    sprite
        .blit_scaled(
            &mut canvas,
            SpriteBlit {
                x: 1,
                y: 1,
                scale: 2,
            },
        )
        .expect("sprite fits canvas");

    assert_eq!(canvas.pixel(1, 1), Some(Rgba::rgb(255, 0, 0)));
    assert_eq!(canvas.pixel(2, 1), Some(Rgba::rgb(255, 0, 0)));
    assert_eq!(canvas.pixel(3, 1), Some(Rgba::new(5, 10, 143, 255)));
    assert_eq!(
        canvas.dirty_region(),
        Some(ecosystem::canvas::DirtyRegion {
            x: 1,
            y: 1,
            width: 4,
            height: 2,
        })
    );
}

#[test]
fn sprite_blit_rejects_zero_scale() {
    let sprite = Sprite::from_rgba_pixels(1, 1, vec![Rgba::rgb(255, 0, 0)]).expect("valid sprite");
    let mut canvas = Canvas::new(2, 2, Rgba::rgb(0, 0, 0)).expect("valid canvas");

    let error = sprite
        .blit_scaled(
            &mut canvas,
            SpriteBlit {
                x: 0,
                y: 0,
                scale: 0,
            },
        )
        .expect_err("zero scale rejected");

    assert_eq!(error.to_string(), "sprite scale must be at least 1");
}

fn tiny_rgba_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut encoder = png::Encoder::new(Cursor::new(&mut output), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("png header");
    writer.write_image_data(rgba).expect("png pixels");
    drop(writer);
    output
}
