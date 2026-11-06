use crate::canvas::Layer;

pub fn draw_rect(layer: &mut Layer, x: u32, y: u32, w: u32, h: u32, c: [u8; 3], a: u8) {
    let x1 = x.saturating_add(w).min(layer.width());
    let y1 = y.saturating_add(h).min(layer.height());
    for yy in y..y1 {
        for xx in x..x1 {
            layer.blend_pixel(xx, yy, c, a);
        }
    }
}
