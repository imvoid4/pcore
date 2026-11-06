use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

use pcore::brush::Brush;
use pcore::canvas::Layer;

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
            let mut record = Some(layer.end_stroke());
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

criterion_group!(
    benches,
    bench_layer_creation,
    bench_first_pixel,
    bench_repeated_writes,
    bench_canvas_paint,
    bench_brush_stamping,
    bench_brush_full_stroke,
    bench_undo,
);

criterion_main!(benches);
