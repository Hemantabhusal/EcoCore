use ecosystem::{
    canvas::{Canvas, DirtyRegion, Rgba},
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
    assert!(command.starts_with("\u{1b}[16;46H\u{1b}_Ga=T,q=2,f=32,i=100"));
    assert!(!command.contains("a=d"));
}

#[test]
fn kitty_renderer_patches_dirty_region_without_deleting_visible_image() {
    let mut canvas = Canvas::new(10, 10, Rgba::rgb(255, 0, 0)).expect("valid canvas");
    let mut renderer = KittyRenderer::new(KittyRendererConfig {
        image_ids: [KittyImageId::new(100), KittyImageId::new(101)],
        image_columns: 30,
        image_rows: 10,
    });

    renderer.render_frame(TerminalSize::new(120, 40), &canvas);
    canvas
        .set_pixel(5, 4, Rgba::rgb(0, 255, 120))
        .expect("pixel in bounds");
    let frame = renderer.render_frame(TerminalSize::new(120, 40), &canvas);
    let command = String::from_utf8(frame.bytes).expect("renderer output is utf8");

    assert_eq!(frame.image_id, KittyImageId::new(100));
    assert_eq!(frame.deleted_image_id, None);
    assert!(command.starts_with("\u{1b}_Ga=f,q=2,f=32,i=100,r=1,x=5,y=4,s=1,v=1,X=1"));
    assert!(!command.contains("a=d"));
    assert!(frame.partial_update);
}

#[test]
fn kitty_renderer_patches_multiple_dirty_tiles_without_full_frame_fallback() {
    let mut canvas = Canvas::new(96, 64, Rgba::rgb(255, 0, 0)).expect("valid canvas");
    let mut renderer = KittyRenderer::new(KittyRendererConfig {
        image_ids: [KittyImageId::new(100), KittyImageId::new(101)],
        image_columns: 30,
        image_rows: 10,
    });

    renderer.render_frame(TerminalSize::new(120, 40), &canvas);
    canvas.clear_dirty();
    canvas
        .set_pixel(1, 1, Rgba::rgb(0, 255, 120))
        .expect("pixel in bounds");
    canvas
        .set_pixel(94, 62, Rgba::rgb(0, 120, 255))
        .expect("pixel in bounds");

    let frame = renderer.render_frame(TerminalSize::new(120, 40), &canvas);
    let command = String::from_utf8(frame.bytes).expect("renderer output is utf8");

    assert_eq!(frame.image_id, KittyImageId::new(100));
    assert_eq!(frame.deleted_image_id, None);
    assert!(frame.partial_update);
    assert_eq!(command.matches("\u{1b}_Ga=f").count(), 2);
    assert!(command.contains("x=0,y=0,s=16,v=16"));
    assert!(command.contains("x=80,y=48,s=16,v=16"));
    assert!(!command.contains("a=d"));
    assert!(!command.contains("a=T"));
}

#[test]
fn kitty_renderer_uses_next_buffer_for_near_full_dirty_region() {
    let mut canvas = Canvas::new(10, 10, Rgba::rgb(255, 0, 0)).expect("valid canvas");
    let mut renderer = KittyRenderer::new(KittyRendererConfig {
        image_ids: [KittyImageId::new(100), KittyImageId::new(101)],
        image_columns: 30,
        image_rows: 10,
    });

    renderer.render_frame(TerminalSize::new(120, 40), &canvas);
    canvas.clear_dirty();
    canvas.mark_dirty_region(DirtyRegion {
        x: 0,
        y: 0,
        width: 10,
        height: 10,
    });

    let frame = renderer.render_frame(TerminalSize::new(120, 40), &canvas);
    let command = String::from_utf8(frame.bytes).expect("renderer output is utf8");
    let draw_index = command.find("i=101").expect("next image is drawn");
    let delete_index = command.find("a=d").expect("previous image is deleted");

    assert_eq!(frame.image_id, KittyImageId::new(101));
    assert_eq!(frame.deleted_image_id, Some(KittyImageId::new(100)));
    assert!(!frame.partial_update);
    assert!(command.starts_with("\u{1b}[16;46H\u{1b}_Ga=T,q=2,f=32,i=101"));
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

    assert!(cleanup.contains("a=d,q=2,d=i,i=100"));
    assert!(cleanup.contains("a=d,q=2,d=i,i=101"));
    assert_eq!(frame.image_id, KittyImageId::new(100));
    assert_eq!(frame.deleted_image_id, None);
}

#[test]
fn kitty_renderer_tracks_frame_and_cleanup_protocol_stats() {
    let mut canvas = Canvas::new(10, 10, Rgba::rgb(255, 0, 0)).expect("valid canvas");
    let mut renderer = KittyRenderer::new(KittyRendererConfig {
        image_ids: [KittyImageId::new(100), KittyImageId::new(101)],
        image_columns: 30,
        image_rows: 10,
    });

    let first = renderer.render_frame(TerminalSize::new(120, 40), &canvas);
    canvas
        .set_pixel(5, 4, Rgba::rgb(0, 255, 120))
        .expect("pixel in bounds");
    let second = renderer.render_frame(TerminalSize::new(120, 40), &canvas);
    let frame_bytes = first.bytes.len() + second.bytes.len();
    let stats = renderer.stats();

    assert_eq!(stats.frames_rendered(), 2);
    assert_eq!(stats.frame_bytes(), frame_bytes as u64);
    assert_eq!(stats.full_frame_bytes(), first.bytes.len() as u64);
    assert_eq!(stats.partial_frame_bytes(), second.bytes.len() as u64);
    assert_eq!(stats.average_frame_bytes(), (frame_bytes / 2) as u64);
    assert_eq!(stats.latest_image_id(), Some(KittyImageId::new(100)));
    assert_eq!(stats.latest_deleted_image_id(), None);
    assert_eq!(stats.latest_placement(), Some(second.placement));

    let cleanup_bytes = renderer.reset().len();
    let stats = renderer.stats();

    assert_eq!(stats.resets(), 1);
    assert_eq!(stats.cleanup_bytes(), cleanup_bytes as u64);
    assert_eq!(
        stats.total_protocol_bytes(),
        (frame_bytes + cleanup_bytes) as u64
    );
    assert_eq!(stats.latest_image_id(), None);
    assert_eq!(stats.latest_deleted_image_id(), None);
    assert_eq!(stats.latest_placement(), None);
}
