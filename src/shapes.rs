use crate::canvas::Layer;

type Color = [u8; 3];

pub fn draw_rect(
    layer: &mut Layer,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    c: Color,
    a: u8,
    outline_c: Color,
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
fn draw_ellipse_inside(
    cx: i32,
    cy: i32,
    rx: i32,
    ry: i32,
    layer: &mut Layer,
    color: Color,
    alpha: u8,
) {
    if rx <= 0 || ry <= 0 || alpha == 0 {
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

// draws the outside of an ellipse using scanlines.
fn draw_ellipse_outline(
    layer: &mut Layer,
    cx: i32,
    cy: i32,
    rx: i32,
    ry: i32,
    color: Color,
    alpha: u8,
    thickness: u32,
) {
    if rx <= 0 || ry <= 0 || thickness == 0 || alpha == 0 {
        return;
    }

    let out_rx = rx + thickness as i32;
    let out_ry = ry + thickness as i32;

    let mut outer_spans = vec![0i32; (out_ry + 1) as usize];
    let mut inner_spans = vec![-1i32; (out_ry + 1) as usize];

    fn calculate_spans(rx: i32, ry: i32, spans: &mut [i32]) {
        if rx <= 0 || ry <= 0 {
            return;
        }

        let a2 = rx * rx;
        let b2 = ry * ry;

        let mut x = 0;
        let mut y = ry;

        let mut px = 0;
        let mut py = 2 * a2 * y;

        let mut p = (b2 as f64 - (a2 * ry) as f64 + 0.25 * a2 as f64).round() as i32;

        while px < py {
            spans[y as usize] = spans[y as usize].max(x);

            x += 1;
            px += 2 * b2;

            if p < 0 {
                p += b2 + px;
            } else {
                y -= 1;
                py -= 2 * a2;
                p += b2 + px - py;
            }
        }

        spans[y as usize] = spans[y as usize].max(x);

        p = (b2 as f64 * (x as f64 + 0.5).powi(2) + a2 as f64 * (y as f64 - 1.0).powi(2)
            - (a2 * b2) as f64)
            .round() as i32;

        while y >= 0 {
            spans[y as usize] = spans[y as usize].max(x);

            y -= 1;
            py -= 2 * a2;

            if p > 0 {
                p += a2 - py;
            } else {
                x += 1;
                px += 2 * b2;
                p += a2 - py + px;
            }
        }
    }

    calculate_spans(out_rx, out_ry, &mut outer_spans);
    calculate_spans(rx, ry, &mut inner_spans);

    for y_offset in 0..=out_ry {
        let out_x = outer_spans[y_offset as usize];
        let in_x = inner_spans[y_offset as usize];

        let left_len = if in_x < 0 {
            2 * out_x + 1
        } else {
            out_x - in_x
        };

        let right_len = if in_x < 0 { 0 } else { out_x - in_x };

        draw_span(layer, cx - out_x, cy - y_offset, left_len, color, alpha);
        draw_span(layer, cx + in_x + 1, cy - y_offset, right_len, color, alpha);

        if y_offset > 0 {
            draw_span(layer, cx - out_x, cy + y_offset, left_len, color, alpha);
            draw_span(layer, cx + in_x + 1, cy + y_offset, right_len, color, alpha);
        }
    }
}

pub fn draw_ellipse(
    layer: &mut Layer,
    cx: i32,
    cy: i32,
    rx: i32,
    ry: i32,
    c: Color,
    a: u8,
    outline_c: Color,
    outline_a: u8,
    thickness: u32,
) {
    if rx <= 0 || ry <= 0 {
        return;
    }

    draw_ellipse_inside(cx, cy, rx, ry, layer, c, a); // cancels itself if alpha is zero
    draw_ellipse_outline(layer, cx, cy, rx, ry, outline_c, outline_a, thickness); // cancels itself if outline alpha is zero or thickness is zero
}
