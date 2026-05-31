use crate::framebuffer::{Cell, Color, Framebuffer, FramebufferError};

const SKY: Color = Color::rgb(8, 18, 34);
const GROUND: Color = Color::rgb(35, 50, 35);
const WATER: Color = Color::rgb(35, 120, 210);
const CREATURE: Color = Color::rgb(255, 180, 80);

pub fn build_static_landscape_frame(
    width: u16,
    height: u16,
) -> Result<Framebuffer, FramebufferError> {
    let mut frame = Framebuffer::new(
        width,
        height,
        Cell::new(' ', Color::rgb(120, 150, 190), SKY),
    )?;

    if height < 3 {
        return Ok(frame);
    }

    let ground_y = height - 1;
    let water_y = height - 2;
    let creature_y = height / 2;
    let center_x = width / 2;

    for x in 0..width {
        frame.set(x, ground_y, Cell::new('.', Color::rgb(90, 150, 85), GROUND))?;

        let water_glyph = if x % 4 == 0 { '>' } else { '~' };
        frame.set(x, water_y, Cell::new(water_glyph, WATER, SKY))?;
    }

    // The first MVP frame keeps one creature visible before live CPU data exists.
    frame.set(center_x, creature_y, Cell::new('o', CREATURE, SKY))?;

    Ok(frame)
}
