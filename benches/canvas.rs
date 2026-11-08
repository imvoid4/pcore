use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

use pcore::brush::Brush;
use pcore::canvas::{Layer, UndoAction};
use pcore::shapes::draw_rect;

// ───────────────────────────── existing layer benches ─────────────────────────────

fn bench_layer_creation(c: &mut Criterion) {
    c.bench_function("canvas/create_4096", |b| {
        b.iter(|| {
            black_box(Layer::new(4096, 4096));
        });
    });
}

fn bench_first_pixel(c: &mut Criterion) {
    c.bench_function("canvas/first_pixel", |b| {
        b.iter_batched(
            || Layer::new(4096, 4096),
            |mut layer| {
                layer.set_pixel(black_box(100), black_box(100), black_box(0xFF0000FF));
                black_box(layer);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_repeated_writes(c: &mut Criterion) {
    let mut layer = Layer::new(4096, 4096);

    // Allocate the tile before benchmarking.
    layer.set_pixel(100, 100, 0xFF0000FF);

    c.bench_function("canvas/repeated_pixel_writes", |b| {
        b.iter(|| {
            for i in 0..10_000 {
                layer.set_pixel(
                    black_box(i % 256),
                    black_box(i / 256),
                    black_box(0xFF0000FF),
                );
            }
        });
    });
}

fn bench_canvas_paint(c: &mut Criterion) {
    c.bench_function("canvas/paint_4096", |b| {
        b.iter_batched(
            || Layer::new(4096, 4096),
            |mut layer| {
                for y in (0..4096).step_by(4) {
                    for x in (0..4096).step_by(4) {
                        layer.set_pixel(x, y, 0xFF0000FF);
                    }
                }

                black_box(layer);
            },
            BatchSize::SmallInput,
        );
    });
}

// ───────────────────────────────── helpers ────────────────────────────────────────

/// Write one pixel into each of the first `n` tiles (16 tiles per row on a 4096 canvas).
/// Allocates the tiles if they don't exist, and saves them if a stroke is open.
fn touch_tiles(layer: &mut Layer, n: usize, value: u32) {
    for i in 0..n as u32 {
        layer.set_pixel((i % 16) * 256 + 5, (i / 16) * 256 + 5, value);
    }
}

/// Like `touch_tiles`, but also writes the far corner (255, 255) of each tile, so
/// that shrinking to a non-tile-aligned size actually has pixels to clip.
fn paint_tiles(layer: &mut Layer, n: usize, value: u32) {
    for i in 0..n as u32 {
        let (ox, oy) = ((i % 16) * 256, (i / 16) * 256);
        layer.set_pixel(ox + 5, oy + 5, value);
        layer.set_pixel(ox + 255, oy + 255, value);
    }
}

// ───────────────────────────────── brush benches ──────────────────────────────────

/// Cost of the stamping + spacing logic alone. One stroke stays open for the whole
/// benchmark, so each tile's undo copy is paid once, outside the timed loop. Each
/// iteration drags the brush 64px, alternating direction.
fn bench_brush_stamping(c: &mut Criterion) {
    let mut g = c.benchmark_group("brush/stroke_to_64px");
    for radius in [4.0f32, 16.0, 64.0] {
        // Approximate dabs per 64px segment: spacing is 0.1 * diameter (min 0.5px).
        let dabs = (64.0 / (0.1 * 2.0 * radius).max(0.5)) as u64;
        g.throughput(Throughput::Elements(dabs));
        g.bench_with_input(
            BenchmarkId::new("radius", radius as u32),
            &radius,
            |b, &r| {
                let mut layer = Layer::new(4096, 4096);
                let mut brush = Brush::new(r, 0.5);
                brush.color = [200, 80, 40];
                brush.begin_stroke(&mut layer, 300.0, 300.0);
                let mut flip = false;
                b.iter(|| {
                    flip = !flip;
                    let x = if flip { 364.0 } else { 300.0 };
                    brush.stroke_to(&mut layer, black_box(x), 300.0);
                });
            },
        );
    }
    g.finish();
}

/// A whole realistic stroke: begin, 250 pointer moves along a wavy 1000px line, end.
/// Tiles already exist, so this includes the undo save (256 KiB copy per tile
/// crossed) and dropping the undo record, but not first-time tile allocation.
fn bench_brush_full_stroke(c: &mut Criterion) {
    let mut layer = Layer::new(4096, 4096);
    let mut brush = Brush::new(8.0, 0.5);
    brush.color = [30, 90, 200];
    for tx in 0..5 {
        layer.set_pixel(tx * 256 + 1, 300, 1); // pre-allocate the 5 tiles the stroke crosses
    }
    let _ = layer.end_stroke();

    c.bench_function("brush/full_stroke_1000px_r8", |b| {
        b.iter(|| {
            brush.begin_stroke(&mut layer, 100.0, 300.0);
            for i in 1..=250 {
                let x = 100.0 + i as f32 * 4.0;
                let y = 300.0 + (i as f32 * 0.1).sin() * 20.0;
                brush.stroke_to(&mut layer, black_box(x), y);
            }
            black_box(brush.end_stroke(&mut layer));
        });
    });
}

// ───────────────────────────────── undo benches ───────────────────────────────────

fn bench_undo(c: &mut Criterion) {
    let mut g = c.benchmark_group("undo");

    // The cold path: first write to an existing tile in a stroke copies its 256 KiB
    // (included: freeing the copy when the record is dropped).
    {
        let mut layer = Layer::new(4096, 4096);
        touch_tiles(&mut layer, 1, 1);
        let _ = layer.end_stroke();
        g.bench_function("save_one_tile", |b| {
            b.iter(|| {
                layer.begin_stroke();
                layer.set_pixel(10, 10, black_box(7));
                black_box(layer.end_stroke());
            });
        });
    }

    // Applying an undo record is just swapping pointers; check it scales with tile
    // count only. The inverse record is fed back in, so this ping-pongs undo/redo.
    for n in [4usize, 64, 256] {
        g.bench_with_input(BenchmarkId::new("apply_undo_tiles", n), &n, |b, &n| {
            let mut layer = Layer::new(4096, 4096);
            touch_tiles(&mut layer, n, 1); // create the tiles...
            let _ = layer.end_stroke();
            layer.begin_stroke();
            touch_tiles(&mut layer, n, 2); // ...then a stroke that overwrites them
            let mut record = Some(UndoAction::Stroke(layer.end_stroke()));
            b.iter(|| {
                record = Some(layer.apply_undo(record.take().unwrap()));
            });
        });
    }

    // Worst case: one stroke touching every tile of an already-painted 4096x4096
    // canvas = 64 MiB copied, then freed.
    {
        let mut layer = Layer::new(4096, 4096);
        touch_tiles(&mut layer, 256, 1);
        let _ = layer.end_stroke();
        g.sample_size(10);
        g.bench_function("save_all_256_tiles", |b| {
            b.iter(|| {
                layer.begin_stroke();
                touch_tiles(&mut layer, 256, black_box(3));
                black_box(layer.end_stroke());
            });
        });
    }

    g.finish();
}

// ─────────────────────── blend_pixel / rect / blending-brush benches ───────────────────────

/// blend_pixel's two paths: a == 255 skips reading the destination, a < 255 reads and
/// blends. Same shape as `canvas/repeated_pixel_writes`, so the numbers compare directly.
fn bench_repeated_blend_writes(c: &mut Criterion) {
    let mut g = c.benchmark_group("canvas/repeated_blend_writes");
    g.throughput(Throughput::Elements(10_000));
    for (name, a) in [("opaque", 255u8), ("translucent", 128u8)] {
        g.bench_function(name, |b| {
            let mut layer = Layer::new(4096, 4096);
            layer.set_pixel(100, 100, 0xFF0000FF); // allocate the tile
            b.iter(|| {
                for i in 0..10_000u32 {
                    layer.blend_pixel(black_box(i % 256), black_box(i / 256), [255, 0, 0], a);
                }
            });
        });
    }
    g.finish();
}

/// draw_rect cost per pixel. A stroke is left open for the whole benchmark, so each
/// tile's undo copy is paid once, before timing: this is the per-pixel work only.
/// Throughput is pixels, so criterion prints Melem/s (1000 Melem/s = 1 ns/pixel).
fn bench_rect(c: &mut Criterion) {
    let mut g = c.benchmark_group("rect");
    for (size, samples) in [(64u32, 100usize), (512, 50), (4096, 10)] {
        g.sample_size(samples);
        g.throughput(Throughput::Elements(size as u64 * size as u64));
        for (name, alpha) in [("opaque", 255u8), ("blend50", 128u8)] {
            g.bench_with_input(BenchmarkId::new(name, size), &size, |b, &size| {
                let mut layer = Layer::new(4096, 4096);
                layer.begin_stroke();
                draw_rect(
                    &mut layer,
                    0,
                    0,
                    size,
                    size,
                    [200, 80, 40],
                    255,
                    [255, 0, 0],
                    255,
                    10,
                ); // allocate + save tiles
                b.iter(|| {
                    draw_rect(
                        &mut layer,
                        0,
                        0,
                        black_box(size),
                        size,
                        [30, 90, 200],
                        alpha,
                        [255, 0, 0],
                        alpha,
                        10,
                    )
                });
            });
        }
    }
    g.finish();
}

/// Realistic tool use: begin, draw, end, drop the undo record. Tiles already exist, so
/// this includes the undo save (a 256 KiB copy per tile touched) on top of the drawing.
fn bench_rect_with_undo(c: &mut Criterion) {
    let mut g = c.benchmark_group("rect_with_undo");
    for size in [64u32, 512, 1024] {
        g.throughput(Throughput::Elements(size as u64 * size as u64));
        g.bench_with_input(BenchmarkId::new("blend50", size), &size, |b, &size| {
            let mut layer = Layer::new(4096, 4096);
            layer.begin_stroke();
            draw_rect(
                &mut layer,
                0,
                0,
                size,
                size,
                [200, 80, 40],
                255,
                [10, 10, 10],
                255,
                10,
            ); // allocate tiles
            let _ = layer.end_stroke();
            b.iter(|| {
                layer.begin_stroke();
                draw_rect(
                    &mut layer,
                    0,
                    0,
                    size,
                    size,
                    [30, 90, 200],
                    128,
                    [10, 10, 10],
                    255,
                    10,
                );
                black_box(layer.end_stroke());
            });
        });
    }
    g.finish();
}

/// Which blend path the brush mostly hits (same 64px drag as brush/stroke_to_64px):
///   hard_opaque       a == 255 almost everywhere  -> opaque fast path
///   soft_opaque       opaque core, blended falloff
///   hard_translucent  flow 0.5, every pixel blends
///   soft_translucent  flow 0.5 and falloff: the "airbrush" worst case
fn bench_brush_modes(c: &mut Criterion) {
    let mut g = c.benchmark_group("brush_mode");
    for radius in [16.0f32, 64.0] {
        let dabs = (64.0 / (0.1 * 2.0 * radius).max(0.5)) as u64;
        g.throughput(Throughput::Elements(dabs));
        for (name, hardness, flow) in [
            ("hard_opaque", 1.0f32, 1.0f32),
            ("soft_opaque", 0.0, 1.0),
            ("hard_translucent", 1.0, 0.5),
            ("soft_translucent", 0.0, 0.5),
        ] {
            g.bench_with_input(BenchmarkId::new(name, radius as u32), &radius, |b, &r| {
                let mut layer = Layer::new(4096, 4096);
                let mut brush = Brush::new(r, hardness);
                brush.flow = flow;
                brush.color = [200, 80, 40];
                brush.begin_stroke(&mut layer, 300.0, 300.0);
                let mut flip = false;
                b.iter(|| {
                    flip = !flip;
                    let x = if flip { 364.0 } else { 300.0 };
                    brush.stroke_to(&mut layer, black_box(x), 300.0);
                });
            });
        }
    }
    g.finish();
}

fn bench_resize(c: &mut Criterion) {
    let mut g = c.benchmark_group("resize");
    g.sample_size(20);

    // Nothing to move: pure grid rebuild cost.
    g.bench_function("noop_same_size", |b| {
        let mut layer = Layer::new(4096, 4096);
        b.iter(|| black_box(layer.resize(4096, 4096)));
    });

    g.bench_function("empty_grow_8192_roundtrip", |b| {
        let mut layer = Layer::new(4096, 4096);
        b.iter(|| {
            let undo = layer.resize(8192, 8192);
            black_box(layer.apply_undo(undo));
        });
    });

    // Fully painted 4096x4096 (256 tiles, 64 MiB).
    let cases: [(&str, u32, u32); 4] = [
        // drops whole tiles, no edge clipping needed
        ("shrink_aligned_2048", 2048, 2048),
        // drops tiles AND clips the new edge tiles (copies them for the patch)
        ("shrink_unaligned_2000", 2000, 2000),
        // width only: clips the last column, keeps every row
        ("shrink_width_2000x4096", 2000, 4096),
        // existing tiles just move; the patch should be empty
        ("grow_8192", 8192, 8192),
    ];

    for (name, w, h) in cases {
        g.bench_function(format!("painted_{name}_roundtrip"), |b| {
            let mut layer = Layer::new(4096, 4096);
            paint_tiles(&mut layer, 256, 1);
            let _ = layer.end_stroke();
            b.iter(|| {
                let undo = layer.resize(black_box(w), black_box(h));
                black_box(layer.apply_undo(undo));
            });
        });

        g.bench_function(format!("painted_{name}_undo_only"), |b| {
            let mut layer = Layer::new(4096, 4096);
            paint_tiles(&mut layer, 256, 1);
            let _ = layer.end_stroke();
            let mut record = Some(layer.resize(w, h));
            b.iter(|| {
                record = Some(layer.apply_undo(record.take().unwrap()));
            });
        });
    }

    g.finish();
}

criterion_group!(
    benches,
    bench_layer_creation,
    bench_first_pixel,
    bench_repeated_writes,
    bench_canvas_paint,
    bench_brush_stamping,
    bench_brush_full_stroke,
    bench_undo,
    bench_repeated_blend_writes,
    bench_rect,
    bench_rect_with_undo,
    bench_brush_modes,
    bench_resize,
);

criterion_main!(benches);
