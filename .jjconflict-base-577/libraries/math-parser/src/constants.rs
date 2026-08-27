use crate::value::{Number, Value};
use num_complex::ComplexFloat;
use std::f64::consts::{LN_2, PI};

pub type BuiltinFunction = fn(&[Value]) -> Option<Value>;

/// Truncates both operands to nonnegative integers for `gcd`/`lcm`, or `None` when either is non-finite or beyond f64's exactly-representable integer range.
fn integer_operands(a: f64, b: f64) -> Option<(u64, u64)> {
	// The largest magnitude below which every integer is exactly representable in f64
	const EXACT_INTEGER_LIMIT: f64 = (1_u64 << f64::MANTISSA_DIGITS) as f64;

	let (a, b) = (a.trunc(), b.trunc());
	if !a.is_finite() || !b.is_finite() || a.abs() > EXACT_INTEGER_LIMIT || b.abs() > EXACT_INTEGER_LIMIT {
		return None;
	}
	Some(((a as i64).unsigned_abs(), (b as i64).unsigned_abs()))
}

fn euclidean_gcd(mut x: u64, mut y: u64) -> u64 {
	while y != 0 {
		(x, y) = (y, x % y);
	}
	x
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

		// Inverse trig with legacy names and standard aliases
		"invsin" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.asin()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.asin()))),
			_ => None,
		},
		"asin" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.asin()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.asin()))),
			_ => None,
		},

		"invcos" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.acos()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.acos()))),
			_ => None,
		},
		"acos" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.acos()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.acos()))),
			_ => None,
		},

		"invtan" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.atan()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.atan()))),
			_ => None,
		},
		"atan" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.atan()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.atan()))),
			_ => None,
		},

		"invcsc" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().asin()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().asin()))),
			_ => None,
		},
		"acsc" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().asin()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().asin()))),
			_ => None,
		},

		"invsec" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().acos()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().acos()))),
			_ => None,
		},
		"asec" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().acos()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().acos()))),
			_ => None,
		},

		"invcot" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().atan()))),
			[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().atan()))),
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

		"pow" => |values| match values {
			[Value::Number(Number::Real(x)), Value::Number(Number::Real(n))] => Some(Value::Number(Number::Real(x.powf(*n)))),
			[Value::Number(Number::Complex(x)), Value::Number(Number::Real(n))] => Some(Value::Number(Number::Complex(x.powf(*n)))),
			[Value::Number(Number::Complex(x)), Value::Number(Number::Complex(n))] => Some(Value::Number(Number::Complex(x.powc(*n)))),
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
			[Value::Number(n), Value::Number(base)] => {
				// Custom base logarithm using change of base formula
				let compute_log = |x: f64, b: f64| -> f64 { x.ln() / b.ln() };
				match (n, base) {
					(Number::Real(x), Number::Real(b)) => Some(Value::Number(Number::Real(compute_log(*x, *b)))),
					_ => None,
				}
			}
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
				let gcd = integer_operands(*a, *b).map_or(f64::NAN, |(x, y)| euclidean_gcd(x, y) as f64);
				Some(Value::Number(Number::Real(gcd)))
			}
			_ => None,
		},

		"lcm" => |values| match values {
			[Value::Number(Number::Real(a)), Value::Number(Number::Real(b))] => {
				let Some((x, y)) = integer_operands(*a, *b) else {
					return Some(Value::Number(Number::Real(f64::NAN)));
				};
				if x == 0 || y == 0 {
					return Some(Value::Number(Number::Real(0.)));
				}

				// Multiply in f64 so huge results can't overflow the integer range
				let lcm = (x / euclidean_gcd(x, y)) as f64 * y as f64;
				Some(Value::Number(Number::Real(lcm)))
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

		// Logical Functions
		"isnan" => |values| match values {
			[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(if real.is_nan() { 1. } else { 0. }))),
			_ => None,
		},

		"eq" => |values| match values {
			[Value::Number(a), Value::Number(b)] => Some(Value::Number(Number::Real(if a == b { 1. } else { 0. }))),
			_ => None,
		},

		"greater" => |values| match values {
			[Value::Number(Number::Real(a)), Value::Number(Number::Real(b))] => Some(Value::Number(Number::Real(if a > b { 1. } else { 0. }))),
			_ => None,
		},
		_ => return None,
	})
}
