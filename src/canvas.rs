// tiled layer where `set_pixel` is the only hot-path primitive.
//
// undo is built into the write path: the first time a stroke touches a tile, that
// tile's previous contents are saved (once). after that, writes to the tile are a
// plain store, with no hashing, no atomics and no per-pixel undo bookkeeping.

use crate::util::mul255;

pub const TILE_SHIFT: u32 = 8;
pub const TILE_SIZE: u32 = 1 << TILE_SHIFT;
const TILE_MASK: u32 = TILE_SIZE - 1;
const TILE_PIXELS: usize = 1 << (2 * TILE_SHIFT);

// a boxed ARRAY (not a slice) so the compiler can prove
// `(local_y << 8 | local_x)` is < 65536 and drop the per-pixel bounds check.
type Tile = [u32; TILE_PIXELS];

fn new_tile() -> Box<Tile> {
    // vec -> boxed slice -> boxed array: no 256 KiB stack temporary, and the zeroed
    // allocation is lazily mapped by the OS.
    vec![0u32; TILE_PIXELS]
        .into_boxed_slice()
        .try_into()
        .unwrap()
}

fn copy_tile(t: &Tile) -> Box<Tile> {
    t.to_vec().into_boxed_slice().try_into().unwrap()
}

#[derive(Clone)]
struct Slot {
    // ID of the last stroke that saved this tile's previous contents.
    saved_in: u32,
    tile: Option<Box<Tile>>,
}

// previous contents of every tile a stroke touched (`None` = tile did not exist).
// applying it swaps the contents back and returns the inverse, so the same call
// does both undo and redo.
pub struct StrokeUndo {
    tiles: Vec<(u32, Option<Box<Tile>>)>,
}

pub struct CanvasUndo {
    width: u32,
    height: u32,
    patch: Vec<(u32, u32, Option<Box<Tile>>)>,
}

// zero everything outside keep_w x keep_h; returns whether anything changed
fn clip_tile(t: &mut Tile, keep_w: u32, keep_h: u32) -> bool {
    let mut changed = false;
    for row in 0..TILE_SIZE {
        let start = (row << TILE_SHIFT) as usize;
        let line = &mut t[start..start + TILE_SIZE as usize];
        let from = if row >= keep_h { 0 } else { keep_w as usize };
        let tail = &mut line[from..];
        if tail.iter().any(|&p| p != 0) {
            tail.fill(0);
            changed = true;
        }
    }
    changed
}

pub enum UndoAction {
    Stroke(StrokeUndo),
    Canvas(CanvasUndo),
}

impl StrokeUndo {
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }
}

#[derive(Clone)]
pub struct Layer {
    slots: Vec<Slot>, // dense grid, row-major: idx = tile_y * cols + tile_x
    cols: u32,
    width: u32,
    height: u32,
    stroke: u32,
    undo: Vec<(u32, Option<Box<Tile>>)>,
}

impl Layer {
    // (width, height), the conventional order.
    pub fn new(width: u32, height: u32) -> Self {
        let cols = width.div_ceil(TILE_SIZE);
        let rows = height.div_ceil(TILE_SIZE);
        Self {
            slots: (0..cols * rows)
                .map(|_| Slot {
                    saved_in: 0,
                    tile: None,
                })
                .collect(),
            cols,
            width,
            height,
            stroke: 1,
            undo: Vec::new(),
        }
    }

    pub fn begin_stroke(&mut self) {
        self.stroke += 1;
    }

    // Finish the stroke and take its undo record.
    pub fn end_stroke(&mut self) -> StrokeUndo {
        StrokeUndo {
            tiles: std::mem::take(&mut self.undo),
        }
    }

    pub fn apply_undo(&mut self, action: UndoAction) -> UndoAction {
        self.stroke += 1; // invalidate current stroke saves

        match action {
            UndoAction::Stroke(s) => {
                let mut inverse = Vec::with_capacity(s.tiles.len());
                for (idx, mut t) in s.tiles {
                    std::mem::swap(&mut self.slots[idx as usize].tile, &mut t);
                    inverse.push((idx, t));
                }
                UndoAction::Stroke(StrokeUndo { tiles: inverse })
            }
            UndoAction::Canvas(c) => {
                UndoAction::Canvas(self.reshape(c.width, c.height, c.patch, false))
            }
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, color: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = ((y >> TILE_SHIFT) * self.cols + (x >> TILE_SHIFT)) as usize;
        if self.slots[idx].saved_in != self.stroke {
            self.save_tile(idx); // once per tile per stroke; also allocates if empty
        }
        if let Some(t) = &mut self.slots[idx].tile {
            t[(((y & TILE_MASK) << TILE_SHIFT) | (x & TILE_MASK)) as usize] = color;
        }
    }

    #[inline]
    pub fn blend_pixel(&mut self, x: u32, y: u32, rgb: [u8; 3], a: u8) {
        if a == 0 || x >= self.width || y >= self.height {
            return;
        }
        let idx = ((y >> TILE_SHIFT) * self.cols + (x >> TILE_SHIFT)) as usize;
        if self.slots[idx].saved_in != self.stroke {
            self.save_tile(idx);
        }
        if let Some(t) = &mut self.slots[idx].tile {
            let px = &mut t[(((y & TILE_MASK) << TILE_SHIFT) | (x & TILE_MASK)) as usize];
            if a == 255 {
                // Opaque: no need to read the destination.
                *px = u32::from_le_bytes([rgb[0], rgb[1], rgb[2], 255]);
                return;
            }
            let d = px.to_le_bytes();
            let (a, inv) = (a as u32, 255 - a as u32);
            *px = u32::from_le_bytes([
                (mul255(rgb[0] as u32, a) + mul255(d[0] as u32, inv)) as u8,
                (mul255(rgb[1] as u32, a) + mul255(d[1] as u32, inv)) as u8,
                (mul255(rgb[2] as u32, a) + mul255(d[2] as u32, inv)) as u8,
                (a + mul255(d[3] as u32, inv)) as u8,
            ]);
        }
    }

    //------------ stays out of the hot loop
    #[cold]
    #[inline(never)]
    //------------
    fn save_tile(&mut self, idx: usize) {
        let slot = &mut self.slots[idx];
        let before = slot.tile.as_deref().map(copy_tile);
        if slot.tile.is_none() {
            slot.tile = Some(new_tile());
        }
        slot.saved_in = self.stroke;
        self.undo.push((idx as u32, before));
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> u32 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        let idx = ((y >> TILE_SHIFT) * self.cols + (x >> TILE_SHIFT)) as usize;
        match &self.slots[idx].tile {
            Some(t) => t[(((y & TILE_MASK) << TILE_SHIFT) | (x & TILE_MASK)) as usize],
            None => 0,
        }
    }

    fn reshape(
        &mut self,
        new_w: u32,
        new_h: u32,
        patch: Vec<(u32, u32, Option<Box<Tile>>)>,
        clip_edges: bool,
    ) -> CanvasUndo {
        let new_cols = new_w.div_ceil(TILE_SIZE);
        let new_rows = new_h.div_ceil(TILE_SIZE);
        let old_cols = self.cols;

        let mut new_slots: Vec<Slot> = (0..new_cols * new_rows)
            .map(|_| Slot {
                saved_in: 0,
                tile: None,
            })
            .collect();
        let mut inverse = Vec::new();

        // 1. move tiles into the new grid (no copies); dropped ones go to the inverse
        for (i, slot) in std::mem::take(&mut self.slots).into_iter().enumerate() {
            let Some(tile) = slot.tile else { continue };
            let (tx, ty) = (i as u32 % old_cols, i as u32 / old_cols);
            if tx < new_cols && ty < new_rows {
                new_slots[(ty * new_cols + tx) as usize].tile = Some(tile);
            } else {
                inverse.push((tx, ty, Some(tile)));
            }
        }

        // 2. on shrink, clip edge tiles; keep a copy only if clipping changed them
        if clip_edges && (new_w < self.width || new_h < self.height) {
            for ty in 0..new_rows {
                for tx in 0..new_cols {
                    let keep_w = (new_w - tx * TILE_SIZE).min(TILE_SIZE);
                    let keep_h = (new_h - ty * TILE_SIZE).min(TILE_SIZE);
                    if keep_w == TILE_SIZE && keep_h == TILE_SIZE {
                        continue;
                    }
                    if let Some(t) = &mut new_slots[(ty * new_cols + tx) as usize].tile {
                        let before = copy_tile(t);
                        if clip_tile(t, keep_w, keep_h) {
                            inverse.push((tx, ty, Some(before)));
                        }
                    }
                }
            }
        }

        // 3. apply the restore patch, recording whatever it displaces
        for (tx, ty, tile) in patch {
            if tx < new_cols && ty < new_rows {
                let slot = &mut new_slots[(ty * new_cols + tx) as usize];
                let displaced = std::mem::replace(&mut slot.tile, tile);
                inverse.push((tx, ty, displaced));
            }
        }

        let undo = CanvasUndo {
            width: self.width,
            height: self.height,
            patch: inverse,
        };

        self.slots = new_slots;
        self.cols = new_cols;
        self.width = new_w;
        self.height = new_h;
        self.undo.clear(); // grid geometry changed
        undo
    }

    pub fn resize(&mut self, new_width: u32, new_height: u32) -> UndoAction {
        if new_width == self.width && new_height == self.height {
            return UndoAction::Stroke(StrokeUndo { tiles: Vec::new() });
        }
        UndoAction::Canvas(self.reshape(new_width, new_height, Vec::new(), true))
    }

    //getters
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}
