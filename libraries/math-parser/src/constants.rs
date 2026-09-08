use crate::ast::BinaryOp;
use crate::value::{Complex, Number, Value, complex_divide, complex_log_gamma};
use num_complex::ComplexFloat;
use std::cmp::Ordering;
use std::f64::consts::{LN_2, PI, TAU};

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

/// Rounds a real, passing an integer through since it is already whole.
fn rounding(values: &[Value], function: fn(f64) -> f64) -> Option<Value> {
	if let [whole @ Value::Number(Number::Integer(_))] = values {
		return Some(*whole);
	}
	let [x] = reals(values)?;
	Some(Value::from_f64(function(x)))
}

/// The argument ordered `extreme` of all the others (the earliest among equals), returning that operand itself. A complex argument has no order and errors.
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

/// Every argument in the complex plane, or `None` if there are no arguments at all.
fn complex_operands(values: &[Value]) -> Option<impl Iterator<Item = Complex> + Clone + '_> {
	if values.is_empty() {
		return None;
	}
	Some(values.iter().map(|Value::Number(number)| number.as_complex()))
}

/// The [`power_of_two_scale`] of the numbers' largest part.
fn scale_of(numbers: impl Iterator<Item = Complex>) -> f64 {
	power_of_two_scale(numbers.flat_map(|number| [number.re, number.im]))
}

/// Applies a one-argument function that may climb into the complex plane: a real result that does not exist,
/// like `sqrt(-4)`, `ln(-1)`, or `asin(2)`, is recomputed as the function's principal complex value.
fn climbing(values: &[Value], real_function: fn(f64) -> f64, complex_function: fn(Complex) -> Complex) -> Option<Value> {
	let [Value::Number(number)] = values else { return None };
	match number.as_real() {
		Some(real) => {
			let result = real_function(real);
			let result = if result.is_nan() {
				Value::from(complex_function(Complex::new(real, 0.)))
			} else {
				Value::from_f64(result)
			};
			Some(result)
		}
		None => Some(Value::from(complex_function(number.as_complex()))),
	}
}

/// The power of two at or below the largest magnitude, dividing by which is exact and brings every value within ±2, so sums and
/// squares of the scaled values neither overflow nor underflow. It's 1 when the largest magnitude is zero, subnormal, or infinite.
fn power_of_two_scale(reals: impl Iterator<Item = f64>) -> f64 {
	let largest = reals.fold(0_f64, |largest, real| largest.max(real.abs()));
	if !largest.is_normal() {
		return 1.;
	}

	// Keeping only the exponent bits zeroes the mantissa, leaving the power of two
	f64::from_bits(largest.to_bits() & (0x7FF << 52))
}

/// Computes the variance of the arguments, the mean of `|x - mean|²`, divided by the returned scale, over the count less `correction`
/// (1 for a sample, undefined for a single value, or 0 for a population). Staying scaled lets a standard deviation take its root before overflowing.
fn scaled_variance(values: &[Value], correction: usize) -> Option<(f64, f64)> {
	let numbers = complex_operands(values)?;
	let divisor = values.len().checked_sub(correction).filter(|divisor| *divisor > 0)? as f64;
	let scale = scale_of(numbers.clone());

	let mean = numbers.clone().map(|number| number / scale).sum::<Complex>() / values.len() as f64;
	let variance = numbers.map(|number| (number / scale - mean).norm_sqr()).sum::<f64>() / divisor;
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
	let z = x.as_complex();
	let product = (r <= DIRECT_PRODUCT_LIMIT).then(|| {
		if binomial {
			(1..=r).fold(Complex::from(1.), |accumulated, k| accumulated * (z - (k - 1) as f64) / k as f64)
		} else {
			(0..r).fold(Complex::from(1.), |accumulated, k| accumulated * (z - k as f64))
		}
	});

	// An overflowed product has NaN cross terms, so the gamma function takes over with the overflow's direction
	let result = match product {
		Some(product) if !Number::Complex(product).is_nan() => product,
		_ => {
			let count_log = if binomial { complex_log_gamma(Complex::from(r as f64 + 1.)) } else { Complex::from(0.) };
			(complex_log_gamma(z + 1.) - complex_log_gamma(z - r as f64 + 1.) - count_log).exp()
		}
	};

	// A real top has a real answer, so the rounding residue in the imaginary part is dropped
	Value::Number(if x.as_real().is_some() { Number::Real(result.re) } else { Number::Complex(result) })
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

	Some((builtin_function(function)?.function, base))
}

/// A built-in math function and whether it's variadic.
#[derive(Clone, Copy)]
pub struct Builtin {
	pub function: BuiltinFunction,
	/// Takes any count of arguments, like `min(a, b, c)`, which makes its name usable as a lone reducer token.
	pub variadic: bool,
}

/// Defines a built-in function taking a particular count of arguments, or a few like `log(x)` and `log(x, base)`.
fn fixed_arity(function: BuiltinFunction) -> Builtin {
	Builtin { function, variadic: false }
}

/// Defines a built-in function taking any count of arguments.
fn variadic(function: BuiltinFunction) -> Builtin {
	Builtin { function, variadic: true }
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
			// Change of base, staying real when it can and climbing into the complex plane when it cannot
			[Value::Number(x), Value::Number(base)] => match (x.as_real(), base.as_real()) {
				(Some(x), Some(base)) => {
					let log = x.ln() / base.ln();
					Some(if log.is_nan() {
						Value::from(Complex::new(x, 0.).ln() / Complex::new(base, 0.).ln())
					} else {
						Value::from_f64(log)
					})
				}
				_ => Some(Value::from(x.as_complex().ln() / base.as_complex().ln())),
			},
			_ => None,
		}),

		"root" => fixed_arity(|values| {
			let [Value::Number(x), Value::Number(n)] = values else { return None };
			// A complex degree is the general power `x^(1/n)`
			let Some(n) = n.as_real() else {
				return Some(Value::from(x.as_complex().powc(n.as_complex().inv())));
			};
			match x.as_real() {
				Some(x) => {
					// An odd root of a negative real is real, where `powf` alone would climb to the principal complex root
					if x < 0. && n.rem_euclid(2.) == 1. {
						return Some(Value::from_f64(-(-x).powf(1. / n)));
					}
					let root = x.powf(1. / n);
					Some(if root.is_nan() { Value::from(Complex::new(x, 0.).powf(1. / n)) } else { Value::from_f64(root) })
				}
				None => Some(Value::from(x.as_complex().powf(1. / n))),
			}
		}),

		// Geometry Functions
		// Folding pairwise hypotenuses gives the root of the sum of squares without ever squaring, avoiding overflow
		"hypot" => variadic(|values| Some(Value::from_f64(real_operands(values)?.fold(0., f64::hypot)))),

		"atan2" => fixed_arity(|values| {
			let [y, x] = reals(values)?;
			Some(Value::from_f64(y.atan2(x)))
		}),

		// Mapping Functions
		// Each part's absolute value, where `|x|` is instead the one magnitude of the whole value
		"abs" => fixed_arity(|values| match values {
			[Value::Number(Number::Integer(integer))] => Some(integer.checked_abs().map_or(Value::from_f64((*integer as f64).abs()), Value::from_i64)),
			[Value::Number(Number::Real(real))] => Some(Value::from_f64(real.abs())),
			[Value::Number(Number::Complex(complex))] => Some(Value::from(Complex::new(complex.re.abs(), complex.im.abs()))),
			_ => None,
		}),

		"floor" => fixed_arity(|values| rounding(values, f64::floor)),
		"ceil" => fixed_arity(|values| rounding(values, f64::ceil)),
		"round" => fixed_arity(|values| rounding(values, f64::round)),

		"clamp" => fixed_arity(|values| {
			let [Value::Number(x), Value::Number(min), Value::Number(max)] = values else { return None };
			// The bounds apply in turn, so the upper one wins where they cross
			let at_least = if min.real_ordering(*x)? == Ordering::Greater { min } else { x };
			Some(Value::Number(if max.real_ordering(*at_least)? == Ordering::Less { *max } else { *at_least }))
		}),

		// Variadic across one or more real arguments
		"min" => variadic(|values| extremum(values, Ordering::Less)),
		"max" => variadic(|values| extremum(values, Ordering::Greater)),

		// Statistics across one or more arguments, with the median and mode over real ones since they need an order
		// TODO: Offer `avg` and `average` as autocomplete aliases in the expression widget, resolving to `mean`
		"mean" => variadic(|values| {
			let numbers = complex_operands(values)?;
			let scale = scale_of(numbers.clone());
			Some(Value::from(numbers.map(|number| number / scale).sum::<Complex>() / values.len() as f64 * scale))
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
			let numbers = complex_operands(values)?;
			let scale = scale_of(numbers.clone());
			let mean_square = numbers.map(|number| (number / scale).norm_sqr()).sum::<f64>() / values.len() as f64;
			Some(Value::from_f64(mean_square.sqrt() * scale))
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

		"lerp" => fixed_arity(|values| {
			let [a, b, t] = reals(values)?;
			Some(Value::from_f64(lerp(a, b, t)))
		}),

		"remap" => fixed_arity(|values| {
			let [value, in_a, in_b, out_a, out_b] = reals(values)?;
			Some(Value::from_f64(lerp(out_a, out_b, inverse_lerp(value, in_a, in_b))))
		}),

		"trunc" => fixed_arity(|values| rounding(values, f64::trunc)),

		"fract" => fixed_arity(|values| {
			let [x] = reals(values)?;
			Some(Value::from_f64(x.fract()))
		}),

		"sign" => fixed_arity(|values| {
			let [x] = reals(values)?;
			Some(Value::from_i64(if x > 0. {
				1
			} else if x < 0. {
				-1
			} else {
				0
			}))
		}),

		"mod" => fixed_arity(|values| {
			// Integers stay exact, reaching the reals only for a zero modulus or `i64::MIN` modulo `-1`
			if let [Value::Number(Number::Integer(x)), Value::Number(Number::Integer(modulus))] = values
				&& let Some(remainder) = x.checked_rem(*modulus)
			{
				return Some(Value::from_i64(if remainder != 0 && (remainder < 0) != (*modulus < 0) { remainder + modulus } else { remainder }));
			}

			let [x, modulus] = reals(values)?;
			// Floored, so a truncated remainder with the opposite sign from the modulus moves over by one modulus
			let remainder = x % modulus;
			Some(Value::from_f64(if remainder != 0. && (remainder < 0.) != (modulus < 0.) { remainder + modulus } else { remainder }))
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

		// The conjugate negates the imaginary part
		"conj" => fixed_arity(|values| {
			let [Value::Number(number)] = values else { return None };
			Some(Value::Number(match number {
				Number::Complex(complex) => Number::Complex(complex.conj()),
				real => *real,
			}))
		}),

		_ => return None,
	})
}
