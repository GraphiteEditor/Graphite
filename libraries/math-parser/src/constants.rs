use std::f64::consts::PI;

use num_complex::{Complex, ComplexFloat};

use crate::value::{Number, Value};

pub fn default_functions(name: &str, values: &[Value]) -> Option<Value> {
	if values.len() != 1 {
		return None; // We expect exactly one value as input
	}

	match &values[0] {
		Value::Number(Number::Real(real)) => match name {
			"sin" => Some(Value::Number(Number::Real(real.sin()))),
			"cos" => Some(Value::Number(Number::Real(real.cos()))),
			"tan" => Some(Value::Number(Number::Real(real.tan()))),
			"csc" => Some(Value::Number(Number::Real(real.sin().recip()))),
			"sec" => Some(Value::Number(Number::Real(real.cos().recip()))),
			"cot" => Some(Value::Number(Number::Real(real.tan().recip()))),

			"invsin" => Some(Value::Number(Number::Real(real.asin()))),
			"invcos" => Some(Value::Number(Number::Real(real.acos()))),
			"invtan" => Some(Value::Number(Number::Real(real.atan()))),
			"invcsc" => Some(Value::Number(Number::Real(real.recip().asin()))),
			"invsec" => Some(Value::Number(Number::Real(real.recip().acos()))),
			"invcot" => Some(Value::Number(Number::Real((PI / 2.0 - real).atan()))),

			_ => None, // Handle unknown function names
		},

		Value::Number(Number::Complex(complex)) => match name {
			"sin" => Some(Value::Number(Number::Complex(complex.sin()))),
			"cos" => Some(Value::Number(Number::Complex(complex.cos()))),
			"tan" => Some(Value::Number(Number::Complex(complex.tan()))),
			"csc" => Some(Value::Number(Number::Complex(complex.sin().recip()))),
			"sec" => Some(Value::Number(Number::Complex(complex.cos().recip()))),
			"cot" => Some(Value::Number(Number::Complex(complex.tan().recip()))),

			"invsin" => Some(Value::Number(Number::Complex(complex.asin()))),
			"invcos" => Some(Value::Number(Number::Complex(complex.acos()))),
			"invtan" => Some(Value::Number(Number::Complex(complex.atan()))),
			"invcsc" => Some(Value::Number(Number::Complex(complex.recip().asin()))),
			"invsec" => Some(Value::Number(Number::Complex(complex.recip().acos()))),
			"invcot" => Some(Value::Number(Number::Complex((Complex::new(PI / 2.0, 0.0) - complex).atan()))),

			_ => None, // Handle unknown function names
		},

		_ => None, // Handle cases where the value is not a number
	}
}
