use crate::canvas::{Canvas, Rgba};

pub(super) fn draw_cafe_background(canvas: &mut Canvas) {
    draw_vertical_gradient(
        canvas,
        0,
        0,
        canvas.width(),
        canvas.height(),
        Rgba::rgb(16, 12, 24),
        Rgba::rgb(54, 31, 31),
    );
    draw_wall_depth(canvas);
    draw_warm_light(canvas);
    draw_rect(
        canvas,
        0,
        cafe_counter_top(canvas.height()),
        canvas.width(),
        canvas.height() - cafe_counter_top(canvas.height()),
        Rgba::rgb(85, 43, 30),
    );
    draw_rect(
        canvas,
        0,
        cafe_counter_top(canvas.height()),
        canvas.width(),
        8,
        Rgba::rgb(178, 101, 49),
    );
    draw_rect(
        canvas,
        0,
        cafe_counter_top(canvas.height()) + 8,
        canvas.width(),
        3,
        Rgba::rgb(48, 26, 26),
    );
    draw_window(canvas);
    draw_shelves(canvas);
    draw_counter_props(canvas);
}

pub(super) fn cafe_counter_top(height: u16) -> u16 {
    height.saturating_sub(height / 3)
}

pub(super) fn draw_rect(canvas: &mut Canvas, x: u16, y: u16, width: u16, height: u16, color: Rgba) {
    let max_y = y.saturating_add(height).min(canvas.height());
    let max_x = x.saturating_add(width).min(canvas.width());
    for py in y..max_y {
        for px in x..max_x {
            canvas.set_pixel(px, py, color).expect("rect is clipped");
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct WindowGeometry {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

pub(super) fn window_geometry(canvas_width: u16, canvas_height: u16) -> WindowGeometry {
    let width = canvas_width / 3;
    let height = canvas_height / 2;
    WindowGeometry {
        x: canvas_width - width - canvas_width / 12,
        y: canvas_height / 8,
        width,
        height,
    }
}

fn draw_wall_depth(canvas: &mut Canvas) {
    let counter_top = cafe_counter_top(canvas.height());
    draw_rect(canvas, 82, 0, 26, counter_top, Rgba::rgb(12, 9, 19));
    draw_rect(canvas, 108, 0, 4, counter_top, Rgba::rgb(58, 34, 31));
    draw_rect(
        canvas,
        0,
        counter_top.saturating_sub(18),
        canvas.width(),
        18,
        Rgba::rgb(39, 23, 27),
    );
    draw_rect(
        canvas,
        0,
        counter_top.saturating_sub(2),
        canvas.width(),
        2,
        Rgba::rgb(91, 50, 35),
    );
}

fn draw_warm_light(canvas: &mut Canvas) {
    let lamp_x = canvas.width() / 5;
    let lamp_y = canvas.height() / 5;
    draw_rect(
        canvas,
        lamp_x.saturating_sub(2),
        0,
        4,
        lamp_y.saturating_add(5),
        Rgba::rgb(58, 34, 31),
    );
    draw_rect(
        canvas,
        lamp_x.saturating_sub(18),
        lamp_y,
        36,
        11,
        Rgba::rgb(219, 149, 68),
    );
    draw_rect(
        canvas,
        lamp_x.saturating_sub(12),
        lamp_y + 11,
        24,
        7,
        Rgba::rgb(255, 190, 91),
    );

    // Cached blocky glow: readable from terminal distance without per-frame cost.
    draw_rect(
        canvas,
        lamp_x.saturating_sub(52),
        lamp_y + 18,
        104,
        24,
        Rgba::rgb(91, 52, 38),
    );
    draw_rect(
        canvas,
        lamp_x.saturating_sub(36),
        lamp_y + 18,
        72,
        18,
        Rgba::rgb(120, 72, 45),
    );
    draw_rect(
        canvas,
        lamp_x.saturating_sub(20),
        lamp_y + 18,
        40,
        12,
        Rgba::rgb(188, 123, 61),
    );
}

fn draw_window(canvas: &mut Canvas) {
    let WindowGeometry {
        x,
        y,
        width: window_width,
        height: window_height,
    } = window_geometry(canvas.width(), canvas.height());

    draw_rect(
        canvas,
        x.saturating_sub(4),
        y.saturating_sub(4),
        window_width + 8,
        window_height + 8,
        Rgba::rgb(93, 55, 40),
    );
    draw_vertical_gradient(
        canvas,
        x,
        y,
        window_width,
        window_height,
        Rgba::rgb(7, 22, 68),
        Rgba::rgb(13, 77, 128),
    );
    draw_moon(canvas, x + window_width / 4, y + window_height / 5);
    draw_rect(
        canvas,
        x + window_width / 2 - 2,
        y,
        4,
        window_height,
        Rgba::rgb(36, 27, 38),
    );
    draw_rect(
        canvas,
        x,
        y + window_height / 2 - 2,
        window_width,
        4,
        Rgba::rgb(36, 27, 38),
    );
    draw_rect(
        canvas,
        x + 8,
        y + window_height.saturating_sub(10),
        window_width.saturating_sub(16),
        3,
        Rgba::rgb(23, 87, 132),
    );
}

fn draw_moon(canvas: &mut Canvas, x: u16, y: u16) {
    draw_rect(canvas, x, y, 10, 10, Rgba::rgb(207, 211, 184));
    draw_rect(canvas, x + 6, y, 5, 10, Rgba::rgb(8, 31, 78));
}

fn draw_shelves(canvas: &mut Canvas) {
    let y = canvas.height() / 4;
    let width = canvas.width() / 3;
    draw_rect(canvas, 24, y, width, 6, Rgba::rgb(132, 73, 42));
    draw_rect(canvas, 24, y + 6, width, 3, Rgba::rgb(70, 38, 31));
    for cup in 0..5 {
        let x = 34 + cup * 22;
        draw_rect(
            canvas,
            x,
            y.saturating_sub(12),
            10,
            12,
            Rgba::rgb(218, 166, 91),
        );
        draw_rect(
            canvas,
            x + 2,
            y.saturating_sub(10),
            6,
            3,
            Rgba::rgb(246, 205, 126),
        );
    }
    draw_rect(
        canvas,
        42,
        y + 34,
        width.saturating_sub(20),
        5,
        Rgba::rgb(112, 61, 40),
    );
    for jar in 0..4 {
        let x = 52 + jar * 25;
        draw_rect(canvas, x, y + 18, 12, 16, Rgba::rgb(83, 61, 70));
        draw_rect(canvas, x + 2, y + 20, 8, 9, Rgba::rgb(197, 135, 80));
    }
}

fn draw_counter_props(canvas: &mut Canvas) {
    let y = cafe_counter_top(canvas.height()).saturating_add(14);
    let stage_width = canvas.width() / 5;
    let stage_x = canvas.width().saturating_sub(stage_width) / 2;
    draw_rect(
        canvas,
        stage_x,
        cafe_counter_top(canvas.height()).saturating_add(12),
        stage_width,
        10,
        Rgba::rgb(46, 28, 28),
    );
    draw_rect(
        canvas,
        stage_x.saturating_add(6),
        cafe_counter_top(canvas.height()).saturating_add(12),
        stage_width.saturating_sub(12),
        2,
        Rgba::rgb(121, 64, 40),
    );
    draw_rect(
        canvas,
        stage_x.saturating_add(stage_width.saturating_sub(28)),
        cafe_counter_top(canvas.height()).saturating_add(6),
        15,
        13,
        Rgba::rgb(119, 69, 43),
    );
    draw_rect(
        canvas,
        stage_x.saturating_add(stage_width.saturating_sub(25)),
        cafe_counter_top(canvas.height()).saturating_add(9),
        9,
        5,
        Rgba::rgb(218, 150, 78),
    );
    draw_rect(
        canvas,
        canvas.width() / 5,
        y,
        18,
        10,
        Rgba::rgb(204, 139, 74),
    );
    draw_rect(
        canvas,
        canvas.width() / 5 + 22,
        y.saturating_sub(8),
        8,
        18,
        Rgba::rgb(182, 112, 61),
    );
    draw_rect(
        canvas,
        canvas.width().saturating_sub(90),
        y,
        40,
        14,
        Rgba::rgb(45, 30, 32),
    );
    draw_rect(
        canvas,
        canvas.width().saturating_sub(78),
        y.saturating_sub(14),
        15,
        14,
        Rgba::rgb(108, 70, 52),
    );
    draw_rect(
        canvas,
        canvas.width().saturating_sub(73),
        y.saturating_sub(24),
        5,
        10,
        Rgba::rgb(214, 156, 88),
    );
    for panel in 0..5 {
        let x = 24 + panel * 72;
        draw_rect(canvas, x, y + 22, 48, 22, Rgba::rgb(103, 53, 33));
        draw_rect(canvas, x + 3, y + 25, 42, 3, Rgba::rgb(149, 82, 44));
    }
}

fn draw_vertical_gradient(
    canvas: &mut Canvas,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    top: Rgba,
    bottom: Rgba,
) {
    let max_y = y.saturating_add(height).min(canvas.height());
    let max_x = x.saturating_add(width).min(canvas.width());
    for py in y..max_y {
        let t = (py - y) as f32 / height.max(1) as f32;
        let color = lerp_color(top, bottom, t);
        for px in x..max_x {
            canvas
                .set_pixel(px, py, color)
                .expect("gradient is clipped");
        }
    }
}

fn lerp_color(start: Rgba, end: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    Rgba::rgb(
        lerp_channel(start.r, end.r, t),
        lerp_channel(start.g, end.g, t),
        lerp_channel(start.b, end.b, t),
    )
}

fn lerp_channel(start: u8, end: u8, t: f32) -> u8 {
    (f32::from(start) + (f32::from(end) - f32::from(start)) * t).round() as u8
}
