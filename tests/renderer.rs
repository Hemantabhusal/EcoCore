use ecosystem::{
    canvas::{Canvas, Rgba},
    kitty::KittyImageId,
    renderer::{KittyRenderer, KittyRendererConfig},
    terminal::TerminalSize,
};

#[test]
fn kitty_renderer_places_first_frame_without_deleting_any_image() {
    let canvas = Canvas::new(2, 1, Rgba::rgb(255, 0, 0)).expect("valid canvas");
    let mut renderer = KittyRenderer::new(KittyRendererConfig {
        image_ids: [KittyImageId::new(100), KittyImageId::new(101)],
        image_columns: 30,
        image_rows: 10,
    });

    let frame = renderer.render_frame(TerminalSize::new(120, 40), &canvas);
    let command = String::from_utf8(frame.bytes).expect("renderer output is utf8");

    assert_eq!(frame.image_id, KittyImageId::new(100));
    assert_eq!(frame.deleted_image_id, None);
    assert_eq!(frame.placement.cursor_column, 46);
    assert!(command.starts_with("\u{1b}[16;46H\u{1b}_Ga=T,f=32,i=100"));
    assert!(!command.contains("a=d"));
}

#[test]
fn kitty_renderer_draws_next_buffer_before_deleting_previous_buffer() {
    let canvas = Canvas::new(2, 1, Rgba::rgb(255, 0, 0)).expect("valid canvas");
    let mut renderer = KittyRenderer::new(KittyRendererConfig {
        image_ids: [KittyImageId::new(100), KittyImageId::new(101)],
        image_columns: 30,
        image_rows: 10,
    });

    renderer.render_frame(TerminalSize::new(120, 40), &canvas);
    let frame = renderer.render_frame(TerminalSize::new(120, 40), &canvas);
    let command = String::from_utf8(frame.bytes).expect("renderer output is utf8");
    let draw_index = command.find("i=101").expect("new image is drawn");
    let delete_index = command.find("a=d").expect("previous image is deleted");

    assert_eq!(frame.image_id, KittyImageId::new(101));
    assert_eq!(frame.deleted_image_id, Some(KittyImageId::new(100)));
    assert!(draw_index < delete_index);
}

#[test]
fn kitty_renderer_cleanup_deletes_all_managed_buffers_and_resets_state() {
    let canvas = Canvas::new(2, 1, Rgba::rgb(255, 0, 0)).expect("valid canvas");
    let mut renderer = KittyRenderer::new(KittyRendererConfig {
        image_ids: [KittyImageId::new(100), KittyImageId::new(101)],
        image_columns: 30,
        image_rows: 10,
    });

    renderer.render_frame(TerminalSize::new(120, 40), &canvas);
    let cleanup = String::from_utf8(renderer.reset()).expect("cleanup output is utf8");
    let frame = renderer.render_frame(TerminalSize::new(120, 40), &canvas);

    assert!(cleanup.contains("a=d,d=i,i=100"));
    assert!(cleanup.contains("a=d,d=i,i=101"));
    assert_eq!(frame.image_id, KittyImageId::new(100));
    assert_eq!(frame.deleted_image_id, None);
}
