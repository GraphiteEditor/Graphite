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

		map.insert(
			"invsin",
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
			"invtan",
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
			"invsec",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().acos()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().acos()))),
				_ => None,
			}),
		);

		map.insert(
			"invcot",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real((PI / 2.0 - real).atan()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex((Complex::new(PI / 2.0, 0.0) - complex).atan()))),
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

		// Logarithm Functions
		map.insert(
			"ln",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.ln()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.ln()))),
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
