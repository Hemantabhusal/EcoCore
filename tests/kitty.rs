use ecosystem::{
    canvas::{Canvas, Rgba},
    kitty::{KittyEncodeScratch, KittyGraphicsEncoder, KittyImageId, KittyPlacement},
};

#[test]
fn kitty_encoder_transmits_canvas_as_rgba_graphics_command() {
    let mut canvas = Canvas::new(2, 1, Rgba::TRANSPARENT).expect("valid canvas");
    canvas
        .set_pixel(0, 0, Rgba::new(1, 2, 3, 4))
        .expect("pixel in bounds");
    canvas
        .set_pixel(1, 0, Rgba::new(5, 6, 7, 8))
        .expect("pixel in bounds");

    let bytes = KittyGraphicsEncoder::new(KittyImageId::new(7)).encode_canvas(&canvas);
    let command = String::from_utf8(bytes).expect("kitty commands are utf8");

    assert_eq!(
        command,
        "\u{1b}_Ga=T,f=32,i=7,s=2,v=1,m=0;AQIDBAUGBwg=\u{1b}\\"
    );
}

#[test]
fn kitty_encoder_chunks_large_payloads_with_continuation_flags() {
    let canvas = Canvas::new(3, 1, Rgba::rgb(255, 0, 0)).expect("valid canvas");

    let bytes = KittyGraphicsEncoder::new(KittyImageId::new(9))
        .with_chunk_size(8)
        .encode_canvas(&canvas);
    let command = String::from_utf8(bytes).expect("kitty commands are utf8");

    assert!(command.starts_with("\u{1b}_Ga=T,f=32,i=9,s=3,v=1,m=1;"));
    assert!(command.contains("\u{1b}\\\u{1b}_Gm=0;"));
    assert!(command.ends_with("\u{1b}\\"));
}

#[test]
fn kitty_encoder_can_place_canvas_in_a_terminal_cell_rectangle() {
    let canvas = Canvas::new(2, 1, Rgba::rgb(255, 0, 0)).expect("valid canvas");

    let bytes = KittyGraphicsEncoder::new(KittyImageId::new(11))
        .with_placement(KittyPlacement::new(30, 10))
        .encode_canvas(&canvas);
    let command = String::from_utf8(bytes).expect("kitty commands are utf8");

    assert!(command.starts_with("\u{1b}_Ga=T,f=32,i=11,s=2,v=1,c=30,r=10,C=1,m=0;"));
}

#[test]
fn kitty_encoder_can_reuse_encode_scratch_buffers() {
    let canvas = Canvas::new(2, 1, Rgba::rgb(10, 20, 30)).expect("valid canvas");
    let encoder = KittyGraphicsEncoder::new(KittyImageId::new(12));
    let mut scratch = KittyEncodeScratch::default();
    let mut first = Vec::new();
    let mut second = Vec::new();

    encoder.append_canvas_with_scratch(&canvas, &mut first, &mut scratch);
    let first_capacities = scratch.capacities();
    encoder.append_canvas_with_scratch(&canvas, &mut second, &mut scratch);

    assert_eq!(first, second);
    assert_eq!(scratch.capacities(), first_capacities);
}

#[test]
fn kitty_encoder_deletes_image_by_id_for_cleanup() {
    let bytes = KittyGraphicsEncoder::new(KittyImageId::new(42)).encode_delete();
    let command = String::from_utf8(bytes).expect("kitty commands are utf8");

    assert_eq!(command, "\u{1b}_Ga=d,d=i,i=42;\u{1b}\\");
}
