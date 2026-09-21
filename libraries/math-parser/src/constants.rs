use crate::value::{Complex, Number, Value};
use num_complex::ComplexFloat;
use std::f64::consts::LN_2;

pub type BuiltinFunction = fn(&[Value]) -> Option<Value>;

/// The largest magnitude below which every integer is exactly representable in f64.
const EXACT_INTEGER_LIMIT: f64 = (1_u64 << f64::MANTISSA_DIGITS) as f64;

/// Truncates an operand to a nonnegative integer for `gcd`/`lcm`, or `None` when it is non-finite or beyond f64's exactly-representable integer range.
fn integer_operand(value: f64) -> Option<u128> {
	let value = value.trunc();
	(value.is_finite() && value.abs() <= EXACT_INTEGER_LIMIT).then(|| (value as i64).unsigned_abs() as u128)
}

/// Rounds a combinatorics operand to the nearest whole number, or `None` when it is negative, non-finite, or beyond f64's exactly-representable integer range.
fn whole_operand(value: f64) -> Option<u64> {
	let value = value.round();
	(0. ..=EXACT_INTEGER_LIMIT).contains(&value).then_some(value as u64)
}

/// Accumulates one multiplicative `step` per iteration, stopping once the running product reaches infinity, since it stays there.
/// That bounds the work to a few thousand steps for operands whose true result no f64 can hold.
fn bounded_product(steps: impl Iterator<Item = u64>, step: impl Fn(f64, u64) -> f64) -> f64 {
	let mut product = 1.;
	for index in steps {
		if !product.is_finite() {
			break;
		}
		product = step(product, index);
	}
	product
}

/// Collects every argument as a real number, or `None` if any is complex or there are no arguments at all.
fn real_operands(values: &[Value]) -> Option<Vec<f64>> {
	if values.is_empty() {
		return None;
	}

	values
		.iter()
		.map(|value| match value {
			Value::Number(Number::Real(real)) => Some(*real),
			_ => None,
		})
		.collect()
}

/// Applies a one-argument function that may climb into the complex plane: a real result that does not exist,
/// like `sqrt(-4)`, `ln(-1)`, or `asin(2)`, is recomputed as the function's principal complex value.
fn climbing(values: &[Value], real_function: fn(f64) -> f64, complex_function: fn(Complex) -> Complex) -> Option<Value> {
	match values {
		[Value::Number(Number::Real(real))] => {
			let result = real_function(*real);
			let result = if result.is_nan() {
				Value::from(complex_function(Complex::new(*real, 0.)))
			} else {
				Value::from_f64(result)
			};
			Some(result)
		}
		[Value::Number(Number::Complex(complex))] => Some(Value::from(complex_function(*complex))),
		_ => None,
	}
}

/// The power of two at or below the largest magnitude, dividing by which is exact and brings every value within ±2, so sums and
/// squares of the scaled values neither overflow nor underflow. It's 1 when the largest magnitude is zero, subnormal, or infinite.
fn power_of_two_scale(reals: &[f64]) -> f64 {
	let largest = reals.iter().fold(0_f64, |largest, real| largest.max(real.abs()));
	if !largest.is_normal() {
		return 1.;
	}

	// Keeping only the exponent bits zeroes the mantissa, leaving the power of two
	f64::from_bits(largest.to_bits() & (0x7FF << 52))
}

/// Computes the variance of the real arguments divided by the returned scale, over the count less `correction` (1 for a sample,
/// undefined for a single value, or 0 for a population). Staying scaled lets a standard deviation take its root before overflowing.
fn scaled_variance(values: &[Value], correction: usize) -> Option<(f64, f64)> {
	let reals = real_operands(values)?;
	let divisor = reals.len().checked_sub(correction).filter(|divisor| *divisor > 0)? as f64;
	let scale = power_of_two_scale(&reals);

	let mean = reals.iter().map(|real| real / scale).sum::<f64>() / reals.len() as f64;
	let variance = reals.iter().map(|real| (real / scale - mean).powi(2)).sum::<f64>() / divisor;
	Some((variance, scale))
}

/// Interpolates from `a` to `b` by `t` as `a + (b - a) t`, or as `a (1 - t) + b t` when finite endpoints are too far apart for `b - a` to fit.
fn lerp(a: f64, b: f64, t: f64) -> f64 {
	let difference = b - a;
	if difference.is_infinite() && a.is_finite() && b.is_finite() {
		return a * (1. - t) + b * t;
	}
	a + difference * t
}

/// The fraction of the way `value` lies from `a` to `b`, halving every operand first (which keeps the ratio) when a difference of finite ones would overflow.
fn inverse_lerp(value: f64, a: f64, b: f64) -> f64 {
	let (numerator, denominator) = (value - a, b - a);
	if (numerator.is_infinite() || denominator.is_infinite()) && [value, a, b].iter().all(|operand| operand.is_finite()) {
		return (value / 2. - a / 2.) / (b / 2. - a / 2.);
	}
	numerator / denominator
}

/// Computes the greatest common divisor of two nonnegative integers by the Euclidean algorithm.
pub fn gcd(a: u128, b: u128) -> u128 {
	let (mut a, mut b) = (a, b);
	// O(log min(a, b)) iterations, worst case 184 loops with the largest consecutive u128 Fibonacci numbers
	while b != 0 {
		(a, b) = (b, a % b);
	}
	a
}

/// Computes the least common multiple of two nonnegative integers. Operands within f64's exact integer range cannot overflow it.
pub fn lcm(a: u128, b: u128) -> u128 {
	checked_lcm(a, b).unwrap_or_default()
}

/// Computes the least common multiple of two nonnegative integers, or `None` when it exceeds integer storage.
fn checked_lcm(a: u128, b: u128) -> Option<u128> {
	if a == 0 || b == 0 {
		return Some(0);
	}
	(a / gcd(a, b)).checked_mul(b)
}

/// Resolves a base-suffixed function name like `log2` or `root3.25` into the corresponding two-argument
/// function and the baked-in second argument parsed from the suffix.
pub fn suffixed_function(name: &str) -> Option<(BuiltinFunction, f64)> {
	let (function, suffix) = ["log", "root"].into_iter().find_map(|prefix| Some((prefix, name.strip_prefix(prefix)?)))?;
	let suffix = suffix.strip_prefix('_').unwrap_or(suffix);

	// A base is written in plain decimal, leaving anything else, like the keyword-valued `loginf` or the scientific `log2e5`, to resolve as a variable or custom function
	if !suffix.starts_with(|c: char| c.is_ascii_digit()) || !suffix.chars().all(|c| c.is_ascii_digit() || c == '.') {
		return None;
	}
	let base = suffix.parse::<f64>().ok().filter(|base| base.is_finite())?;

	Some((builtin_function(function)?, base))
}

/// Looks up a built-in math function by name, returning a plain function pointer so dispatch avoids hashing and dynamic allocation.
pub fn builtin_function(name: &str) -> Option<BuiltinFunction> {
	Some(match name {
		// Trigonometric functions, with the inverses climbing into the complex plane outside the real domain (`asin(2)`)
		"sin" => |values| climbing(values, f64::sin, Complex::sin),
		"cos" => |values| climbing(values, f64::cos, Complex::cos),
		"tan" => |values| climbing(values, f64::tan, Complex::tan),
		"csc" => |values| climbing(values, |x| x.sin().recip(), |z| z.sin().recip()),
		"sec" => |values| climbing(values, |x| x.cos().recip(), |z| z.cos().recip()),
		"cot" => |values| climbing(values, |x| x.tan().recip(), |z| z.tan().recip()),

		// TODO: Offer the `arc-`/`ar-` spellings (`arcsin`, `artanh`) and the legacy `inv-` names as autocomplete aliases in the expression widget, resolving to these canonical names
		"asin" => |values| climbing(values, f64::asin, Complex::asin),
		"acos" => |values| climbing(values, f64::acos, Complex::acos),
		"atan" => |values| climbing(values, f64::atan, Complex::atan),
		"acsc" => |values| climbing(values, |x| x.recip().asin(), |z| z.recip().asin()),
		"asec" => |values| climbing(values, |x| x.recip().acos(), |z| z.recip().acos()),
		"acot" => |values| climbing(values, |x| x.recip().atan(), |z| z.recip().atan()),

		// Hyperbolic functions, with the inverses likewise climbing outside the real domain (`acosh(0.5)`, `atanh(2)`)
		"sinh" => |values| climbing(values, f64::sinh, Complex::sinh),
		"cosh" => |values| climbing(values, f64::cosh, Complex::cosh),
		"tanh" => |values| climbing(values, f64::tanh, Complex::tanh),
		"csch" => |values| climbing(values, |x| x.sinh().recip(), |z| z.sinh().recip()),
		"sech" => |values| climbing(values, |x| x.cosh().recip(), |z| z.cosh().recip()),
		"coth" => |values| climbing(values, |x| x.tanh().recip(), |z| z.tanh().recip()),
		"asinh" => |values| climbing(values, f64::asinh, Complex::asinh),
		"acosh" => |values| climbing(values, f64::acosh, Complex::acosh),
		"atanh" => |values| climbing(values, f64::atanh, Complex::atanh),
		"acsch" => |values| climbing(values, |x| x.recip().asinh(), |z| z.recip().asinh()),
		"asech" => |values| climbing(values, |x| x.recip().acosh(), |z| z.recip().acosh()),
		"acoth" => |values| climbing(values, |x| x.recip().atanh(), |z| z.recip().atanh()),

		// Logarithms, exponentials, and roots, climbing outside the real domain (`ln(-1)`, `sqrt(-4)`)
		"ln" => |values| climbing(values, f64::ln, Complex::ln),
		"exp" => |values| climbing(values, f64::exp, Complex::exp),
		"sqrt" => |values| climbing(values, f64::sqrt, Complex::sqrt),
		"cbrt" => |values| climbing(values, f64::cbrt, |z| z.powf(1. / 3.)),
		"log2" => |values| climbing(values, f64::log2, |z| z.ln() / LN_2),

		"log" => |values| match values {
			[value] => climbing(std::slice::from_ref(value), f64::log10, |z| z.log10()),
			// Change of base, staying real when it can and climbing into the complex plane when it cannot
			[Value::Number(Number::Real(x)), Value::Number(Number::Real(base))] => {
				let log = x.ln() / base.ln();
				Some(if log.is_nan() {
					Value::from(Complex::new(*x, 0.).ln() / Complex::new(*base, 0.).ln())
				} else {
					Value::from_f64(log)
				})
			}
			[Value::Number(x), Value::Number(base)] => Some(Value::from(x.as_complex().ln() / base.as_complex().ln())),
			_ => None,
		},

		"root" => |values| match values {
			[Value::Number(Number::Real(x)), Value::Number(Number::Real(n))] => {
				// An odd root of a negative real is real, where `powf` alone would climb to the principal complex root
				if *x < 0. && n.rem_euclid(2.) == 1. {
					return Some(Value::from_f64(-(-x).powf(1. / *n)));
				}
				let root = x.powf(1. / *n);
				Some(if root.is_nan() { Value::from(Complex::new(*x, 0.).powf(1. / *n)) } else { Value::from_f64(root) })
			}
			[Value::Number(Number::Complex(x)), Value::Number(Number::Real(n))] => Some(Value::from(x.powf(1. / *n))),
			_ => None,
		},

		// Geometry Functions
		// Folding pairwise hypotenuses gives the root of the sum of squares without ever squaring, avoiding overflow
		"hypot" => |values| Some(Value::from_f64(real_operands(values)?.into_iter().fold(0., f64::hypot))),

		"atan2" => |values| match values {
			[Value::Number(Number::Real(y)), Value::Number(Number::Real(x))] => Some(Value::Number(Number::Real(y.atan2(*x)))),
			_ => None,
		},

		// Mapping Functions
		// Each part's absolute value, where `|x|` is instead the one magnitude of the whole value
		"abs" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.abs()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::from(Complex::new(complex.re.abs(), complex.im.abs()))),
			_ => None,
		},

		"floor" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.floor()))),
			_ => None,
		},

		"ceil" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.ceil()))),
			_ => None,
		},

		"round" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.round()))),
			_ => None,
		},

		"clamp" => |values| match values {
			[Value::Number(Number::Real(x)), Value::Number(Number::Real(min)), Value::Number(Number::Real(max))] => Some(Value::Number(Number::Real(x.clamp(*min, *max)))),
			_ => None,
		},

		// Variadic across one or more real arguments
		"min" => |values| {
			let [Value::Number(Number::Real(first)), rest @ ..] = values else { return None };
			let mut min = *first;
			for value in rest {
				let Value::Number(Number::Real(real)) = value else { return None };
				min = min.min(*real);
			}
			Some(Value::Number(Number::Real(min)))
		},

		"max" => |values| {
			let [Value::Number(Number::Real(first)), rest @ ..] = values else { return None };
			let mut max = *first;
			for value in rest {
				let Value::Number(Number::Real(real)) = value else { return None };
				max = max.max(*real);
			}
			Some(Value::Number(Number::Real(max)))
		},

		// Statistics across one or more real arguments
		// TODO: Offer `avg` and `average` as autocomplete aliases in the expression widget, resolving to `mean`
		"mean" => |values| {
			let reals = real_operands(values)?;
			let scale = power_of_two_scale(&reals);
			Some(Value::from_f64(reals.iter().map(|real| real / scale).sum::<f64>() / reals.len() as f64 * scale))
		},

		"median" => |values| {
			let mut reals = real_operands(values)?;
			reals.sort_by(f64::total_cmp);
			let middle = reals.len() / 2;
			// An even count has no single middle value, so the two straddling it are averaged
			let median = if reals.len() % 2 == 0 { reals[middle - 1].midpoint(reals[middle]) } else { reals[middle] };

			Some(Value::from_f64(median))
		},

		// The bare names are the sample forms and the `pop` suffix marks the population forms
		"variance" => |values| scaled_variance(values, 1).map(|(variance, scale)| Value::from_f64(variance * scale * scale)),
		"variancepop" => |values| scaled_variance(values, 0).map(|(variance, scale)| Value::from_f64(variance * scale * scale)),
		"stdev" => |values| scaled_variance(values, 1).map(|(variance, scale)| Value::from_f64(variance.sqrt() * scale)),
		"stdevpop" => |values| scaled_variance(values, 0).map(|(variance, scale)| Value::from_f64(variance.sqrt() * scale)),

		"geomean" => |values| {
			let reals = real_operands(values)?;
			// A negative operand has no real geometric mean, and averaging the logarithms keeps the product from overflowing
			if reals.iter().any(|real| *real < 0.) {
				return None;
			}
			Some(Value::from_f64((reals.iter().map(|real| real.ln()).sum::<f64>() / reals.len() as f64).exp()))
		},

		"harmmean" => |values| {
			let reals = real_operands(values)?;

			// Like the geometric mean, a negative operand has no meaningful harmonic mean, while a zero one makes it zero
			if reals.iter().any(|real| *real < 0.) {
				return None;
			}
			let smallest = reals.iter().copied().fold(f64::INFINITY, f64::min);
			if smallest == 0. || smallest.is_infinite() {
				return Some(Value::from_f64(smallest));
			}

			// Dividing the smallest operand by each keeps every reciprocal term within 1, so their sum can't overflow
			let scaled_reciprocal_sum = reals.iter().map(|real| smallest / real).sum::<f64>();
			Some(Value::from_f64(reals.len() as f64 / scaled_reciprocal_sum * smallest))
		},

		"rms" => |values| {
			let reals = real_operands(values)?;
			let scale = power_of_two_scale(&reals);
			let mean_square = reals.iter().map(|real| (real / scale).powi(2)).sum::<f64>() / reals.len() as f64;
			Some(Value::from_f64(mean_square.sqrt() * scale))
		},

		"mode" => |values| {
			let mut reals = real_operands(values)?;
			reals.sort_by(f64::total_cmp);

			// In ascending order, the first run of the greatest length is the smallest of the most frequent values, and no mode exists when no value repeats
			let mut mode = None;
			let mut mode_count = 1;
			for run in reals.chunk_by(|a, b| a == b) {
				if run.len() > mode_count {
					mode = Some(run[0]);
					mode_count = run.len();
				}
			}
			mode.map(Value::from_f64)
		},

		"count" => |values| Some(Value::from_f64(values.len() as f64)),

		// Variadic parity across logical operands, which must each be exactly 0 or 1
		"xor" => |values| {
			let mut parity = false;
			for value in values {
				let Value::Number(Number::Real(real)) = value else { return None };
				if *real == 1. {
					parity = !parity;
				} else if *real != 0. {
					return None;
				}
			}
			Some(Value::from_f64(parity as u8 as f64))
		},

		"lerp" => |values| match values {
			[Value::Number(Number::Real(a)), Value::Number(Number::Real(b)), Value::Number(Number::Real(t))] => Some(Value::from_f64(lerp(*a, *b, *t))),
			_ => None,
		},

		"remap" => |values| match values {
			[
				Value::Number(Number::Real(value)),
				Value::Number(Number::Real(in_a)),
				Value::Number(Number::Real(in_b)),
				Value::Number(Number::Real(out_a)),
				Value::Number(Number::Real(out_b)),
			] => Some(Value::from_f64(lerp(*out_a, *out_b, inverse_lerp(*value, *in_a, *in_b)))),
			_ => None,
		},

		"trunc" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.trunc()))),
			_ => None,
		},

		"fract" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.fract()))),
			_ => None,
		},

		"sign" => |values| match values {
			[Value::Number(Number::Real(real))] => {
				let s = if *real > 0. {
					1.
				} else if *real < 0. {
					-1.
				} else {
					0.
				};
				Some(Value::Number(Number::Real(s)))
			}
			_ => None,
		},

		"gcd" => |values| {
			let reduced = real_operands(values)?
				.into_iter()
				.try_fold(0_u128, |accumulated, real| Some(gcd(accumulated, integer_operand(real)?)))?;
			Some(Value::from_f64(reduced as f64))
		},

		"lcm" => |values| {
			let reduced = real_operands(values)?
				.into_iter()
				.try_fold(1_u128, |accumulated, real| checked_lcm(accumulated, integer_operand(real)?))?;
			Some(Value::from_f64(reduced as f64))
		},

		// Combinatorics over whole numbers: `choose(n, r)` is the binomial coefficient and `pick(n, r)` the falling factorial
		"choose" => |values| match values {
			[Value::Number(Number::Real(n)), Value::Number(Number::Real(r))] => {
				let (n, r) = (whole_operand(*n)?, whole_operand(*r)?);
				if r > n {
					return Some(Value::from_f64(0.));
				}

				// Multiplying then dividing at each step keeps every intermediate whole, and the smaller of `r` and `n - r` halves the steps
				let r = r.min(n - r);
				let binomial = bounded_product(1..=r, |accumulated, k| accumulated * (n - r + k) as f64 / k as f64);
				Some(Value::from_f64(binomial))
			}
			_ => None,
		},

		"pick" => |values| match values {
			[Value::Number(Number::Real(n)), Value::Number(Number::Real(r))] => {
				let (n, r) = (whole_operand(*n)?, whole_operand(*r)?);
				if r > n {
					return Some(Value::from_f64(0.));
				}

				let falling_factorial = bounded_product(0..r, |accumulated, k| accumulated * (n - k) as f64);
				Some(Value::from_f64(falling_factorial))
			}
			_ => None,
		},

		// The conjugate negates the imaginary part
		"conj" => |values| match values {
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.conj()))),
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(*real))),
			_ => None,
		},

		_ => return None,
	})
}
