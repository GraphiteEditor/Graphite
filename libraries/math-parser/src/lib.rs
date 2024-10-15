#[macro_use]
extern crate log;

mod ast;
mod constants;
mod context;
mod executer;
mod parser;
mod value;

use context::{EvalContext, ValueMap};
use executer::EvalError;
use parser::ParseError;
use value::Value;

pub fn evaluate(expression: &str) -> Result<Result<Value, EvalError>, ParseError> {
	debug!("Evaluating expression {expression}");
	let expr = ast::Node::from_str(expression);
	dbg!(&expr);
	let context = EvalContext::default();
	expr.map(|node| node.eval(&context))
}

#[cfg(test)]
mod tests {
	use value::Number;

	use super::*;
	const EPSILON: f64 = 1e10_f64;

	#[track_caller]
	fn end_to_end(expression: &str, expected: impl Into<Value>) {
		let actual = evaluate(expression).unwrap().unwrap();
		let expected = expected.into();

		match (actual, expected) {
			// Compare Complex<f64>
			(Value::Number(Number::Complex(actual_c)), Value::Number(Number::Complex(expected_c))) => {
				assert!(
					(actual_c.re.is_infinite() && expected_c.re.is_infinite()) || (actual_c.re - expected_c.re).abs() < EPSILON,
					"Expected real part {}, but got {}",
					expected_c.re,
					actual_c.re
				);
				assert!(
					(actual_c.im.is_infinite() && expected_c.im.is_infinite()) || (actual_c.im - expected_c.im).abs() < EPSILON,
					"Expected imaginary part {}, but got {}",
					expected_c.im,
					actual_c.im
				);
			}
			// Compare Number::Real(f64)
			(Value::Number(Number::Real(actual_f)), Value::Number(Number::Real(expected_f))) => {
				if actual_f.is_infinite() || expected_f.is_infinite() {
					assert!(
						actual_f.is_infinite() && expected_f.is_infinite() && actual_f == expected_f,
						"Expected infinite value {}, but got {}",
						expected_f,
						actual_f
					);
				} else if actual_f.is_nan() || expected_f.is_nan() {
					assert!(actual_f.is_nan() && expected_f.is_nan(), "Expected NaN, but got {}", actual_f);
				} else {
					assert!((actual_f - expected_f).abs() < EPSILON, "Expected {}, but got {}", expected_f, actual_f);
				}
			}
			// Handle mismatched types
			_ => panic!("Mismatched types: expected {:?}, got {:?}", expected, actual),
		}
	}
	#[test]
	fn simple_infix() {
		end_to_end("5 + 5", 10.);
		end_to_end("5 - 3", 2.);
		end_to_end("4*4", 16.);
		end_to_end("8/2", 4.);
	}
	#[test]
	fn simple_prefix() {
		end_to_end("-10", -10.);
		end_to_end("sqrt25", 5.);
		end_to_end("sqrt(25)", 5.);
	}
	#[test]
	fn order_of_operations() {
		end_to_end("-10 + 5", -5.);
		end_to_end("5+1*1+5", 11.);
		end_to_end("5+(-1)*1+5", 9.);
		end_to_end("sqrt25 + 11", 16.);
		end_to_end("sqrt(25+11)", 6.);
	}

	#[test]
	fn nested_operations_with_parentheses() {
		end_to_end("(5 + 3) * (2 + 6)", 64.);
		end_to_end("2 * (3 + 5 * (2 + 1))", 36.);
		end_to_end("10 / (2 + 3) + (7 * 2)", 16.);
	}

	#[test]
	fn multiple_nested_functions() {
		end_to_end("sqrt(16) + sqrt(9) * sqrt(4)", 10.);
		end_to_end("sqrt(sqrt(81))", 3.);
		end_to_end("sqrt((25 + 11) / 9)", 2.);
	}

	#[test]
	fn mixed_operations_with_functions() {
		end_to_end("sqrt(16) * 2 + 5", 13.);
		end_to_end("sqrt(49) - 1 + 2 * 3", 12.);
		end_to_end("(sqrt(36) + 2) * 2", 16.);
	}

	#[test]
	fn exponentiation_operations() {
		end_to_end("2^3", 8.);
		end_to_end("2^3 + 4^2", 24.);
		end_to_end("2^(3+1)", 16.);
	}

	#[test]
	fn order_of_operations_with_negatives() {
		end_to_end("-5 + (-3 * 2)", -11.);
		end_to_end("-(5 + 3 * (2 - 1))", -8.);
		end_to_end("-(sqrt(16) + sqrt(9))", -7.);
	}

	#[test]
	fn combining_different_operation_types() {
		end_to_end("5 * 2 + sqrt(16) / 2 - 3", 9.);
		end_to_end("4 + 3 * (2 + 1) - sqrt(25)", 8.);
		end_to_end("10 + sqrt(64) - (5 * (2 + 1))", 3.);
	}

	#[test]
	fn constants() {
		end_to_end("pi", std::f64::consts::PI);
		end_to_end("e", std::f64::consts::E);
		end_to_end("phi", 1.61803398875); // Approx. golden ratio
		end_to_end("tau", 2.0 * std::f64::consts::PI);
		end_to_end("inf", f64::INFINITY);
		end_to_end("∞", f64::INFINITY);
	}

	#[test]
	fn constants_with_operations() {
		end_to_end("2 * pi", 2.0 * std::f64::consts::PI);
		end_to_end("e + 1", std::f64::consts::E + 1.0);
		end_to_end("phi * 2", 1.61803398875 * 2.0);
		end_to_end("2^tau", 2f64.powf(2.0 * std::f64::consts::PI));
		end_to_end("inf - 1000", f64::INFINITY); // Infinity stays infinity
	}

	#[test]
	fn trig_with_constants() {
		end_to_end("sin(pi)", 0.0);
		end_to_end("cos(0)", 1.0);
		end_to_end("tan(pi/4)", 1.0);
		end_to_end("sin(tau)", 0.0);
		end_to_end("cos(tau/2)", -1.0);
	}

	#[test]
	fn complex_operations_with_constants() {
		end_to_end("2 * sin(pi/2) + cos(0)", 3.0); // sin(pi/2) = 1, cos(0) = 1
		end_to_end("sqrt(pi) + tau / 2", std::f64::consts::PI.sqrt() + std::f64::consts::PI);
		end_to_end("e^(pi - 1)", std::f64::consts::E.powf(std::f64::consts::PI - 1.0));
		end_to_end("sqrt(inf)", f64::INFINITY); // sqrt(inf) = inf
	}

	#[test]
	fn trig_with_negative_constants() {
		end_to_end("sin(-pi)", 0.0);
		end_to_end("cos(-pi)", -1.0);
		end_to_end("tan(-pi/4)", -1.0);
	}
}
