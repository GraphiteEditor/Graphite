use criterion::{black_box, criterion_group, criterion_main, Criterion};
use path_bool::*;

pub fn criterion_benchmark(c: &mut Criterion) {
	let path_a = path_from_path_data("M0,340C161.737914,383.575765 107.564182,490.730587 273,476 C419,463 481.741198,514.692273 481.333333,768 C481.333333,768 -0,768 -0,768 C-0,768 0,340 0,340 Z ");
	let path_b = path_from_path_data(
			"M458.370270,572.165771C428.525848,486.720093 368.618805,467.485992 273,476 C107.564178,490.730591 161.737915,383.575775 0,340 C0,340 0,689 0,689 C56,700 106.513901,779.342590 188,694.666687 C306.607422,571.416260 372.033966,552.205139 458.370270,572.165771 Z",
		);
	c.bench_function("painted_dreams_diff", |b| {
		b.iter(|| path_boolean(black_box(&path_a), FillRule::NonZero, black_box(&path_b), FillRule::NonZero, PathBooleanOperation::Difference))
	});
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

/*
[libraries/path-bool/src/intersection_path_segment.rs:84:2] seg0 = Cubic(
	DVec2(
		458.37027,
		572.165771,
	),
	DVec2(
		428.525848,
		486.720093,
	),
	DVec2(
		368.618805,
		467.485992,
	),
	DVec2(
		273.0,
		476.0,
	),
)
[libraries/path-bool/src/intersection_path_segment.rs:84:2] seg1 = Cubic(
	DVec2(
		273.0,
		476.0,
	),
	DVec2(
		419.0,
		463.0,
	),
	DVec2(
		481.741198,
		514.692273,
	),
	DVec2(
		481.333333,
		768.0,
	),
)
[libraries/path-bool/src/intersection_path_segment.rs:84:2] seg0 = Cubic(
	DVec2(
		273.0,
		476.0,
	),
	DVec2(
		107.564178,
		490.730591,
	),
	DVec2(
		161.737915,
		383.575775,
	),
	DVec2(
		0.0,
		340.0,
	),
)
[libraries/path-bool/src/intersection_path_segment.rs:84:2] seg1 = Cubic(
	DVec2(
		0.0,
		340.0,
	),
	DVec2(
		161.737914,
		383.575765,
	),
	DVec2(
		107.564182,
		490.730587,
	),
	DVec2(
		273.0,
		476.0,
	),
)
*/
