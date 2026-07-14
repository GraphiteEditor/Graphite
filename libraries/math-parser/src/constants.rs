use crate::value::{Number, Value};
use lazy_static::lazy_static;
use num_complex::{Complex, ComplexFloat};
use std::collections::HashMap;
use std::f64::consts::{LN_2, PI};

type FunctionImplementation = Box<dyn Fn(&[Value]) -> Option<Value> + Send + Sync>;
lazy_static! {
	pub static ref DEFAULT_FUNCTIONS: HashMap<&'static str, FunctionImplementation> = {
		let mut map: HashMap<&'static str, FunctionImplementation> = HashMap::new();

		map.insert(
			"sqrt",
			Box::new(|values| match values{
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.sqrt()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.sqrt()))),
				_ => None,
			})
		);
		map.insert(
			"sin",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.sin()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.sin()))),
				_ => None,
			}),
		);

		map.insert(
			"cos",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.cos()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.cos()))),
				_ => None,
			}),
		);

		map.insert(
			"tan",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.tan()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.tan()))),
				_ => None,
			}),
		);

		map.insert(
			"csc",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.sin().recip()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.sin().recip()))),
				_ => None,
			}),
		);

		map.insert(
			"sec",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.cos().recip()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.cos().recip()))),
				_ => None,
			}),
		);

		map.insert(
			"cot",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.tan().recip()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.tan().recip()))),
				_ => None,
			}),
		);

		// Inverse trig with legacy names and standard aliases
		map.insert(
			"invsin",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.asin()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.asin()))),
				_ => None,
			}),
		);
		map.insert(
			"asin",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.asin()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.asin()))),
				_ => None,
			}),
		);

		map.insert(
			"invcos",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.acos()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.acos()))),
				_ => None,
			}),
		);
		map.insert(
			"acos",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.acos()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.acos()))),
				_ => None,
			}),
		);

		map.insert(
			"invtan",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.atan()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.atan()))),
				_ => None,
			}),
		);
		map.insert(
			"atan",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.atan()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.atan()))),
				_ => None,
			}),
		);

		map.insert(
			"invcsc",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().asin()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().asin()))),
				_ => None,
			}),
		);
		map.insert(
			"acsc",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().asin()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().asin()))),
				_ => None,
			}),
		);

		map.insert(
			"invsec",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().acos()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().acos()))),
				_ => None,
			}),
		);
		map.insert(
			"asec",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().acos()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().acos()))),
				_ => None,
			}),
		);

		map.insert(
			"invcot",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => {
					Some(Value::Number(Number::Real(real.recip().atan())))
				}
				[Value::Number(Number::Complex(complex))] => {
					Some(Value::Number(Number::Complex(complex.recip().atan())))
				}
				_ => None,
			}),
		);
		map.insert(
			"acot",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => {
					Some(Value::Number(Number::Real(real.recip().atan())))
				}
				[Value::Number(Number::Complex(complex))] => {
					Some(Value::Number(Number::Complex(complex.recip().atan())))
				}
				_ => None,
			}),
		);
		// Hyperbolic Functions
		map.insert(
			"sinh",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.sinh()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.sinh()))),
				_ => None,
			}),
		);

		map.insert(
			"cosh",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.cosh()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.cosh()))),
				_ => None,
			}),
		);

		map.insert(
			"tanh",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.tanh()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.tanh()))),
				_ => None,
			}),
		);

		// Reciprocal hyperbolic functions
		map.insert(
			"csch",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.sinh().recip()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.sinh().recip()))),
				_ => None,
			}),
		);

		map.insert(
			"sech",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.cosh().recip()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.cosh().recip()))),
				_ => None,
			}),
		);

		map.insert(
			"coth",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.tanh().recip()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.tanh().recip()))),
				_ => None,
			}),
		);

		// Inverse Hyperbolic Functions
		map.insert(
			"asinh",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.asinh()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.asinh()))),
				_ => None,
			}),
		);

		map.insert(
			"acosh",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.acosh()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.acosh()))),
				_ => None,
			}),
		);

		map.insert(
			"atanh",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.atanh()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.atanh()))),
				_ => None,
			}),
		);

		// Inverse reciprocal hyperbolic functions
		map.insert(
			"acsch",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().asinh()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().asinh()))),
				_ => None,
			}),
		);

		map.insert(
			"asech",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().acosh()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().acosh()))),
				_ => None,
			}),
		);

		map.insert(
			"acoth",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().atanh()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().atanh()))),
				_ => None,
			}),
		);

		// Logarithm Functions
		map.insert(
			"ln",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.ln()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.ln()))),
				_ => None,
			}),
		);

		// Exponential / power helpers
		map.insert(
			"exp",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.exp()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.exp()))),
				_ => None,
			}),
		);

		map.insert(
			"pow",
			Box::new(|values| match values {
				[Value::Number(Number::Real(x)), Value::Number(Number::Real(n))] => {
					Some(Value::Number(Number::Real(x.powf(*n))))
				}
				[Value::Number(Number::Complex(x)), Value::Number(Number::Real(n))] => {
					Some(Value::Number(Number::Complex(x.powf(*n))))
				}
				[Value::Number(Number::Complex(x)), Value::Number(Number::Complex(n))] => {
					Some(Value::Number(Number::Complex(x.powc(*n))))
				}
				_ => None,
			}),
		);

		map.insert(
			"root",
			Box::new(|values| match values {
				[Value::Number(Number::Real(x)), Value::Number(Number::Real(n))] => {
					Some(Value::Number(Number::Real(x.powf(1.0 / *n))))
				}
				[Value::Number(Number::Complex(x)), Value::Number(Number::Real(n))] => {
					Some(Value::Number(Number::Complex(x.powf(1.0 / *n))))
				}
				_ => None,
			}),
		);

		map.insert(
			"log",
			Box::new(|values| match values {
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
			}),
		);

		map.insert(
			"log2",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.log2()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex / LN_2))),
				_ => None,
			}),
		);

		// Root Functions
		map.insert(
			"sqrt",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.sqrt()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.sqrt()))),
				_ => None,
			}),
		);

		map.insert(
			"cbrt",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.cbrt()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.powf(1.0/3.0)))),
				_ => None,
			}),
		);

		// Geometry Functions
		map.insert(
			"hypot",
			Box::new(|values| match values {
				[Value::Number(Number::Real(a)), Value::Number(Number::Real(b))] => {
					Some(Value::Number(Number::Real(a.hypot(*b))))
				},
				_ => None,
			}),
		);

		map.insert(
			"atan2",
			Box::new(|values| match values {
				[Value::Number(Number::Real(y)), Value::Number(Number::Real(x))] => {
					Some(Value::Number(Number::Real(y.atan2(*x))))
				}
				_ => None,
			}),
		);

		// Mapping Functions
		map.insert(
			"abs",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.abs()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Real(complex.abs()))),
				_ => None,
			}),
		);

		map.insert(
			"floor",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.floor()))),
				_ => None,
			}),
		);

		map.insert(
			"ceil",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.ceil()))),
				_ => None,
			}),
		);

		map.insert(
			"round",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.round()))),
				_ => None,
			}),
		);

		map.insert(
			"clamp",
			Box::new(|values| match values {
				[Value::Number(Number::Real(x)), Value::Number(Number::Real(min)), Value::Number(Number::Real(max))] => {
					Some(Value::Number(Number::Real(x.clamp(*min, *max))))
				},
				_ => None,
			}),
		);

		map.insert(
			"lerp",
			Box::new(|values| match values {
				[Value::Number(Number::Real(a)), Value::Number(Number::Real(b)), Value::Number(Number::Real(t))] => {
					Some(Value::Number(Number::Real(a + (b - a) * t)))
				},
				_ => None,
			}),
		);

		map.insert(
			"remap",
			Box::new(|values| match values {
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
			}),
		);

		map.insert(
			"trunc",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.trunc()))),
				_ => None,
			}),
		);

		map.insert(
			"fract",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.fract()))),
				_ => None,
			}),
		);

		map.insert(
			"sign",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => {
					let s = if *real > 0.0 {
						1.0
					} else if *real < 0.0 {
						-1.0
					} else {
						0.0
					};
					Some(Value::Number(Number::Real(s)))
				}
				_ => None,
			}),
		);

		map.insert(
			"gcd",
			Box::new(|values| match values {
				[Value::Number(Number::Real(a)), Value::Number(Number::Real(b))] => {
					let mut x = a.trunc() as i64;
					let mut y = b.trunc() as i64;
					if x == 0 && y == 0 {
						return Some(Value::Number(Number::Real(0.0)));
					}
					x = x.abs();
					y = y.abs();
					while y != 0 {
						let r = x % y;
						x = y;
						y = r;
					}
					Some(Value::Number(Number::Real(x as f64)))
				}
				_ => None,
			}),
		);

		map.insert(
			"lcm",
			Box::new(|values| match values {
				[Value::Number(Number::Real(a)), Value::Number(Number::Real(b))] => {
					let mut x = a.trunc() as i64;
					let mut y = b.trunc() as i64;
					x = x.abs();
					y = y.abs();
					if x == 0 || y == 0 {
						return Some(Value::Number(Number::Real(0.0)));
					}

					// gcd
					let mut gx = x;
					let mut gy = y;
					while gy != 0 {
						let r = gx % gy;
						gx = gy;
						gy = r;
					}
					let lcm = (x / gx) * y;
					Some(Value::Number(Number::Real(lcm as f64)))
				}
				_ => None,
			}),
		);

		// Complex Number Functions
		map.insert(
			"real",
			Box::new(|values| match values {
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Real(complex.re))),
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(*real))),
				_ => None,
			}),
		);

		map.insert(
			"imag",
			Box::new(|values| match values {
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Real(complex.im))),
				[Value::Number(Number::Real(_))] => Some(Value::Number(Number::Real(0.0))),
				_ => None,
			}),
		);

		map.insert(
			"conj",
			Box::new(|values| match values {
				[Value::Number(Number::Complex(complex))] => {
					Some(Value::Number(Number::Complex(complex.conj())))
				}
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(*real))),
				_ => None,
			}),
		);

		map.insert(
			"arg",
			Box::new(|values| match values {
				[Value::Number(Number::Complex(complex))] => {
					Some(Value::Number(Number::Real(complex.arg())))
				}
				[Value::Number(Number::Real(real))] => {
					let angle = if *real >= 0.0 { 0.0 } else { PI };
					Some(Value::Number(Number::Real(angle)))
				}
				_ => None,
			}),
		);

		// Logical Functions
		map.insert(
			"isnan",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(if real.is_nan() { 1.0 } else { 0.0 }))),
				_ => None,
			}),
		);

		map.insert(
			"eq",
			Box::new(|values| match values {
				[Value::Number(a), Value::Number(b)] => Some(Value::Number(Number::Real(if a == b { 1.0 } else { 0.0 }))),
				_ => None,
			}),
		);

		map.insert(
			"greater",
			Box::new(|values| match values {
				[Value::Number(Number::Real(a)), Value::Number(Number::Real(b))] => {
					Some(Value::Number(Number::Real(if a > b { 1.0 } else { 0.0 })))
				},
				_ => None,
			}),
		);

		map
	};
}
