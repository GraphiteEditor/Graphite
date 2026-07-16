use crate::value::{Number, Value};
use lazy_static::lazy_static;
use num_complex::{Complex, ComplexFloat};
use std::collections::HashMap;
use std::f64::consts::PI;

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
			"asin",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.asin()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.asin()))),
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
			"atan",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.atan()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.atan()))),
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
			"asec",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.recip().acos()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.recip().acos()))),
				_ => None,
			}),
		);

		map.insert(
			"acot",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real((PI / 2. - real).atan()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex((Complex::new(PI / 2., 0.) - complex).atan()))),
				_ => None,
			}),
		);

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
				_ => None,
			}),
		);

		map.insert(
			"exp",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.exp()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Complex(complex.exp()))),
				_ => None,
			}),
		);

		map.insert(
			"abs",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.abs()))),
				[Value::Number(Number::Complex(complex))] => Some(Value::Number(Number::Real(complex.norm()))),
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
			"min",
			Box::new(|values| match values {
				[Value::Number(Number::Real(a)), Value::Number(Number::Real(b))] => Some(Value::Number(Number::Real(a.min(*b)))),
				_ => None,
			}),
		);

		map.insert(
			"max",
			Box::new(|values| match values {
				[Value::Number(Number::Real(a)), Value::Number(Number::Real(b))] => Some(Value::Number(Number::Real(a.max(*b)))),
				_ => None,
			}),
		);

		map.insert(
			"atan2",
			Box::new(|values| match values {
				[Value::Number(Number::Real(y)), Value::Number(Number::Real(x))] => Some(Value::Number(Number::Real(y.atan2(*x)))),
				_ => None,
			}),
		);

		map.insert(
			"sign",
			Box::new(|values| match values {
				[Value::Number(Number::Real(real))] => Some(Value::Number(Number::Real(real.signum()))),
				_ => None,
			}),
		);

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

		map
	};
}
