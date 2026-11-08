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

// fast path for straight, horizontal lines.
// used for scanline rasterization functions like the ellipse function.
fn draw_span(layer: &mut Layer, x: i32, y: i32, l: i32, c: [u8; 3], a: u8) {
    if y < 0 || y >= layer.height() as i32 || l <= 0 {
        return;
    }

    let x0 = x.max(0);
    let x1 = (x + l).min(layer.width() as i32);

    for xx in x0..x1 {
        layer.blend_pixel(xx as u32, y as u32, c, a);
    }
}

// draws an ellipse using spans.
pub fn draw_ellipse_inside(
    cx: i32,
    cy: i32,
    rx: i32,
    ry: i32,
    layer: &mut Layer,
    color: [u8; 3],
    alpha: u8,
) {
    if rx <= 0 || ry <= 0 {
        return;
    }

    let mut x = 0;
    let mut y = ry;

    let rx2 = rx * rx;
    let ry2 = ry * ry;

    // region 1
    let mut dx = 2 * ry2 * x;
    let mut dy = 2 * rx2 * y;

    let mut p1 = (4 * ry2 - 4 * rx2 * ry + rx2) as f64;

    while dx < dy {
        draw_span(layer, cx - x, cy - y, 2 * x + 1, color, alpha);
        draw_span(layer, cx - x, cy + y, 2 * x + 1, color, alpha);

        x += 1;
        dx = 2 * ry2 * x;

        if p1 < 0.0 {
            p1 += (dx + ry2) as f64;
        } else {
            y -= 1;
            dy = 2 * rx2 * y;
            p1 += (dx - dy + ry2) as f64;
        }
    }

    // region 2
    let mut p2 = ry2 as f64 * (x as f64 + 0.5).powi(2) + rx2 as f64 * (y - 1).pow(2) as f64
        - (rx2 * ry2) as f64;

    while y >= 0 {
        draw_span(layer, cx - x, cy - y, 2 * x + 1, color, alpha);
        draw_span(layer, cx - x, cy + y, 2 * x + 1, color, alpha);

        y -= 1;
        dy = 2 * rx2 * y;

        if p2 > 0.0 {
            p2 += (rx2 - dy) as f64;
        } else {
            x += 1;
            dx = 2 * ry2 * x;
            p2 += (dx - dy + rx2) as f64;
        }
    }
}
