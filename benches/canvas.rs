use criterion::{
    black_box, criterion_group, criterion_main,
    BatchSize, Criterion,
};

use pcore::canvas::Layer;

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
                layer.set_pixel(
                    black_box(100),
                    black_box(100),
                    black_box(0xFF0000FF),
                );
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

criterion_group!(
    benches,
    bench_layer_creation,
    bench_first_pixel,
    bench_repeated_writes,
    bench_canvas_paint,
);

criterion_main!(benches);
