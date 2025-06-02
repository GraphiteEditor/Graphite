use criterion::{Criterion, black_box, criterion_group, criterion_main};
use glam::DVec2;
use path_bool::*;

pub fn criterion_benchmark(crit: &mut Criterion) {
	crit.bench_function("intersect 1", |bench| bench.iter(|| path_segment_intersection(black_box(&a()), black_box(&b()), true, &EPS)));
	crit.bench_function("intersect 2", |bench| bench.iter(|| path_segment_intersection(black_box(&c()), black_box(&d()), true, &EPS)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

fn a() -> PathSegment {
	PathSegment::Cubic(
		DVec2::new(458.37027, 572.165771),
		DVec2::new(428.525848, 486.720093),
		DVec2::new(368.618805, 467.485992),
		DVec2::new(273., 476.),
	)
}
fn b() -> PathSegment {
	PathSegment::Cubic(DVec2::new(273., 476.), DVec2::new(419., 463.), DVec2::new(481.741198, 514.692273), DVec2::new(481.333333, 768.))
}
fn c() -> PathSegment {
	PathSegment::Cubic(DVec2::new(273., 476.), DVec2::new(107.564178, 490.730591), DVec2::new(161.737915, 383.575775), DVec2::new(0., 340.))
}
fn d() -> PathSegment {
	PathSegment::Cubic(DVec2::new(0., 340.), DVec2::new(161.737914, 383.575765), DVec2::new(107.564182, 490.730587), DVec2::new(273., 476.))
}
