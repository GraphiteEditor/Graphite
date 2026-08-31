use crate::value::{Number, Value};
use num_complex::ComplexFloat;
use std::f64::consts::{LN_2, PI};

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
	if a == 0 || b == 0 {
		return 0;
	}
	(a / gcd(a, b)) * b
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
		"sin" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.sin()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.sin()))),
			_ => None,
		},

		"cos" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.cos()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.cos()))),
			_ => None,
		},

		"tan" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.tan()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.tan()))),
			_ => None,
		},

		"csc" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.sin().recip()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.sin().recip()))),
			_ => None,
		},

		"sec" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.cos().recip()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.cos().recip()))),
			_ => None,
		},

		"cot" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.tan().recip()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.tan().recip()))),
			_ => None,
		},

		// TODO: Offer the `arc-`/`ar-` spellings (`arcsin`, `artanh`) and the legacy `inv-` names as autocomplete aliases in the expression widget, resolving to these canonical names
		"asin" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.asin()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.asin()))),
			_ => None,
		},

		"acos" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.acos()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.acos()))),
			_ => None,
		},

		"atan" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.atan()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.atan()))),
			_ => None,
		},

		"acsc" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().asin()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().asin()))),
			_ => None,
		},

		"asec" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().acos()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().acos()))),
			_ => None,
		},

		"acot" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().atan()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().atan()))),
			_ => None,
		},
		// Hyperbolic Functions
		"sinh" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.sinh()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.sinh()))),
			_ => None,
		},

		"cosh" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.cosh()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.cosh()))),
			_ => None,
		},

		"tanh" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.tanh()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.tanh()))),
			_ => None,
		},

		// Reciprocal hyperbolic functions
		"csch" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.sinh().recip()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.sinh().recip()))),
			_ => None,
		},

		"sech" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.cosh().recip()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.cosh().recip()))),
			_ => None,
		},

		"coth" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.tanh().recip()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.tanh().recip()))),
			_ => None,
		},

		// Inverse Hyperbolic Functions
		"asinh" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.asinh()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.asinh()))),
			_ => None,
		},

		"acosh" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.acosh()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.acosh()))),
			_ => None,
		},

		"atanh" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.atanh()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.atanh()))),
			_ => None,
		},

		// Inverse reciprocal hyperbolic functions
		"acsch" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().asinh()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().asinh()))),
			_ => None,
		},

		"asech" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().acosh()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().acosh()))),
			_ => None,
		},

		"acoth" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().atanh()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().atanh()))),
			_ => None,
		},

		// Logarithm Functions
		"ln" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.ln()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.ln()))),
			_ => None,
		},

		// Exponential / power helpers
		"exp" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.exp()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.exp()))),
			_ => None,
		},

		"root" => |values| match values {
			[Value::Number(Number::Real(x)), Value::Number(Number::Real(n))] => {
				// Odd integer roots of negative reals are real, which powf alone would report as NaN
				let root = if *x < 0. && n.rem_euclid(2.) == 1. { -(-x).powf(1. / *n) } else { x.powf(1. / *n) };
				Some(Value::Number(Number::Real(root)))
			}
			[Value::Number(Number::Complex(x)), Value::Number(Number::Real(n))] => Some(Value::Number(Number::Complex(x.powf(1. / *n)))),
			_ => None,
		},

		"log" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.log10()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.log10()))),
			// Change of base, staying real when both operands are, and widening into the complex plane when either is not
			[Value::Number(Number::Real(x)), Value::Number(Number::Real(base))] => Some(Value::Number(Number::Real(x.ln() / base.ln()))),
			[Value::Number(x), Value::Number(base)] => Some(Value::Number(Number::Complex(x.as_complex().ln() / base.as_complex().ln()))),
			_ => None,
		},

		"log2" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.log2()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.ln() / LN_2))),
			_ => None,
		},

		// Root Functions
		"sqrt" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.sqrt()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.sqrt()))),
			_ => None,
		},

		"cbrt" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.cbrt()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.powf(1. / 3.)))),
			_ => None,
		},

		// Geometry Functions
		"hypot" => |values| match values {
			[Value::Number(Number::Real(a)), Value::Number(Number::Real(b))] => Some(Value::Number(Number::Real(a.hypot(*b)))),
			_ => None,
		},

		"atan2" => |values| match values {
			[Value::Number(Number::Real(y)), Value::Number(Number::Real(x))] => Some(Value::Number(Number::Real(y.atan2(*x)))),
			_ => None,
		},

		// Mapping Functions
		"abs" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.abs()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Real(complex.abs()))),
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

		// Variadic across one or more real arguments, ignoring NaN like Rust's own f64::min/f64::max
		"min" => |values| {
			// Seeded from the first argument so that arguments which are all NaN give back NaN rather than an infinity of their own, even though a NaN input should represent a bug
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

		"lerp" => |values| match values {
			[Value::Number(Number::Real(a)), Value::Number(Number::Real(b)), Value::Number(Number::Real(t))] => Some(Value::Number(Number::Real(a + (b - a) * t))),
			_ => None,
		},

		"remap" => |values| match values {
			[
				Value::Number(Number::Real(value)),
				Value::Number(Number::Real(in_a)),
				Value::Number(Number::Real(in_b)),
				Value::Number(Number::Real(out_a)),
				Value::Number(Number::Real(out_b)),
			] => {
				let t = (*value - *in_a) / (*in_b - *in_a);
				Some(Value::Number(Number::Real(out_a + t * (out_b - out_a))))
			}
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

		"gcd" => |values| match values {
			[Value::Number(Number::Real(a)), Value::Number(Number::Real(b))] => {
				let gcd = integer_operand(*a).zip(integer_operand(*b)).map_or(f64::NAN, |(a, b)| gcd(a, b) as f64);
				Some(Value::Number(Number::Real(gcd)))
			}
			_ => None,
		},

		"lcm" => |values| match values {
			[Value::Number(Number::Real(a)), Value::Number(Number::Real(b))] => {
				let lcm = integer_operand(*a).zip(integer_operand(*b)).map_or(f64::NAN, |(a, b)| lcm(a, b) as f64);
				Some(Value::Number(Number::Real(lcm)))
			}
			_ => None,
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

		// Complex Number Functions
		"real" => |values| match values {
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Real(complex.re))),
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(*real))),
			_ => None,
		},

		"imag" => |values| match values {
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Real(complex.im))),
			[Value::Number(Number::Real(_))] => Some(Value::Number(Number::Real(0.))),
			_ => None,
		},

		"conj" => |values| match values {
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.conj()))),
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(*real))),
			_ => None,
		},

		"arg" => |values| match values {
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Real(complex.arg()))),
			[Value::Number(Number::Real(real))] => {
				let angle = if *real >= 0. { 0. } else { PI };
				Some(Value::Number(Number::Real(angle)))
			}
			_ => None,
		},

		_ => return None,
	})
}
