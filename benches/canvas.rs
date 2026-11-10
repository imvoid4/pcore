use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

use pcore::canvas::{Layer, UndoAction};

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

    // allocate the tile before benchmarking
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

fn touch_tiles(layer: &mut Layer, n: usize, value: u32) {
    for i in 0..n as u32 {
        layer.set_pixel((i % 16) * 256 + 5, (i / 16) * 256 + 5, value);
    }
}

fn paint_tiles(layer: &mut Layer, n: usize, value: u32) {
    for i in 0..n as u32 {
        let (ox, oy) = ((i % 16) * 256, (i / 16) * 256);
        layer.set_pixel(ox + 5, oy + 5, value);
        layer.set_pixel(ox + 255, oy + 255, value);
    }
}

fn bench_undo(c: &mut Criterion) {
    let mut g = c.benchmark_group("undo");

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

fn bench_resize(c: &mut Criterion) {
    let mut g = c.benchmark_group("resize");
    g.sample_size(20);

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
        ("shrink_aligned_2048", 2048, 2048),
        ("shrink_unaligned_2000", 2000, 2000),
        ("shrink_width_2000x4096", 2000, 4096),
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
    bench_undo,
    bench_repeated_blend_writes,
    bench_resize,
);

criterion_main!(benches);
