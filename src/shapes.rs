use crate::canvas::Layer;

type Color = [u8; 3];

struct Mask {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
}

impl Mask {
    #[inline]
    fn set_bit(&mut self, index: usize, value: bool) {
        let byte = index / 8;
        let bit = index % 8;

        if value {
            self.pixels[byte] |= 1 << bit;
        } else {
            self.pixels[byte] &= !(1 << bit);
        }
    }

    #[inline]
    fn get_bit(&self, index: usize) -> bool {
        let byte = index / 8;
        let bit = index % 8;

        (self.pixels[byte] & (1 << bit)) != 0
    }

    #[inline]
    pub fn set(&mut self, x: u32, y: u32, value: bool) {
        if x >= self.width || y >= self.height {
            return;
        }

        let index = (y * self.width + x) as usize;
        self.set_bit(index, value);
    }

    #[inline]
    pub fn get(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false; // anywhere outside the bounds has not been set and therefore would be false
        }

        let index = (y * self.width + x) as usize;
        self.get_bit(index)
    }
}

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
fn draw_span(layer: &mut Layer, x: u32, y: u32, l: u32, c: [u8; 3], a: u8) {
    if y >= layer.height() || l <= 0 {
        return;
    }

    let x0 = x.max(0);
    let x1 = (x + l).min(layer.width());

    for xx in x0..x1 {
        layer.blend_pixel(xx as u32, y as u32, c, a);
    }
}

// draws an ellipse using spans.
fn draw_ellipse_inside(cx: u32, cy: u32, rx: u32, ry: u32, layer: &mut Layer, c: Color, a: u8) {
    if rx <= 0 || ry <= 0 || a == 0 {
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
        draw_span(layer, cx - x, cy - y, 2 * x + 1, c, a);
        draw_span(layer, cx - x, cy + y, 2 * x + 1, c, a);

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

    while y != 0 {
        draw_span(layer, cx - x, cy - y, 2 * x + 1, c, a);
        draw_span(layer, cx - x, cy + y, 2 * x + 1, c, a);

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
    cx: u32,
    cy: u32,
    rx: u32,
    ry: u32,
    c: Color,
    a: u8,
    thickness: u32,
) {
    if rx == 0 || ry == 0 || thickness == 0 || a == 0 {
        return;
    }

    let Some(out_rx) = rx.checked_add(thickness) else {
        return;
    };
    let Some(out_ry) = ry.checked_add(thickness) else {
        return;
    };

    let Ok(span_count) = usize::try_from(out_ry)
        .ok()
        .and_then(|v| v.checked_add(1))
        .ok_or(())
    else {
        return;
    };

    let mut outer_spans = vec![0i64; span_count];
    let mut inner_spans = vec![-1i64; span_count];

    fn calculate_spans(rx: u32, ry: u32, spans: &mut [i64]) {
        let a = rx as i64;
        let b = ry as i64;

        let a2 = a * a;
        let b2 = b * b;

        let mut x = 0i64;
        let mut y = b;

        let mut px = 0i64;
        let mut py = 2 * a2 * y;

        let mut p = (b2 as f64 - (a2 * b) as f64 + 0.25 * a2 as f64).round();

        while px < py {
            spans[y as usize] = spans[y as usize].max(x);

            x += 1;
            px += 2 * b2;

            if p < 0.0 {
                p += (b2 + px) as f64;
            } else {
                y -= 1;
                py -= 2 * a2;
                p += (b2 + px - py) as f64;
            }
        }

        spans[y as usize] = spans[y as usize].max(x);

        p = (b2 as f64 * (x as f64 + 0.5).powi(2) + a2 as f64 * (y as f64 - 1.0).powi(2)
            - (a2 * b2) as f64)
            .round();

        while y >= 0 {
            spans[y as usize] = spans[y as usize].max(x);

            y -= 1;
            py -= 2 * a2;

            if p > 0.0 {
                p += (a2 - py) as f64;
            } else {
                x += 1;
                px += 2 * b2;
                p += (a2 - py + px) as f64;
            }
        }
    }

    calculate_spans(out_rx, out_ry, &mut outer_spans);
    calculate_spans(rx, ry, &mut inner_spans);

    let cx = cx as i64;
    let cy = cy as i64;

    for y in 0..=out_ry as usize {
        let out_x = outer_spans[y];
        let in_x = inner_spans[y];
        let y = y as i64;

        let left_len = if in_x < 0 {
            2 * out_x + 1
        } else {
            out_x - in_x
        };

        let right_len = if in_x < 0 { 0 } else { out_x - in_x };

        draw_span(
            layer,
            (cx - out_x) as u32,
            (cy - y) as u32,
            left_len as u32,
            c,
            a,
        );
        draw_span(
            layer,
            (cx + in_x + 1) as u32,
            (cy - y) as u32,
            right_len as u32,
            c,
            a,
        );

        if y > 0 {
            draw_span(
                layer,
                (cx - out_x) as u32,
                (cy + y) as u32,
                left_len as u32,
                c,
                a,
            );
            draw_span(
                layer,
                (cx + in_x + 1) as u32,
                (cy + y) as u32,
                right_len as u32,
                c,
                a,
            );
        }
    }
}

pub fn draw_ellipse(
    layer: &mut Layer,
    cx: u32,
    cy: u32,
    rx: u32,
    ry: u32,
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

fn circular_mask(width: u32, height: u32, cx: f32, cy: f32, radius: f32) -> Vec<bool> {
    // can be Vec<bool> because there won't be that many pixels. for larger masks I use bit-packed Vec<u8> because for some reason bool takes up a whole byte in rust?!
    let mut mask = vec![false; (width * height) as usize];

    let r_squared = radius * radius;

    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;

            // check distance using pythagoras
            if dx * dx + dy * dy <= r_squared {
                mask[(y * width + x) as usize] = true;
            }
        }
    }

    mask
}

// bresenham line
fn rasterize_line(x1: u32, y1: u32, x2: u32, y2: u32, mut callback: impl FnMut(u32, u32)) {
    let dx = x1.abs_diff(x2);
    let dy = y1.abs_diff(y2);

    let sx = x1 < x2;
    let sy = y1 < y2;

    let mut err = dx as i64 - dy as i64;
    let (mut x, mut y) = (x1, y1);

    loop {
        callback(x, y);

        if x == x2 && y == y2 {
            break;
        }

        let e2 = 2 * err;

        if e2 > -(dy as i64) {
            err -= dy as i64;
            x = if sx { x + 1 } else { x - 1 };
        }

        if e2 < dx as i64 {
            err += dx as i64;
            y = if sy { y + 1 } else { y - 1 };
        }
    }
}

// draws a line using bresenham's line algorithm and stamps in a stroke mask.
// inefficient but works for now.
pub fn draw_line(layer: &mut Layer, x1: u32, y1: u32, x2: u32, y2: u32, c: Color, a: u8, t: u32) {
    if a == 0 {
        return;
    }

    let circle_mask = circular_mask(t * 2, t * 2, t as f32, t as f32, t as f32);

    // fast path for dots
    if x1 == x2 && y1 == y2 {
        let mask_size = t * 2;

        for i in 0..circle_mask.len() {
            let mx = i as u32 % mask_size;
            let my = i as u32 / mask_size;

            if circle_mask[i] {
                // simply using saturating_add directly in the arguments can cause double blending here
                if let (Some(px), Some(py)) = (
                    x1.saturating_add(mx).checked_sub(t),
                    y1.saturating_add(my).checked_sub(t),
                ) {
                    layer.blend_pixel(px, py, c, a);
                }
            }
        }
    }

    let width = x1 - x2 + t; // add thickness because line thickness adds extra padding
    let height = y1 - y2 + t;

    let mut line_mask = Mask {
        pixels: vec![0u8; ((width * height) + 7) as usize / 8],
        width,
        height,
    };

    rasterize_line(x1, y1, x2, y2, |cx, cy| {
        let mask_size = t * 2;

        for i in 0..circle_mask.len() {
            let mx = i as u32 % mask_size;
            let my = i as u32 / mask_size;

            if circle_mask[i] {
                // can't cause double blending here though because a mask is used
                line_mask.set(mx.saturating_add(cx), my.saturating_add(cy), true);
            }
        }
    });

    for py in 0..height {
        for px in 0..width {
            if line_mask.get(px, py) {
                layer.blend_pixel(x1 + px, y1 + py, c, a);
            }
        }
    }
}
