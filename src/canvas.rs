use std::collections::HashMap;
use std::sync::Arc;

const TILE_SIZE: u32 = 256;

#[derive(Clone)]
struct Tile {
    pub pixels: Vec<u32>,

    pub w: u16,
    pub h: u16,
    pub x: u32,
    pub y: u32,
}

pub struct Layer {
    tiles: HashMap<(i32, i32), Arc<Tile>>,
    width: u32,
    height: u32,
}

impl Layer {
    pub fn new(height: u32, width: u32) -> Self {
        Self {
            tiles: HashMap::new(),
            width,
            height,
        }
    }

    #[inline(always)]
    pub fn set_pixel(&mut self, x: u32, y: u32, color: u32) {
        if x >= self.width || y >= self.height {
            return;
        }

        let tile_x = x / TILE_SIZE;
        let tile_y = y / TILE_SIZE;

        let local_x = x % TILE_SIZE;
        let local_y = y % TILE_SIZE;

        let tile = self.tiles.entry((tile_x as i32, tile_y as i32))
            .or_insert_with(|| {
                let w = (self.width - tile_x * TILE_SIZE)
                    .min(TILE_SIZE) as u16;

                let h = (self.height - tile_y * TILE_SIZE)
                    .min(TILE_SIZE) as u16;

                Arc::new(Tile {
                    pixels: vec![0; w as usize * h as usize],
                    w,
                    h,
                    x: tile_x * TILE_SIZE,
                    y: tile_y * TILE_SIZE,
                })
            });

        let tile = Arc::make_mut(tile);

        let index = (local_y * tile.w as u32 + local_x) as usize;

        tile.pixels[index] = color;
    }
}
