use crate::canvas::Layer;

pub fn draw_rect(
    layer: &mut Layer,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    c: [u8; 3],
    a: u8,
    outline_c: [u8; 3],
    outline_a: u8,
    thickness: u32,
) {
    if a != 0 {
        // alpha will be zero if fill is disabled.
        let x1 = x.saturating_add(w).min(layer.width());
        let y1 = y.saturating_add(h).min(layer.height());
        for yy in y..y1 {
            for xx in x..x1 {
                layer.blend_pixel(xx, yy, c, a);
            }
        }
    }

    if thickness != 0 && outline_a != 0 {
        // thickness / outline alpha will be zero if outline is disabled.
        let x1 = x.saturating_add(w);
        let y1 = y.saturating_add(h);

        // top
        for xx in x.saturating_sub(thickness)..x1 {
            for yy in y.saturating_sub(thickness)..y {
                layer.blend_pixel(xx, yy, outline_c, outline_a);
            }
        }

        // bottom
        for xx in x.saturating_sub(thickness)..x1 {
            for yy in y1..y1.saturating_add(thickness) {
                layer.blend_pixel(xx, yy, outline_c, outline_a);
            }
        }

        // left
        for xx in x.saturating_sub(thickness)..x {
            for yy in y..y1 {
                layer.blend_pixel(xx, yy, outline_c, outline_a);
            }
        }

        // right
        for xx in x1..x1.saturating_add(thickness) {
            for yy in y..y1 {
                layer.blend_pixel(xx, yy, outline_c, outline_a);
            }
        }
    }
}
