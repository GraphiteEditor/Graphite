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

		map
	};
}
