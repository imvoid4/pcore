use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

use pcore::canvas::Layer;
use pcore::shapes::{draw_ellipse, draw_rect};

// time simply to make a rect in theory (no undo etc)
fn bench_rect(c: &mut Criterion) {
    let mut g = c.benchmark_group("rect");
    for (size, samples) in [(64u32, 100usize), (512, 50), (4096, 10)] {
        g.sample_size(samples);
        g.throughput(Throughput::Elements(size as u64 * size as u64));
        for (name, fill_a, line_a) in [
            ("opaque", 255u8, 255u8),
            ("blend50", 128, 128),
            ("zero_fill", 0, 255),
            ("zero_outline", 255, 0),
            ("zero_both", 0, 0),
        ] {
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
                        fill_a,
                        [255, 0, 0],
                        line_a,
                        10,
                    )
                });
            });
        }
    }
    g.finish();
}

// time simply to make an ellipse in theory (no undo etc)
fn bench_ellipse(c: &mut Criterion) {
    let mut g = c.benchmark_group("ellipse");
    for (size, samples) in [(64u32, 100usize), (512, 50), (4096, 10)] {
        g.sample_size(samples);
        g.throughput(Throughput::Elements(size as u64 * size as u64));
        for (name, fill_a, line_a) in [
            ("opaque", 255u8, 255u8),
            ("blend50", 128, 128),
            ("zero_fill", 0, 255),
            ("zero_outline", 255, 0),
            ("zero_both", 0, 0),
        ] {
            g.bench_with_input(BenchmarkId::new(name, size), &size, |b, &size| {
                let mut layer = Layer::new(4096, 4096);
                layer.begin_stroke();
                draw_ellipse(
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
                    draw_ellipse(
                        &mut layer,
                        0,
                        0,
                        black_box(size),
                        size,
                        [30, 90, 200],
                        fill_a,
                        [255, 0, 0],
                        line_a,
                        10,
                    )
                });
            });
        }
    }
    g.finish();
}

// realistic case
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

// realistic case
fn bench_ellipse_with_undo(c: &mut Criterion) {
    let mut g = c.benchmark_group("rect_with_undo");
    for size in [64u32, 512, 1024] {
        g.throughput(Throughput::Elements(size as u64 * size as u64));
        g.bench_with_input(BenchmarkId::new("blend50", size), &size, |b, &size| {
            let mut layer = Layer::new(4096, 4096);
            layer.begin_stroke();
            draw_ellipse(
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
                draw_ellipse(
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

criterion_group!(
    benches,
    bench_ellipse,
    bench_ellipse_with_undo,
    bench_rect,
    bench_rect_with_undo
);

criterion_main!(benches);
