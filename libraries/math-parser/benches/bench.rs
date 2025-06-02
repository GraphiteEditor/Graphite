use criterion::{Criterion, black_box, criterion_group, criterion_main};
use math_parser::ast;
use math_parser::context::EvalContext;

macro_rules! generate_benchmarks {
	($( $input:expr_2021 ),* $(,)?) => {
		fn parsing_bench(c: &mut Criterion) {
			$(
				c.bench_function(concat!("parse ", $input), |b| {
					b.iter(|| {
						let _ = black_box(ast::Node::try_parse_from_str($input)).unwrap();
					});
				});
			)*
		}

		fn evaluation_bench(c: &mut Criterion) {
			$(
				let expr = ast::Node::try_parse_from_str($input).unwrap().0;
				let context = EvalContext::default();

				c.bench_function(concat!("eval ", $input), |b| {
					b.iter(|| {
						let _ = black_box(expr.eval(&context));
					});
				});
			)*
		}

		criterion_group!(benches, parsing_bench, evaluation_bench);
		criterion_main!(benches);
	};
}

generate_benchmarks! {
	"(3 * (4 + sqrt(25)) - cos(pi/3) * (2^3)) + 5 * e", // Mixed nested functions, constants, and operations
	"((5 + 2 * (3 - sqrt(49)))^2) / (1 + sqrt(16)) + tau / 2", // Complex nested expression with constants
	"log(100, 10) + (5 * sin(pi/4) + sqrt(81)) / (2 * phi)", // Logarithmic and trigonometric functions
	"(sqrt(144) * 2 + 5) / (3 * (4 - sin(pi / 6))) + e^2", // Combined square root, trigonometric, and exponential operations
	"cos(2 * pi) + tan(pi / 3) * log(32, 2) - sqrt(256)", // Multiple trigonometric and logarithmic functions
	"(10 * (3 + 2) - 8 / 2)^2 + 7 * (2^4) - sqrt(225) + phi", // Mixed arithmetic with constants
	"(5^2 + 3^3) * (sqrt(81) + sqrt(64)) - tau * log(1000, 10)", // Power and square root with constants
	"((8 * sqrt(49) - 2 * e) + log(256, 2) / (2 + cos(pi))) * 1.5", // Nested functions and constants
	"(tan(pi / 4) + 5) * (3 + sqrt(36)) / (log(1024, 2) - 4)", // Nested functions with trigonometry and logarithm
	"((3 * e + 2 * sqrt(100)) - cos(tau / 4)) * log(27, 3) + phi", // Mixed constant usage and functions
	"(sqrt(100) + 5 * sin(pi / 6) - 8 / log(64, 2)) + e^(1.5)", // Complex mix of square root, division, and exponentiation
	"((sin(pi/2) + cos(0)) * (e^2 - 2 * sqrt(16))) / (log(100, 10) + pi)", // Nested trigonometric, exponential, and logarithmic functions
	"(5 * (7 + sqrt(121)) - (log(243, 3) * phi)) + 3^5 / tau", //
}
