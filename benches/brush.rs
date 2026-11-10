use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

use pcore::brush::Brush;
use pcore::canvas::Layer;

fn bench_brush_stamping(c: &mut Criterion) {
    let mut g = c.benchmark_group("brush/stroke_to_64px");
    for radius in [4.0f32, 16.0, 64.0] {
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

criterion_group!(
    benches,
    bench_brush_stamping,
    bench_brush_full_stroke,
    bench_brush_modes
);

criterion_main!(benches);
