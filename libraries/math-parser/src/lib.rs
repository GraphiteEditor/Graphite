#![allow(unused)]

pub mod ast;
mod constants;
pub mod context;
pub mod executer;
pub mod parser;
pub mod value;

use ast::Unit;
use context::{EvalContext, ValueMap};
use executer::EvalError;
use parser::ParseError;
use value::Value;

pub fn evaluate(expression: &str) -> Result<(Result<Value, EvalError>, Unit), ParseError> {
	let expr = ast::Node::try_parse_from_str(expression);
	let context = EvalContext::default();
	expr.map(|(node, unit)| (node.eval(&context), unit))
}

#[cfg(test)]
mod tests {
	use super::*;
	use ast::Unit;
	use value::Number;

	const EPSILON: f64 = 1e-10_f64;

	macro_rules! test_end_to_end{
		($($name:ident: $input:expr_2021 => ($expected_value:expr_2021, $expected_unit:expr_2021)),* $(,)?) => {
			$(
				#[test]
				fn $name() {
					let expected_value = $expected_value;
					let expected_unit = $expected_unit;

					let expr = ast::Node::try_parse_from_str($input);
					let context = EvalContext::default();

					let (actual_value, actual_unit) = expr.map(|(node, unit)| (node.eval(&context), unit)).unwrap();
					let actual_value = actual_value.unwrap();


					assert!(actual_unit == expected_unit, "Expected unit {:?} but found unit {:?}", expected_unit, actual_unit);

					let expected_value = expected_value.into();

					match (actual_value, expected_value) {
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
						_ => panic!("Mismatched types: expected {:?}, got {:?}", expected_value, actual_value),
					}

				}
			)*
		};
	}

	test_end_to_end! {
		// Basic arithmetic and units
		infix_addition: "5 + 5" => (10., Unit::BASE_UNIT),
		infix_subtraction_units: "5m - 3m" => (2., Unit::LENGTH),
		infix_multiplication_units: "4s * 4s" => (16., Unit { length: 0, mass: 0, time: 2 }),
		infix_division_units: "8m/2s" => (4., Unit::VELOCITY),

		// Order of operations
		order_of_operations_negative_prefix: "-10 + 5" => (-5., Unit::BASE_UNIT),
		order_of_operations_add_multiply: "5+1*1+5" => (11., Unit::BASE_UNIT),
		order_of_operations_add_negative_multiply: "5+(-1)*1+5" => (9., Unit::BASE_UNIT),
		order_of_operations_sqrt: "sqrt25 + 11" => (16., Unit::BASE_UNIT),
		order_of_operations_sqrt_expression: "sqrt(25+11)" => (6., Unit::BASE_UNIT),

		// Parentheses and nested expressions
		parentheses_nested_multiply: "(5 + 3) * (2 + 6)" => (64., Unit::BASE_UNIT),
		parentheses_mixed_operations: "2 * (3 + 5 * (2 + 1))" => (36., Unit::BASE_UNIT),
		parentheses_divide_add_multiply: "10 / (2 + 3) + (7 * 2)" => (16., Unit::BASE_UNIT),

		// Square root and nested square root
		sqrt_chain_operations: "sqrt(16) + sqrt(9) * sqrt(4)" => (10., Unit::BASE_UNIT),
		sqrt_nested: "sqrt(sqrt(81))" => (3., Unit::BASE_UNIT),
		sqrt_divide_expression: "sqrt((25 + 11) / 9)" => (2., Unit::BASE_UNIT),

		// Mixed square root and units
		sqrt_multiply_units: "sqrt(16) * 2g + 5g" => (13., Unit::MASS),
		sqrt_add_multiply: "sqrt(49) - 1 + 2 * 3" => (12., Unit::BASE_UNIT),
		sqrt_addition_multiply: "(sqrt(36) + 2) * 2" => (16., Unit::BASE_UNIT),

		// Exponentiation
		exponent_single: "2^3" => (8., Unit::BASE_UNIT),
		exponent_mixed_operations: "2^3 + 4^2" => (24., Unit::BASE_UNIT),
		exponent_nested: "2^(3+1)" => (16., Unit::BASE_UNIT),

		// Operations with negative values
		negative_units_add_multiply: "-5s + (-3 * 2)s" => (-11., Unit::TIME),
		negative_nested_parentheses: "-(5 + 3 * (2 - 1))" => (-8., Unit::BASE_UNIT),
		negative_sqrt_addition: "-(sqrt(16) + sqrt(9))" => (-7., Unit::BASE_UNIT),
		multiply_sqrt_subtract: "5 * 2 + sqrt(16) / 2 - 3" => (9., Unit::BASE_UNIT),
		add_multiply_subtract_sqrt: "4 + 3 * (2 + 1) - sqrt(25)" => (8., Unit::BASE_UNIT),
		add_sqrt_subtract_nested_multiply: "10 + sqrt(64) - (5 * (2 + 1))" => (3., Unit::BASE_UNIT),

		// Mathematical constants
		constant_pi: "pi" => (std::f64::consts::PI, Unit::BASE_UNIT),
		constant_e: "e" => (std::f64::consts::E, Unit::BASE_UNIT),
		constant_phi: "phi" => (1.61803398875, Unit::BASE_UNIT),
		constant_tau: "tau" => (2.0 * std::f64::consts::PI, Unit::BASE_UNIT),
		constant_infinity: "inf" => (f64::INFINITY, Unit::BASE_UNIT),
		constant_infinity_symbol: "∞" => (f64::INFINITY, Unit::BASE_UNIT),
		multiply_pi: "2 * pi" => (2.0 * std::f64::consts::PI, Unit::BASE_UNIT),
		add_e_constant: "e + 1" => (std::f64::consts::E + 1.0, Unit::BASE_UNIT),
		multiply_phi_constant: "phi * 2" => (1.61803398875 * 2.0, Unit::BASE_UNIT),
		exponent_tau: "2^tau" => (2f64.powf(2.0 * std::f64::consts::PI), Unit::BASE_UNIT),
		infinity_subtract_large_number: "inf - 1000" => (f64::INFINITY, Unit::BASE_UNIT),

		// Trigonometric functions
		trig_sin_pi: "sin(pi)" => (0.0, Unit::BASE_UNIT),
		trig_cos_zero: "cos(0)" => (1.0, Unit::BASE_UNIT),
		trig_tan_pi_div_four: "tan(pi/4)" => (1.0, Unit::BASE_UNIT),
		trig_sin_tau: "sin(tau)" => (0.0, Unit::BASE_UNIT),
		trig_cos_tau_div_two: "cos(tau/2)" => (-1.0, Unit::BASE_UNIT),
	}
}
