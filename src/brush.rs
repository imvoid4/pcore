use crate::canvas::{Layer, StrokeUndo};
use crate::util::mul255;

pub struct Brush {
    pub color: [u8; 3], // straight RGB
    pub flow: f32,      // 0..=1, opacity of each dab
    radius: f32,
    hardness: f32, // 0 = soft, 1 = hard
    spacing: f32,  // distance between dabs, as a fraction of diameter
    mask: Vec<u8>, // coverage 0..=255, size x size, centred
    size: usize,
    last: Option<(f32, f32)>,
    to_next: f32, // distance left until the next dab
}

impl Brush {
    pub fn new(radius: f32, hardness: f32) -> Self {
        let mut b = Self {
            color: [0, 0, 0],
            flow: 1.0,
            radius,
            hardness,
            spacing: 0.1,
            mask: Vec::new(),
            size: 0,
            last: None,
            to_next: 0.0,
        };
        b.rebuild_mask();
        b
    }

    pub fn set_size(&mut self, radius: f32, hardness: f32) {
        self.radius = radius;
        self.hardness = hardness;
        self.rebuild_mask(); // only here, never per dab
    }

    fn rebuild_mask(&mut self) {
        let r = self.radius.max(0.5);
        let c = r.ceil() as i32;
        let size = (2 * c + 1) as usize;
        let outer = r + 0.5;
        let inner = (r * self.hardness.clamp(0.0, 1.0))
            .min(outer - 1.0)
            .max(0.0);
        let mut mask = Vec::with_capacity(size * size);
        for j in 0..size {
            for i in 0..size {
                let dx = i as f32 - c as f32;
                let dy = j as f32 - c as f32;
                let dist = (dx * dx + dy * dy).sqrt();
                let t = ((outer - dist) / (outer - inner)).clamp(0.0, 1.0);
                let s = t * t * (3.0 - 2.0 * t); // smoothstep falloff
                mask.push((s * 255.0 + 0.5) as u8);
            }
        }
        self.mask = mask;
        self.size = size;
    }

    fn spacing_px(&self) -> f32 {
        (self.spacing * 2.0 * self.radius).max(0.5)
    }

    // pointer down.
    pub fn begin_stroke(&mut self, layer: &mut Layer, x: f32, y: f32) {
        layer.begin_stroke();
        self.stamp(layer, x.round() as i32, y.round() as i32);
        self.last = Some((x, y));
        self.to_next = self.spacing_px();
    }

    // pointer move: drop a dab every `spacing_px` along the path since the last point.
    pub fn stroke_to(&mut self, layer: &mut Layer, x: f32, y: f32) {
        let Some((lx, ly)) = self.last else { return };
        let (dx, dy) = (x - lx, y - ly);
        let len = (dx * dx + dy * dy).sqrt();
        if len == 0.0 {
            return;
        }
        let spacing = self.spacing_px();
        let mut d = self.to_next;
        while d <= len {
            let t = d / len;
            self.stamp(
                layer,
                (lx + dx * t).round() as i32,
                (ly + dy * t).round() as i32,
            );
            d += spacing;
        }
        self.to_next = d - len; // carry the remainder into the next segment
        self.last = Some((x, y));
    }

    // pointer up. Returns the undo record for the whole stroke.
    pub fn end_stroke(&mut self, layer: &mut Layer) -> StrokeUndo {
        self.last = None;
        layer.end_stroke()
    }

    // one dab centred on pixel (cx, cy): blend the colour through the coverage mask.
    fn stamp(&self, layer: &mut Layer, cx: i32, cy: i32) {
        let half = (self.size / 2) as i32;
        let pa = (self.flow.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
        let [r, g, b] = self.color.map(|v| v as u32);
        for j in 0..self.size {
            let y = cy - half + j as i32;
            if y < 0 {
                continue;
            }
            for i in 0..self.size {
                let cov = self.mask[j * self.size + i] as u32;
                if cov == 0 {
                    continue;
                }
                let x = cx - half + i as i32;
                if x < 0 {
                    continue;
                }

                let a = mul255(cov, pa);
                layer.blend_pixel(x as u32, y as u32, self.color, a as u8);
            }
        }
    }
}
