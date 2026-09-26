use crate::ast::BinaryOp;
use crate::executer::EvalError;
use crate::matrix::{Matrix, Region};
use crate::quaternion::Quaternion;
use crate::value::{Complex, Number, Value, complex_divide, complex_log_gamma, part_product, power_of_two_scale};
use num_complex::ComplexFloat;
use std::array;
use std::cmp::Ordering;
use std::f64::consts::{LN_2, PI, TAU};
use std::ops::RangeInclusive;

pub type BuiltinFunction = fn(&[Value]) -> Option<Value>;

/// Whether the value is a whole real number, which the integer functions require rather than rounding to.
fn is_whole(value: &Value) -> bool {
	value.as_real().is_some_and(|real| real.fract() == 0.)
}

/// Reads an operand's magnitude for `gcd`/`lcm`, or `None` unless it is a whole number within the widest storage.
fn integer_operand(value: &Value) -> Option<u128> {
	value.as_i128().filter(|_| is_whole(value)).map(i128::unsigned_abs)
}

/// Reads a combinatorics count, or `None` unless it is a whole number.
fn whole_count(value: &Value) -> Option<u64> {
	value.as_u64().filter(|_| is_whole(value))
}

/// Wraps a computed magnitude, which takes the real form only in the one case it exceeds integer storage.
fn integer_value(magnitude: u128) -> Value {
	i64::try_from(magnitude).map_or(Value::from_f64(magnitude as f64), Value::from_i64)
}

/// Reads exactly `N` real arguments, or `None` when the count differs or an argument is not a real number.
fn reals<const N: usize>(values: &[Value]) -> Option<[f64; N]> {
	if values.len() != N {
		return None;
	}

	let mut reals = [0.; N];
	for (real, value) in reals.iter_mut().zip(values) {
		*real = value.as_real()?;
	}
	Some(reals)
}

/// Rounds each part of the one argument, as `floor`, `ceil`, `round`, and `trunc` do.
fn rounding(values: &[Value], function: fn(f64) -> f64) -> Option<Value> {
	let [Value::Number(number)] = values else { return None };
	Some(Value::Number(number.round_parts(function)))
}

/// The argument ordered `extreme` of all the others (the earliest among equals), returning that operand itself, or `None` when one has a vector part and so no order.
fn extremum(values: &[Value], extreme: Ordering) -> Option<Value> {
	let (mut kept, rest) = values.split_first()?;
	// The first argument meets no comparison of its own, so its order is checked here
	kept.as_real()?;
	for candidate in rest {
		let (Value::Number(kept_number), Value::Number(candidate_number)) = (kept, candidate);
		if candidate_number.real_ordering(*kept_number)? == extreme {
			kept = candidate;
		}
	}
	Some(*kept)
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

/// Every argument as a real number, or `None` if any is complex or there are no arguments at all.
fn real_operands(values: &[Value]) -> Option<impl Iterator<Item = f64> + Clone + '_> {
	if values.is_empty() || !values.iter().all(|value| value.as_real().is_some()) {
		return None;
	}
	Some(values.iter().filter_map(Value::as_real))
}

/// Every argument in the complex plane, or `None` if one has a `j` or `k` part or there are no arguments at all.
fn complex_operands(values: &[Value]) -> Option<impl Iterator<Item = Complex> + Clone + '_> {
	if values.is_empty() || !values.iter().all(|Value::Number(number)| number.as_complex().is_some()) {
		return None;
	}
	Some(values.iter().filter_map(|Value::Number(number)| number.as_complex()))
}

/// Every argument in its full quaternion form, or `None` if there are no arguments at all. The statistics take this path only
/// once an argument has a vector part, since reals need one part each rather than four.
fn quaternion_operands(values: &[Value]) -> Option<impl Iterator<Item = Quaternion> + Clone + '_> {
	if values.is_empty() {
		return None;
	}
	Some(values.iter().map(|Value::Number(number)| number.to_quaternion()))
}

/// Applies a one-argument function, climbing into the complex plane where a real result does not exist (`sqrt(-4)`, `ln(-1)`,
/// `asin(2)`) and taking a number with a `j` or `k` part in its own complex plane.
fn apply_climbing(number: Number, real_function: impl Fn(f64) -> f64, complex_function: impl Fn(Complex) -> Complex) -> Number {
	if let Some(real) = number.as_real() {
		let result = real_function(real);
		return if result.is_nan() {
			Number::Complex(complex_function(Complex::new(real, 0.)))
		} else {
			Number::Real(result)
		};
	}

	match number.as_complex() {
		Some(complex) => Number::Complex(complex_function(complex)),
		None => Number::Quaternion(number.to_quaternion().in_plane(complex_function)),
	}
}

/// The one-argument builtin form of [`apply_climbing`].
fn climbing(values: &[Value], real_function: fn(f64) -> f64, complex_function: fn(Complex) -> Complex) -> Option<Value> {
	let [Value::Number(number)] = values else { return None };
	Some(Value::Number(apply_climbing(*number, real_function, complex_function)))
}

/// Applies a function to every part of the one argument, the componentwise reading of `abs`, `fract`, and `sign`.
fn mapping(values: &[Value], function: fn(f64) -> f64) -> Option<Value> {
	let [Value::Number(number)] = values else { return None };
	Some(Value::Number(number.map_parts(function)))
}

/// Folds every argument's quaternion form part by part, where a real's vector parts are zero, or `None` when there are no arguments.
fn zipping(values: &[Value], function: fn(f64, f64) -> f64) -> Option<Value> {
	values
		.iter()
		.map(|Value::Number(number)| number.to_quaternion())
		.reduce(|accumulated, quaternion| accumulated.zip(quaternion, function))
		.map(Value::from)
}

/// Reads exactly `N` arguments in their full quaternion form, or `None` when the count differs.
fn quaternions<const N: usize>(values: &[Value]) -> Option<[Quaternion; N]> {
	if values.len() != N {
		return None;
	}

	let mut quaternions = [Quaternion::default(); N];
	for (quaternion, Value::Number(number)) in quaternions.iter_mut().zip(values) {
		*quaternion = number.to_quaternion();
	}
	Some(quaternions)
}

/// Splits an optional trailing axis argument off a call's `fixed` leading arguments, defaulting it to `k`, the canvas normal.
fn with_axis(values: &[Value], fixed: usize) -> Option<(&[Value], Quaternion)> {
	match values.len().checked_sub(fixed)? {
		0 => Some((values, Quaternion::K)),
		1 => {
			let Value::Number(axis) = values[fixed];
			Some((&values[..fixed], axis.to_quaternion()))
		}
		_ => None,
	}
}

/// The rotor `cos(θ/2) + sin(θ/2) axis` about the axis's vector part, or `None` for an axis with no direction.
fn rotor(angle: f64, axis: Quaternion) -> Option<Quaternion> {
	let axis = Quaternion::new(0., axis.x, axis.y, axis.z).normalized()?;
	let (sin, cos) = (angle / 2.).sin_cos();
	Some(Quaternion::new(cos, 0., 0., 0.) + axis.map(|part| part * sin))
}

/// The projection of `a` onto `b` over all four parts, or `None` for a zero `b`.
fn projection(a: Quaternion, b: Quaternion) -> Option<Quaternion> {
	// Onto the unit direction, whose dot product with `a` cannot overflow, and whose zero parts stay zero beside an infinite one
	let direction = b.normalized()?;
	let length = a.dot(direction);
	Some(direction.map(|part| part_product(part, length, false)))
}

/// The mean of `count` numbers given as their `N` parts, each part averaged on its own over its [`power_of_two_scale`] so its sum cannot overflow.
pub(crate) fn mean_of<const N: usize>(numbers: impl Iterator<Item = [f64; N]> + Clone, count: usize) -> [f64; N] {
	array::from_fn(|index| {
		let parts = numbers.clone().map(|parts| parts[index]);
		let scale = power_of_two_scale(parts.clone());
		parts.map(|part| part / scale).sum::<f64>() / count as f64 * scale
	})
}

/// Computes the variance of the arguments, the mean of `|x - mean|²`, divided by the returned scale, over the count less `correction`
/// (1 for a sample, undefined for a single value, or 0 for a population). Staying scaled lets a standard deviation take its root before overflowing.
fn scaled_variance(values: &[Value], correction: usize) -> Option<(f64, f64)> {
	let divisor = values.len().checked_sub(correction).filter(|divisor| *divisor > 0)? as f64;
	Some(match real_operands(values) {
		Some(reals) => scaled_variance_of(reals.map(|real| [real]), values.len(), divisor),
		None => scaled_variance_of(quaternion_operands(values)?.map(Quaternion::parts), values.len(), divisor),
	})
}

/// The [`scaled_variance`] of `count` numbers given as their `N` parts.
fn scaled_variance_of<const N: usize>(numbers: impl Iterator<Item = [f64; N]> + Clone, count: usize, divisor: f64) -> (f64, f64) {
	let scale = power_of_two_scale(numbers.clone().flatten());
	let scaled = numbers.map(|parts| parts.map(|part| part / scale));

	let sum = scaled.clone().fold([0.; N], |sum, parts| array::from_fn(|index| sum[index] + parts[index]));
	let mean = sum.map(|part| part / count as f64);
	let variance = scaled.map(|parts| (0..N).map(|index| (parts[index] - mean[index]).powi(2)).sum::<f64>()).sum::<f64>() / divisor;
	(variance, scale)
}

/// The root mean square of `count` numbers given by all their parts, over a [`power_of_two_scale`] so no square overflows or underflows.
fn root_mean_square(parts: impl Iterator<Item = f64> + Clone, count: usize) -> f64 {
	let scale = power_of_two_scale(parts.clone());
	let mean_square = parts.map(|part| (part / scale).powi(2)).sum::<f64>() / count as f64;
	mean_square.sqrt() * scale
}

/// Whether every part of the number is finite.
fn all_parts_finite(number: Number) -> bool {
	number.to_quaternion().parts().iter().all(|part| part.is_finite())
}

/// Interpolates from `a` to `b` by `t` as `a + (b - a) t`, or as `a (1 - t) + b t` when finite endpoints are too far apart for `b - a` to fit.
fn lerp(a: f64, b: f64, t: f64) -> f64 {
	let difference = b - a;
	if difference.is_infinite() && a.is_finite() && b.is_finite() {
		return a * (1. - t) + b * t;
	}
	a + difference * t
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

/// Computes the least common multiple of two nonnegative integers. Operands that fit in i64 cannot overflow it.
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

/// The multiple of `step` nearest `x`, with halfway cases away from zero like `f64::round`, or `None` for a zero step or a multiple past integer storage.
fn integer_snap(x: i64, step: i64) -> Option<i64> {
	let (quotient, remainder) = (x.checked_div(step)?, x.checked_rem(step)?);
	// The truncated quotient falls one short of the nearest when the remainder reaches half the step
	let quotient = if remainder.unsigned_abs() >= step.unsigned_abs() - remainder.unsigned_abs() {
		quotient + if (x < 0) == (step < 0) { 1 } else { -1 }
	} else {
		quotient
	};
	quotient.checked_mul(step)
}

/// `choose(n, r)` over a whole nonnegative top, exact while it fits, and zero once `r` exceeds `n`.
fn whole_binomial(n: f64, r: u64) -> f64 {
	if r as f64 > n {
		return 0.;
	}

	// Multiplying then dividing at each step keeps every intermediate whole, and the smaller of `r` and `n - r` halves the steps
	let r = r.min((n - r as f64) as u64);
	bounded_product(1..=r, |accumulated, k| accumulated * (n - r as f64 + k as f64) / k as f64)
}

/// `pick(n, r)` over a whole nonnegative top, and zero once `r` exceeds `n`.
fn whole_falling_factorial(n: f64, r: u64) -> f64 {
	if r as f64 > n {
		return 0.;
	}
	bounded_product(0..r, |accumulated, k| accumulated * (n - k as f64))
}

/// [`whole_binomial`] in integer storage, or `None` once the product outgrows it.
fn exact_binomial(n: u64, r: u64) -> Option<u128> {
	if r > n {
		return Some(0);
	}
	let r = r.min(n - r);
	(1..=r).try_fold(1_u128, |accumulated, k| Some(accumulated.checked_mul((n - r + k) as u128)? / k as u128))
}

/// [`whole_falling_factorial`] in integer storage, or `None` once the product outgrows it.
fn exact_falling_factorial(n: u64, r: u64) -> Option<u128> {
	if r > n {
		return Some(0);
	}
	(0..r).try_fold(1_u128, |accumulated, k| accumulated.checked_mul((n - k) as u128))
}

/// The falling factorial `x (x - 1) ... (x - r + 1)` over any top, or the binomial coefficient dividing it by `r!`, by direct
/// product (a negative whole top as `(-1)^r` times that of `|x| + r - 1`), or past a few thousand terms of any other top, the gamma function.
fn combinatorial(x: Number, r: u64, binomial: bool) -> Value {
	if let Some(real) = x.as_real()
		&& (real.fract() == 0. || real.is_infinite())
	{
		let negative = real < 0. && r % 2 == 1;
		let top = if real < 0. { r as f64 - 1. - real } else { real };

		// Exact while the top fits and the product stays within integer storage, continuing in the reals past either
		let exact = (top < u64::MAX as f64).then(|| if binomial { exact_binomial(top as u64, r) } else { exact_falling_factorial(top as u64, r) });
		return match exact.flatten().map(i64::try_from) {
			Some(Ok(magnitude)) => Value::from_i64(if negative { -magnitude } else { magnitude }),
			_ => {
				let product = if binomial { whole_binomial(top, r) } else { whole_falling_factorial(top, r) };
				Value::from_f64(if negative { -product } else { product })
			}
		};
	}

	// The direct product keeps small cases exact at a step per count, so only a count past a few thousand takes the gamma function
	const DIRECT_PRODUCT_LIMIT: u64 = 4096;
	let over_top = |z: Complex| {
		let product = (r <= DIRECT_PRODUCT_LIMIT).then(|| {
			if binomial {
				(1..=r).fold(Complex::from(1.), |accumulated, k| accumulated * (z - (k - 1) as f64) / k as f64)
			} else {
				(0..r).fold(Complex::from(1.), |accumulated, k| accumulated * (z - k as f64))
			}
		});

		// An overflowed product has NaN cross terms, so the gamma function takes over with the overflow's direction
		match product {
			Some(product) if !Number::Complex(product).is_nan() => product,
			_ => {
				let count_log = if binomial { complex_log_gamma(Complex::from(r as f64 + 1.)) } else { Complex::from(0.) };
				(complex_log_gamma(z + 1.) - complex_log_gamma(z - r as f64 + 1.) - count_log).exp()
			}
		}
	};

	// A real top has a real answer, so the rounding residue in the imaginary part is dropped
	Value::Number(apply_climbing(x, |real| over_top(Complex::from(real)).re, over_top))
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

	match builtin_function(function)? {
		Builtin::Values { function, .. } => Some((function, base)),
		_ => None,
	}
}

/// A built-in function of a matrix with a value result, like `det`.
pub type MatrixToValue = fn(Matrix) -> Value;
/// A built-in function of a matrix with a matrix result, like `linear`.
pub type MatrixToMatrix = fn(Matrix) -> Matrix;
/// A built-in function building a matrix from values, like `rotation`.
pub type ValuesToMatrix = fn(&[Value]) -> Option<Matrix>;
/// A built-in function of a value and regions with a value result, like `inside`.
pub type ValueOfRegions = fn(Value, &[Region]) -> Result<Value, EvalError>;

/// A built-in math function, by the sorts it takes and gives. Those taking matrices have their argument counts checked as the
/// expression is parsed.
#[derive(Clone)]
pub enum Builtin {
	Values {
		function: BuiltinFunction,
		/// Takes any count of arguments, like `min(a, b, c)`, which makes its name usable as a lone reducer token.
		variadic: bool,
	},
	OfMatrix(MatrixToValue),
	MatrixOfMatrix(MatrixToMatrix),
	MatrixOfValues {
		function: ValuesToMatrix,
		arity: RangeInclusive<usize>,
	},
	/// A value, then `regions` regions.
	OfValueAndRegions {
		function: ValueOfRegions,
		regions: usize,
	},
}

/// Defines a built-in function taking a particular count of arguments, or a few like `log(x)` and `log(x, base)`.
fn fixed_arity(function: BuiltinFunction) -> Builtin {
	Builtin::Values { function, variadic: false }
}

/// Defines a built-in function taking any count of arguments.
fn variadic(function: BuiltinFunction) -> Builtin {
	Builtin::Values { function, variadic: true }
}

/// Looks up a built-in math function by name, holding a plain function pointer so dispatch avoids hashing and dynamic allocation.
pub fn builtin_function(name: &str) -> Option<Builtin> {
	Some(match name {
		// Trigonometric functions, with the inverses climbing into the complex plane outside the real domain (`asin(2)`)
		"sin" => fixed_arity(|values| climbing(values, f64::sin, Complex::sin)),
		"cos" => fixed_arity(|values| climbing(values, f64::cos, Complex::cos)),
		"tan" => fixed_arity(|values| climbing(values, f64::tan, Complex::tan)),
		"csc" => fixed_arity(|values| climbing(values, |x| x.sin().recip(), |z| z.sin().recip())),
		"sec" => fixed_arity(|values| climbing(values, |x| x.cos().recip(), |z| z.cos().recip())),
		"cot" => fixed_arity(|values| climbing(values, |x| x.tan().recip(), |z| z.tan().recip())),

		// TODO: Offer the `arc-`/`ar-` spellings (`arcsin`, `artanh`) and the legacy `inv-` names as autocomplete aliases in the expression widget, resolving to these canonical names
		"asin" => fixed_arity(|values| climbing(values, f64::asin, Complex::asin)),
		"acos" => fixed_arity(|values| climbing(values, f64::acos, Complex::acos)),
		"atan" => fixed_arity(|values| climbing(values, f64::atan, Complex::atan)),
		"acsc" => fixed_arity(|values| climbing(values, |x| x.recip().asin(), |z| z.recip().asin())),
		"asec" => fixed_arity(|values| climbing(values, |x| x.recip().acos(), |z| z.recip().acos())),
		"acot" => fixed_arity(|values| climbing(values, |x| x.recip().atan(), |z| z.recip().atan())),

		// Hyperbolic functions, with the inverses likewise climbing outside the real domain (`acosh(0.5)`, `atanh(2)`)
		"sinh" => fixed_arity(|values| climbing(values, f64::sinh, Complex::sinh)),
		"cosh" => fixed_arity(|values| climbing(values, f64::cosh, Complex::cosh)),
		"tanh" => fixed_arity(|values| climbing(values, f64::tanh, Complex::tanh)),
		"csch" => fixed_arity(|values| climbing(values, |x| x.sinh().recip(), |z| z.sinh().recip())),
		"sech" => fixed_arity(|values| climbing(values, |x| x.cosh().recip(), |z| z.cosh().recip())),
		"coth" => fixed_arity(|values| climbing(values, |x| x.tanh().recip(), |z| z.tanh().recip())),
		"asinh" => fixed_arity(|values| climbing(values, f64::asinh, Complex::asinh)),
		"acosh" => fixed_arity(|values| climbing(values, f64::acosh, Complex::acosh)),
		"atanh" => fixed_arity(|values| climbing(values, f64::atanh, Complex::atanh)),
		"acsch" => fixed_arity(|values| climbing(values, |x| x.recip().asinh(), |z| z.recip().asinh())),
		"asech" => fixed_arity(|values| climbing(values, |x| x.recip().acosh(), |z| z.recip().acosh())),
		"acoth" => fixed_arity(|values| climbing(values, |x| x.recip().atanh(), |z| z.recip().atanh())),

		// Logarithms, exponentials, and roots, climbing outside the real domain (`ln(-1)`, `sqrt(-4)`)
		"ln" => fixed_arity(|values| climbing(values, f64::ln, Complex::ln)),
		"exp" => fixed_arity(|values| climbing(values, f64::exp, Complex::exp)),
		"sqrt" => fixed_arity(|values| climbing(values, f64::sqrt, Complex::sqrt)),
		"cbrt" => fixed_arity(|values| climbing(values, f64::cbrt, |z| z.powf(1. / 3.))),
		"log2" => fixed_arity(|values| climbing(values, f64::log2, |z| z.ln() / LN_2)),

		"log" => fixed_arity(|values| match values {
			[value] => climbing(std::slice::from_ref(value), f64::log10, |z| z.log10()),
			// Change of base, staying real when it can and otherwise climbing, each logarithm taken in its own plane
			[Value::Number(x), Value::Number(base)] => {
				if let (Some(x), Some(base)) = (x.as_real(), base.as_real()) {
					let log = x.ln() / base.ln();
					if !log.is_nan() {
						return Some(Value::from_f64(log));
					}
				}
				let (numerator, denominator) = (apply_climbing(*x, f64::ln, Complex::ln), apply_climbing(*base, f64::ln, Complex::ln));
				numerator.binary_op(BinaryOp::Div, denominator).map(Value::Number)
			}
			_ => None,
		}),

		"root" => fixed_arity(|values| {
			let [Value::Number(x), Value::Number(n)] = values else { return None };
			// A complex degree is the general power `x^(1/n)`
			let Some(n) = n.as_real() else {
				let reciprocal = Number::Integer(1).binary_op(BinaryOp::Div, *n)?;
				return x.binary_op(BinaryOp::Pow, reciprocal).map(Value::Number);
			};
			// An odd root of a negative real is real, where `powf` alone would climb to the principal complex root
			if let Some(x) = x.as_real()
				&& x < 0. && n.rem_euclid(2.) == 1.
			{
				return Some(Value::from_f64(-(-x).powf(1. / n)));
			}
			Some(Value::Number(apply_climbing(*x, |x| x.powf(1. / n), |z| z.powf(1. / n))))
		}),

		// Geometry Functions
		// The Euclidean norm over the arguments' magnitudes, folding pairwise hypotenuses so nothing is ever squared, avoiding overflow
		"hypot" => variadic(|values| (!values.is_empty()).then(|| Value::from_f64(values.iter().map(|Value::Number(number)| number.magnitude()).fold(0., f64::hypot)))),

		"atan2" => fixed_arity(|values| {
			let [y, x] = reals(values)?;
			Some(Value::from_f64(y.atan2(x)))
		}),

		// Mapping functions, acting on each part of a vector
		// `|x|` is instead the one magnitude of the whole value
		"abs" => fixed_arity(|values| match values {
			[Value::Number(Number::Integer(integer))] => Some(integer.checked_abs().map_or(Value::from_f64((*integer as f64).abs()), Value::from_i64)),
			_ => mapping(values, f64::abs),
		}),

		"floor" => fixed_arity(|values| rounding(values, f64::floor)),
		"ceil" => fixed_arity(|values| rounding(values, f64::ceil)),
		"round" => fixed_arity(|values| rounding(values, f64::round)),
		"trunc" => fixed_arity(|values| rounding(values, f64::trunc)),
		"fract" => fixed_arity(|values| mapping(values, f64::fract)),
		"sign" => fixed_arity(|values| {
			mapping(values, |x| {
				if x > 0. {
					1.
				} else if x < 0. {
					-1.
				} else {
					0.
				}
			})
		}),

		// The nearest multiple of `step`, rounding each part of `x / step`, so a complex value snaps to the square lattice spanned by `step` and `i step`
		"snap" => fixed_arity(|values| {
			let [Value::Number(x), Value::Number(step)] = values else { return None };
			// Integers stay exact, reaching the reals only for a zero step or a multiple past integer storage
			if let (Number::Integer(x), Number::Integer(step)) = (x, step)
				&& let Some(multiple) = integer_snap(*x, *step)
			{
				return Some(Value::from_i64(multiple));
			}

			let multiple = x.binary_op(BinaryOp::Div, *step)?.round_parts(f64::round);
			multiple.binary_op(BinaryOp::Mul, *step).map(Value::Number)
		}),

		"mod" => fixed_arity(|values| {
			let [Value::Number(x), Value::Number(modulus)] = values else { return None };
			// Integers stay exact, reaching the reals only for a zero modulus or `i64::MIN` modulo `-1`
			if let (Number::Integer(x), Number::Integer(modulus)) = (x, modulus)
				&& let Some(remainder) = x.checked_rem(*modulus)
			{
				return Some(Value::from_i64(if remainder != 0 && (remainder < 0) != (*modulus < 0) { remainder + modulus } else { remainder }));
			}

			// A real modulus wraps each part, floored so a remainder with the opposite sign from the modulus moves over by one modulus
			if let Some(modulus) = modulus.as_real() {
				return Some(Value::Number(x.map_parts(|part| {
					let remainder = part % modulus;
					if remainder != 0. && (remainder < 0.) != (modulus < 0.) { remainder + modulus } else { remainder }
				})));
			}

			// Otherwise `x - floor(x / m) m` with a per-part floor lands in the cell spanned by `m` and its rotations `im`, `jm`, `km`
			let quotient = x.binary_op(BinaryOp::Div, *modulus)?.map_parts(f64::floor);
			x.binary_op(BinaryOp::Sub, quotient.binary_op(BinaryOp::Mul, *modulus)?).map(Value::Number)
		}),

		// Variadic, exact over reals and otherwise part by part
		"min" => variadic(|values| extremum(values, Ordering::Less).or_else(|| zipping(values, f64::min))),
		"max" => variadic(|values| extremum(values, Ordering::Greater).or_else(|| zipping(values, f64::max))),

		// Statistics across one or more arguments: the median and mode over real ones, since they need an order, and the geometric and harmonic means within the complex plane
		// TODO: Offer `avg` and `average` as autocomplete aliases in the expression widget, resolving to `mean`
		"mean" => variadic(|values| {
			if values.is_empty() {
				return None;
			}

			// Integers sum exactly, since an `i128` holds the sum of any count of them
			let integer_sum = values.iter().try_fold(0_i128, |sum, value| match value {
				Value::Number(Number::Integer(integer)) => Some(sum + *integer as i128),
				_ => None,
			});
			if let Some(sum) = integer_sum {
				let count = values.len() as i128;
				return Some(if sum % count == 0 {
					Value::from_i64((sum / count) as i64)
				} else {
					Value::from_f64(sum as f64 / count as f64)
				});
			}

			Some(match real_operands(values) {
				Some(reals) => Value::from_f64(mean_of(reals.map(|real| [real]), values.len())[0]),
				None => Value::from(Quaternion::from_parts(mean_of(quaternion_operands(values)?.map(Quaternion::parts), values.len()))),
			})
		}),

		"median" => variadic(|values| {
			let mut reals: Vec<f64> = real_operands(values)?.collect();
			reals.sort_by(f64::total_cmp);
			let middle = reals.len() / 2;
			// An even count has no single middle value, so the two straddling it are averaged
			let median = if reals.len().is_multiple_of(2) {
				reals[middle - 1].midpoint(reals[middle])
			} else {
				reals[middle]
			};

			Some(Value::from_f64(median))
		}),

		// The bare names are the sample forms and the `pop` suffix marks the population forms
		"variance" => variadic(|values| scaled_variance(values, 1).map(|(variance, scale)| Value::from_f64(variance * scale * scale))),
		"variancepop" => variadic(|values| scaled_variance(values, 0).map(|(variance, scale)| Value::from_f64(variance * scale * scale))),
		"stdev" => variadic(|values| scaled_variance(values, 1).map(|(variance, scale)| Value::from_f64(variance.sqrt() * scale))),
		"stdevpop" => variadic(|values| scaled_variance(values, 0).map(|(variance, scale)| Value::from_f64(variance.sqrt() * scale))),

		// The principal root of the product, from the mean of the magnitudes' logarithms (so nothing overflows) and the product's
		// argument wrapped into `(-π, π]`, so `geomean(-1, -4)` is 2 and `geomean(-1, 2)` climbs to `i√2` like `sqrt(-2)`
		"geomean" => variadic(|values| {
			let numbers = complex_operands(values)?;
			// One value is its own mean exactly, where the polar form below would leave a residue on a negative one
			if let [value] = values {
				return Some(*value);
			}

			let count = values.len() as f64;
			let log_magnitude = numbers.clone().map(|number| number.norm().ln()).sum::<f64>() / count;
			let argument = numbers.map(|number| number.arg()).sum::<f64>().rem_euclid(TAU);
			let argument = if argument > PI { argument - TAU } else { argument };
			Some(Value::from(Complex::new(log_magnitude, argument / count).exp()))
		}),

		"harmmean" => variadic(|values| {
			let numbers = complex_operands(values)?;

			// A zero operand makes the mean zero, and only infinite ones make it their own mean, each contributing a zero reciprocal
			let smallest = numbers.clone().map(|number| number.norm()).fold(f64::INFINITY, f64::min);
			if smallest == 0. {
				return Some(Value::from_f64(0.));
			}
			if smallest.is_infinite() {
				return Some(Value::from(numbers.sum::<Complex>() / values.len() as f64));
			}

			// Dividing the smallest magnitude by each operand keeps every reciprocal term within 1, so their sum can't overflow
			let scaled_reciprocal_sum = Number::Complex(numbers.map(|number| complex_divide(Complex::from(smallest), number)).sum::<Complex>()).canonical();

			// Reciprocals that cancel, as in `harmmean(-1, 1)`, sum to zero, and the language's own division takes that to infinity
			let count = Number::Real(values.len() as f64);
			count
				.binary_op(BinaryOp::Div, scaled_reciprocal_sum)?
				.binary_op(BinaryOp::Mul, Number::Real(smallest))
				.map(Value::Number)
		}),

		"rms" => variadic(|values| {
			let rms = match real_operands(values) {
				Some(reals) => root_mean_square(reals, values.len()),
				None => root_mean_square(quaternion_operands(values)?.flat_map(Quaternion::parts), values.len()),
			};
			Some(Value::from_f64(rms))
		}),

		"mode" => variadic(|values| {
			let mut reals: Vec<f64> = real_operands(values)?.collect();
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
		}),

		"count" => variadic(|values| Some(Value::from_i64(values.len() as i64))),

		// Variadic parity across logical operands, which must each be exactly 0 or 1
		"xor" => variadic(|values| {
			let mut parity = false;
			for value in values {
				parity ^= value.as_bool()?;
			}
			Some(Value::from_bool(parity))
		}),

		// Interpolates between values of any rung by a real fraction
		"lerp" => fixed_arity(|values| {
			let [Value::Number(a), Value::Number(b), Value::Number(t)] = values else { return None };
			t.as_real()?;

			// Reals by a fractional `t` take the scalar form, while the general one below keeps integers exact and carries vectors
			if let (Some(a), Some(b), Number::Real(t)) = (a.as_real(), b.as_real(), t) {
				return Some(Value::from_f64(lerp(a, b, *t)));
			}

			// As in the real `lerp`, finite endpoints too far apart for `b - a` to fit are weighted separately instead
			let step = b.binary_op(BinaryOp::Sub, *a)?.binary_op(BinaryOp::Mul, *t)?;
			if !all_parts_finite(step) && all_parts_finite(*a) && all_parts_finite(*b) {
				let complement = Number::Integer(1).binary_op(BinaryOp::Sub, *t)?;
				return a.binary_op(BinaryOp::Mul, complement)?.binary_op(BinaryOp::Add, b.binary_op(BinaryOp::Mul, *t)?).map(Value::Number);
			}
			a.binary_op(BinaryOp::Add, step).map(Value::Number)
		}),

		// Spherical interpolation between unit quaternions, `a (a⁻¹ b)^t`, along the shorter arc since `q` and `-q` are one rotation
		"slerp" => fixed_arity(|values| {
			let [Value::Number(a), Value::Number(b), Value::Number(t)] = values else { return None };
			let t = t.as_real()?;
			let (a, b) = (a.to_quaternion(), b.to_quaternion());
			let b = if a.dot(b) < 0. { -b } else { b };

			// `a⁻¹ b` as the conjugate of `b* / a*`, so it shares division's scaling of a tiny or huge `a`
			let ratio = (b.conj() / a.conj()).conj();
			Some(Value::from(a * ratio.pow(Quaternion::new(t, 0., 0., 0.))))
		}),

		// Vector functions, over the vector part or over all four parts as each is defined
		"dot" => fixed_arity(|values| {
			let [a, b] = quaternions(values)?;
			Some(Value::from_f64(a.dot(b)))
		}),

		"cross" => fixed_arity(|values| {
			let [a, b] = quaternions(values)?;
			Some(Value::from(a.cross(b)))
		}),

		"normalize" => fixed_arity(|values| {
			let [v] = quaternions(values)?;
			v.normalized().map(Value::from)
		}),

		"distance" => fixed_arity(|values| {
			let [a, b] = quaternions(values)?;
			Some(Value::from_f64((a - b).norm()))
		}),

		// The angle from `a` to `b`, over all four parts, signed by the turn's direction seen from `+k` so that `rotate(a, angle(a, b))` is parallel to `b`
		"angle" => fixed_arity(|values| {
			let [a, b] = quaternions(values)?;
			let (a, b) = (a.normalized()?, b.normalized()?);

			// Kahan's form over the unit directions, which stays accurate at every angle
			let unsigned = 2. * (a - b).norm().atan2((a + b).norm());
			Some(Value::from_f64(if a.cross(b).z < 0. { -unsigned } else { unsigned }))
		}),

		// Rotates by an angle about an axis, `k` unless given, through the rotor sandwich `q v conj(q)`, which leaves the weight alone
		"rotate" => fixed_arity(|values| {
			let (values, axis) = with_axis(values, 2)?;
			let [Value::Number(v), Value::Number(angle)] = values else { return None };
			let rotor = rotor(angle.as_real()?, axis)?;
			Some(Value::from(rotor * v.to_quaternion() * rotor.conj()))
		}),

		"rotor" => fixed_arity(|values| {
			let (values, axis) = with_axis(values, 1)?;
			let [angle] = reals(values)?;
			rotor(angle, axis).map(Value::from)
		}),

		// A rotor's unit axis, which a rotor with no vector part does not have
		"axis" => fixed_arity(|values| {
			let [q] = quaternions(values)?;
			Quaternion::new(0., q.x, q.y, q.z).normalized().map(Value::from)
		}),

		"project" => fixed_arity(|values| {
			let [a, b] = quaternions(values)?;
			projection(a, b).map(Value::from)
		}),

		"reject" => fixed_arity(|values| {
			let [a, b] = quaternions(values)?;
			projection(a, b).map(|projected| Value::from(a - projected))
		}),

		// Reflects `v` across the hyperplane normal to `n`
		"reflect" => fixed_arity(|values| {
			let [v, n] = quaternions(values)?;
			projection(v, n).map(|projected| Value::from(v - projected.map(|part| part * 2.)))
		}),

		// The counterclockwise perpendicular in the `xy` plane, `cross(k, v)`
		"perp" => fixed_arity(|values| {
			let [v] = quaternions(values)?;
			Some(Value::from(Quaternion::K.cross(v)))
		}),

		// Integer functions, exact throughout integer storage
		"gcd" => variadic(|values| {
			if values.is_empty() {
				return None;
			}
			let reduced = values.iter().try_fold(0_u128, |accumulated, value| Some(gcd(accumulated, integer_operand(value)?)))?;
			Some(integer_value(reduced))
		}),

		"lcm" => variadic(|values| {
			if values.is_empty() {
				return None;
			}
			let reduced = values.iter().try_fold(1_u128, |accumulated, value| checked_lcm(accumulated, integer_operand(value)?))?;
			Some(integer_value(reduced))
		}),

		// Combinatorics over any top and a whole count: `choose(x, r)` is the binomial coefficient and `pick(x, r)` the falling factorial
		"choose" => fixed_arity(|values| {
			let [Value::Number(x), r] = values else { return None };
			Some(combinatorial(*x, whole_count(r)?, true))
		}),

		"pick" => fixed_arity(|values| {
			let [Value::Number(x), r] = values else { return None };
			Some(combinatorial(*x, whole_count(r)?, false))
		}),

		// The conjugate negates the vector part on every rung
		"conj" => fixed_arity(|values| {
			let [Value::Number(number)] = values else { return None };
			Some(Value::Number(match number {
				Number::Complex(complex) => Number::Complex(complex.conj()),
				Number::Quaternion(quaternion) => Number::Quaternion(quaternion.conj()),
				real => *real,
			}))
		}),

		// Matrix functions
		"det" => Builtin::OfMatrix(|matrix| Value::from_f64(matrix.determinant())),
		"linear" => Builtin::MatrixOfMatrix(|matrix| Matrix {
			translation: Quaternion::ZERO,
			..matrix
		}),
		// The image of the origin, `A 0`
		"translation" => Builtin::OfMatrix(|matrix| Value::from(matrix.translation)),

		// Left multiplication by the value, `L_q`
		"matrix" => Builtin::MatrixOfValues {
			arity: 1..=1,
			function: |values| {
				let [q] = quaternions(values)?;
				Some(Matrix::left_multiplication(q))
			},
		},

		"rotation" => Builtin::MatrixOfValues {
			arity: 1..=2,
			function: |values| {
				let (values, axis) = with_axis(values, 1)?;
				let [angle] = reals(values)?;
				Some(Matrix::rotation(rotor(angle, axis)?, axis))
			},
		},

		"scale" => Builtin::MatrixOfValues {
			arity: 1..=1,
			function: |values| {
				let [q] = quaternions(values)?;
				Some(Matrix::scale(q))
			},
		},

		"shear" => Builtin::MatrixOfValues {
			arity: 3..=3,
			function: |values| {
				let [along, by, factor] = values else { return None };
				let [along, by] = quaternions(&[*along, *by])?;
				Some(Matrix::shear(along, by, factor.as_real()?))
			},
		},

		// Range functions read a range literal by its corners, which may be infinite, like `inside(x, 0..inf)` for `x >= 0`, and undo
		// any other region to its parameter, which only the axes it spans constrain, a flat one to 0
		"inside" => Builtin::OfValueAndRegions {
			regions: 1,
			function: |p, regions| {
				let [region] = regions else { return Err(EvalError::TypeError) };
				let Value::Number(p) = p;
				let p = p.to_quaternion().parts();

				if let Region::Range(a, b) = *region {
					let spanned = Matrix::range_axes(a, b);
					let (a, b) = (a.parts(), b.parts());
					return Ok(Value::from_bool((0..4).all(|axis| !spanned[axis] || (a[axis].min(b[axis])..=a[axis].max(b[axis])).contains(&p[axis]))));
				}

				let range = region.matrix();
				let RangeParameter { parameter, flat, .. } = range_parameter(range, Quaternion::from_parts(p))?;
				let within = |axis: usize, part: f64| if flat[axis] { part == 0. } else { !range.axes[axis] || (0. ..=1.).contains(&part) };
				Ok(Value::from_bool(parameter.parts().into_iter().enumerate().all(|(axis, part)| within(axis, part))))
			},
		},

		"clamp" => Builtin::OfValueAndRegions {
			regions: 1,
			function: |x, regions| {
				let [region] = regions else { return Err(EvalError::TypeError) };
				let Value::Number(number) = x;
				let parts = number.to_quaternion().parts();

				let clamped = if let Region::Range(a, b) = *region {
					let spanned = Matrix::range_axes(a, b);
					let (a, b) = (a.parts(), b.parts());
					Quaternion::from_parts(array::from_fn(|axis| {
						if spanned[axis] {
							parts[axis].clamp(a[axis].min(b[axis]), a[axis].max(b[axis]))
						} else {
							parts[axis]
						}
					}))
				} else {
					let range = region.matrix();
					let RangeParameter { parameter, region, flat } = range_parameter(range, Quaternion::from_parts(parts))?;
					let parameter_parts = parameter.parts();
					let clamped = Quaternion::from_parts(array::from_fn(|axis| match (flat[axis], range.axes[axis]) {
						(true, _) => 0.,
						(false, true) => parameter_parts[axis].clamp(0., 1.),
						(false, false) => parameter_parts[axis],
					}));
					if clamped == parameter { Quaternion::from_parts(parts) } else { region.apply(clamped) }
				};

				// A value already within the range is itself, keeping an integer's exact storage
				Ok(if clamped.parts() == parts { x } else { Value::from(clamped) })
			},
		},

		// From one range to another, `B A⁻¹ x`, where a flat axis of `A` leaves the parameter undefined
		"remap" => Builtin::OfValueAndRegions {
			regions: 2,
			function: |x, regions| {
				let [from, to] = regions else { return Err(EvalError::TypeError) };
				let Value::Number(x) = x;
				let RangeParameter { parameter, flat, .. } = range_parameter(from.matrix(), x.to_quaternion())?;
				if flat.contains(&true) {
					return Err(EvalError::FlatRemapSource);
				}
				Ok(Value::from(to.matrix().region().apply(parameter)))
			},
		},

		_ => return None,
	})
}

/// Where a value lies against a range: its parameter `R⁻¹ p`, the invertible region that maps the parameter back, and which axes are flat.
struct RangeParameter {
	parameter: Quaternion,
	region: Matrix,
	/// The spanned axes with no extent, on which the parameter measures how far the value lies off the range.
	flat: [bool; 4],
}

fn range_parameter(range: Matrix, p: Quaternion) -> Result<RangeParameter, EvalError> {
	if !range.is_finite() {
		return Err(EvalError::Indeterminate);
	}

	let (region, flat) = range.invertible_region();
	let parameter = region.inverse().ok_or(EvalError::SingularRange)?.apply(p);
	if Number::Quaternion(parameter).is_nan() {
		return Err(EvalError::Indeterminate);
	}
	Ok(RangeParameter { parameter, region, flat })
}
