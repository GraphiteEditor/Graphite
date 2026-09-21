pub mod ast;
pub mod constants;
pub mod context;
pub mod executer;
pub mod lexer;
pub mod parser;
pub mod value;

use context::EvalContext;
use executer::EvalError;
use parser::ParseError;
use value::Value;

pub fn evaluate(expression: &str) -> Result<Result<Value, EvalError>, ParseError> {
	let expr = ast::Node::try_parse_from_str(expression);
	let context = EvalContext::default();
	expr.map(|node| node.eval(&context))
}

#[cfg(test)]
mod tests {
	use super::*;
	use value::{Complex, Number};

	const EPSILON: f64 = 1e-10_f64;

	#[test]
	fn malformed_juxtaposed_numbers_fail_to_parse() {
		// Two numbers cannot be glued together by a stray decimal point (they must not parse as implicit multiplication)
		for input in ["1..5", "1.5.5", "1..", ".5.5"] {
			assert!(evaluate(input).is_err(), "expected `{input}` to be a parse error");
		}
	}

	#[test]
	fn unrecognized_characters_fail_to_parse() {
		// Unrecognized trailing input must be rejected rather than silently dropped after a valid prefix
		for input in ["2@", "5#", "2 $ 3", "sqrt(4)@", "5 & 3", "5 | 3", "2 = 3", "\\", "2 \\ 3", "\\2", "\\_foo"] {
			assert!(evaluate(input).is_err(), "expected `{input}` to be a parse error");
		}
	}

	#[test]
	fn not_sign_is_prefix_only() {
		// `¬` spells only the prefix logical not, so it must not stand in for `!` in its postfix factorial role
		for input in ["5¬", "5¬3", "(2 + 3)¬", "3¬¬"] {
			assert!(evaluate(input).is_err(), "expected `{input}` to be a parse error");
		}
	}

	#[test]
	fn juxtaposed_numbers_fail_to_parse() {
		// Adjacent number literals like digit-grouped `10 000` must not silently multiply, and a `.`-led literal after any operand needs its leading zero
		for input in ["2 3", "10 000", "1 .5", "2 3 + 1", "1e5 3", "2. 3", "sqrt(4).5", "sqrt(4) .5", "pi.5", "5!.5"] {
			assert!(evaluate(input).is_err(), "expected `{input}` to be a parse error");
		}
	}

	#[test]
	fn statistics_without_an_answer_are_errors() {
		// No value repeats, so no mode exists, and negative operands have no geometric or harmonic mean
		for input in ["mode(1, 2, 3)", "geomean(-1, 4)", "harmmean(1, -1)"] {
			assert!(evaluate(input).unwrap().is_err(), "expected `{input}` to be an evaluation error");
		}

		// A single value has no spread to estimate a sample from
		for input in ["variance(5)", "stdev(5)"] {
			assert!(evaluate(input).unwrap().is_err(), "expected `{input}` to be an evaluation error");
		}
	}

	#[test]
	fn statistics_avoid_intermediate_overflow_and_underflow() {
		// Each result fits in f64 even though naively summing, squaring, or taking reciprocals of the operands would not
		for (input, expected) in [
			("mean(1e308, 1e308)", 1e308),
			("median(1e308, 1e308)", 1e308),
			("rms(1e308)", 1e308),
			("rms(1e-200)", 1e-200),
			("stdevpop(1e-200, -1e-200)", 1e-200),
			("variancepop(1e308, 1e308)", 0.),
			("stdevpop(1e308, -1e308)", 1e308),
			("harmmean(1e-308, 1e-308)", 1e-308),
			("lerp(-1e308, 1e308, 0.5)", 0.),
			("lerp(-1e308, 1e308, 1)", 1e308),
			("remap(0, -1e308, 1e308, 0, 1)", 0.5),
			("remap(0.5, 0, 1, -1e308, 1e308)", 0.),
		] {
			assert_eq!(evaluate(input).unwrap().unwrap().as_real(), Some(expected), "`{input}`");
		}

		// Three large operands whose least common multiple exceeds integer storage must not wrap around
		let input = "lcm(9007199254740992, 9007199254740991, 9007199254740990)";
		assert!(!matches!(evaluate(input), Ok(Ok(value)) if value.as_real().is_some_and(f64::is_finite)), "`{input}`");
	}

	#[test]
	fn harmonic_mean_shortcuts_keep_an_invalid_operand_invalid() {
		// The zero and infinity shortcuts must not turn an operand with no real value into a number
		for input in ["harmmean(sqrt(-1), 0)", "harmmean(sqrt(-1), inf)"] {
			assert!(!matches!(evaluate(input), Ok(Ok(value)) if value.as_real().is_some_and(|real| !real.is_nan())), "`{input}`");
		}
	}

	#[test]
	fn xor_requires_logical_operands() {
		// Logical operands must be exactly 0 or 1
		for input in ["xor(2, 1)", "xor(0.5, 1)"] {
			assert!(evaluate(input).unwrap().is_err(), "expected `{input}` to be an evaluation error");
		}
	}

	#[test]
	fn bindings_shadow_constants_and_the_prefix_reaches_the_builtin() {
		struct ShadowingBindings;
		impl context::ValueProvider for ShadowingBindings {
			fn get_value(&self, name: &str) -> Option<Value> {
				match name {
					"e" => Some(Value::from_f64(2.5)),
					"π" => Some(Value::from_f64(3.)),
					_ => None,
				}
			}
		}
		let eval = |source: &str| ast::Node::try_parse_from_str(source).unwrap().eval(&EvalContext::new(ShadowingBindings, context::NothingMap));

		// A binding shadows the builtin of exactly its spelling, so a bound `π` shadows the constant while `pi` still reaches the builtin
		assert_eq!(eval("e").unwrap().as_real(), Some(2.5));
		assert_eq!(eval("π + pi").unwrap().as_real(), Some(3. + std::f64::consts::PI));

		// The `\` prefix always reaches the builtin, and names no variable when no builtin has that name
		assert_eq!(eval("\\e + \\π").unwrap().as_real(), Some(std::f64::consts::E + std::f64::consts::PI));
		assert!(matches!(eval("\\foo"), Err(EvalError::MissingValue(name)) if name == "\\foo"));

		// Constants are lowercase-only, so another casing is an unbound variable rather than a spelling of the constant
		assert!(matches!(eval("E"), Err(EvalError::MissingValue(name)) if name == "E"));
	}

	#[test]
	fn host_functions_shadow_builtins_except_behind_the_prefix() {
		struct DoublingSin;
		impl context::FunctionProvider for DoublingSin {
			fn run_function(&self, name: &str, args: &[Value]) -> Option<Value> {
				(name == "sin").then(|| Value::from_f64(2. * args[0].as_real().unwrap()))
			}
		}
		let eval = |source: &str| {
			ast::Node::try_parse_from_str(source)
				.unwrap()
				.eval(&EvalContext::new(context::NothingMap, DoublingSin))
				.unwrap()
				.as_real()
		};

		assert_eq!(eval("sin(3)"), Some(6.));
		assert_eq!(eval("\\sin(pi / 2)"), Some(1.));
	}

	#[test]
	fn dot_led_function_suffixes_fail_to_parse() {
		// A `.`-led base suffix is an error (the supported spelling is `log0.5`)
		for input in ["log.5(8)", "log.5", "root.5(9)", "log_.5(8)"] {
			assert!(evaluate(input).is_err(), "expected `{input}` to be a parse error");
		}
	}

	#[test]
	fn scientific_function_suffixes_are_not_bases() {
		struct ScientificName;
		impl context::ValueProvider for ScientificName {
			fn get_value(&self, name: &str) -> Option<Value> {
				(name == "log2e5").then(|| Value::from_f64(10.))
			}
		}

		// A base is plain decimal, so `log2e5(8)` reads as the variable `log2e5` times 8, never a base-200000 log
		let result = ast::Node::try_parse_from_str("log2e5(8)").unwrap().eval(&EvalContext::new(ScientificName, context::NothingMap));
		assert_eq!(result.unwrap().as_real(), Some(80.));
	}

	#[test]
	fn extremely_long_fraction_parses() {
		let input = format!("0.{}", "1".repeat(320));
		let value = evaluate(&input).unwrap().unwrap();
		assert_eq!(value.as_real(), Some(1. / 9.));
	}

	fn run_end_to_end_test(input: &str, expected_value: Value) {
		let expr = match ast::Node::try_parse_from_str(input) {
			Ok(expr) => expr,
			Err(err) => panic!("failed to parse `{input}`: {err}"),
		};
		let context = EvalContext::default();

		let actual_value = match expr.eval(&context) {
			Ok(v) => v,
			Err(err) => panic!("failed to evaluate `{input}` because of error {err}"),
		};

		match (actual_value, expected_value) {
			(Value::Number(Number::Complex(a)), Value::Number(Number::Complex(e))) => {
				// real part
				if a.re.is_infinite() || e.re.is_infinite() {
					assert!(a.re == e.re, "`{}` → real part: expected {:?}, got {:?}", input, e.re, a.re);
				} else {
					assert!((a.re - e.re).abs() < EPSILON, "`{}` → real part: expected {}, got {}", input, e.re, a.re);
				}

				// imag part
				if a.im.is_infinite() || e.im.is_infinite() {
					assert!(a.im == e.im, "`{}` → imag part: expected {:?}, got {:?}", input, e.im, a.im);
				} else {
					assert!((a.im - e.im).abs() < EPSILON, "`{}` → imag part: expected {}, got {}", input, e.im, a.im);
				}
			}

			(Value::Number(Number::Real(a)), Value::Number(Number::Real(e))) => {
				if a.is_infinite() || e.is_infinite() {
					// both must be infinite and equal (i.e. both +∞ or both −∞)
					assert!(a == e, "`{input}` → expected infinite {e:?}, got {a:?}");
				} else if a.is_nan() || e.is_nan() {
					// both must be NaN
					assert!(a.is_nan() && e.is_nan(), "`{input}` → expected NaN, got {a:?}");
				} else {
					let diff = (a - e).abs();
					assert!(diff < EPSILON, "`{input}` → expected {e}, got {a}, Δ={diff}");
				}
			}

			(got, expect) => {
				panic!("`{input}` → mismatched types: expected {expect:?}, got {got:?}");
			}
		}
	}

	macro_rules! test_end_to_end {
		($($name:ident: $input:expr => $expected:expr),* $(,)?) => {
			$(
				#[test]
				fn $name() {
					run_end_to_end_test($input, ($expected).into());
				}
			)*
		};
	}
	test_end_to_end! {
		// Basic arithmetic
		infix_addition: "5 + 5" => 10.,
		infix_subtraction: "5 - 3" => 2.,
		infix_multiplication: "4 * 4" => 16.,
		infix_division: "8/2" => 4.,
		modulo_pos_pos: "3.2 % 2" => 1.2,
		modulo_pos_neg: "3.2 % -2" => 1.2,
		modulo_neg_neg: "(-3.2) % -2" => -1.2,
		modulo_neg_pos: "(-3.2) % 2" => -1.2,
		exp_pos_pos: "3.2 ^ 2" => 256. / 25.,
		exp_pos_neg: "3.2 ^ -2" => 25. / 256.,
		exp_neg_neg: "-3.2 ^ -2" => -25. / 256.,
		exp_neg_pos: "-3.2 ^ 2" => -256. / 25.,

		// Order of operations
		order_of_operations_negative_prefix: "-10 + 5" => -5.,
		order_of_operations_add_multiply: "5+1*1+5" => 11.,
		order_of_operations_add_negative_multiply: "5+(-1)*1+5" => 9.,
		order_of_operations_sqrt: "sqrt(25) + 11" => 16.,
		order_of_operations_sqrt_expression: "sqrt(25+11)" => 6.,

		// Parentheses and nested expressions
		parentheses_nested_multiply: "(5 + 3) * (2 + 6)" => 64.,
		parentheses_mixed_operations: "2 * (3 + 5 * (2 + 1))" => 36.,
		parentheses_divide_add_multiply: "10 / (2 + 3) + (7 * 2)" => 16.,

		// Square root and nested square root
		sqrt_chain_operations: "sqrt(16) + sqrt(9) * sqrt(4)" => 10.,
		sqrt_nested: "sqrt(sqrt(81))" => 3.,
		sqrt_divide_expression: "sqrt((25 + 11) / 9)" => 2.,

		// Mixed square root and units
		sqrt_add_multiply: "sqrt(49) - 1 + 2 * 3" => 12.,
		sqrt_addition_multiply: "(sqrt(36) + 2) * 2" => 16.,

		// Exponentiation
		exponent_single: "2^3" => 8.,
		exponent_mixed_operations: "2^3 + 4^2" => 24.,
		exponent_nested: "2^(3+1)" => 16.,
		exponent_right_associative: "2^2^3" => 256.,
		exponent_unary_operand: "2^-1" => 0.5,

		// Implicit multiplication binds like `*`/`/`: tighter than `+`, looser than `^`, left to right
		implicit_multiplication_constant: "2pi" => 2. * std::f64::consts::PI,
		implicit_multiplication_before_addition: "2pi + 1" => 2. * std::f64::consts::PI + 1.,
		implicit_multiplication_shares_division: "1/2pi" => std::f64::consts::PI / 2.,
		implicit_multiplication_left_to_right: "6/2pi" => 3. * std::f64::consts::PI,
		implicit_multiplication_power_operand: "2pi^2" => 2. * std::f64::consts::PI.powi(2),
		implicit_multiplication_function: "2sqrt(4)" => 4.,
		implicit_multiplication_excludes_unary_minus: "2 -3" => -1.,
		implicit_multiplication_trailing_number: "pi 2" => 2. * std::f64::consts::PI,
		implicit_multiplication_trailing_number_after_call: "sqrt(4) 3" => 6.,
		implicit_multiplication_trailing_number_unspaced: "sqrt(4)3" => 6.,
		implicit_multiplication_trailing_leading_zero: "sqrt(4) 0.5" => 1.,
		implicit_multiplication_trailing_number_after_name_run: "2pi 3" => 6. * std::f64::consts::PI,
		implicit_multiplication_trailing_number_after_factorial: "3! 2" => 12.,
		implicit_multiplication_trailing_number_after_infinity: "∞ 2" => f64::INFINITY,

		// Factorial (postfix !)
		factorial_simple: "5!" => 120.,
		factorial_nested: "(3 + 2)!" => 120.,
		factorial_zero: "0!" => 1.,
		factorial_chain: "3!!" => 720., // (3!)! = 6! = 720

		// Operations with negative values
		negative_nested_parentheses: "-(5 + 3 * (2 - 1))" => -8.,
		negative_sqrt_addition: "-(sqrt(16) + sqrt(9))" => -7.,
		multiply_sqrt_subtract: "5 * 2 + sqrt(16) / 2 - 3" => 9.,
		add_multiply_subtract_sqrt: "4 + 3 * (2 + 1) - sqrt(25)" => 8.,
		add_sqrt_subtract_nested_multiply: "10 + sqrt(64) - (5 * (2 + 1))" => 3.,

		// Mathematical constants
		constant_pi: "pi" => std::f64::consts::PI,
		constant_e: "e" => std::f64::consts::E,
		constant_phi: "phi" => 1.61803398875,
		constant_tau: "tau" => 2. * std::f64::consts::PI,
		constant_infinity: "if(inf == ∞, inf, 0)" => f64::INFINITY,
		multiply_pi: "2 * pi" => 2. * std::f64::consts::PI,
		add_e_constant: "e + 1" => std::f64::consts::E + 1.,
		multiply_phi_constant: "phi * 2" => 1.61803398875 * 2.,
		exponent_tau: "2^tau" => 2f64.powf(2. * std::f64::consts::PI),
		infinity_subtract_large_number: "inf - 1000" => f64::INFINITY,

		// Decimals with no leading digit before the point
		leading_dot_decimal: ".5" => 0.5,
		leading_dot_in_expression: "1+.5" => 1.5,
		leading_dot_exponent: ".5e3" => 500.,

		// Trigonometric functions
		trig_sin_pi: "sin(pi)" => 0.,
		trig_cos_zero: "cos(0)" => 1.,
		trig_tan_pi_div_four: "tan(pi/4)" => 1.,
		trig_sin_tau: "sin(tau)" => 0.,
		trig_cos_tau_div_two: "cos(tau/2)" => -1.,
		trig_csc: "csc(pi/2)" => 1.,
		trig_sec: "sec(0)" => 1.,
		trig_cot: "cot(pi/4)" => 1.,

		// Inverse trig aliases
		inverse_trig_asin: "asin(1)" => std::f64::consts::FRAC_PI_2,
		inverse_trig_acos: "acos(1)" => 0.,
		inverse_trig_atan: "atan(1)" => std::f64::consts::FRAC_PI_4,
		inverse_trig_acsc: "acsc(1)" => std::f64::consts::FRAC_PI_2,
		inverse_trig_asec: "asec(1)" => 0.,
		inverse_trig_acot: "acot(1)" => std::f64::consts::FRAC_PI_4,

		// Hyperbolic and reciprocal hyperbolic
		hyperbolic_sinh: "sinh(0)" => 0.,
		hyperbolic_cosh: "cosh(0)" => 1.,
		hyperbolic_tanh: "tanh(0)" => 0.,
		hyperbolic_csch: "csch(1)" => 1f64.sinh().recip(),
		hyperbolic_sech: "sech(0)" => 1.,
		hyperbolic_coth: "coth(1)" => 1f64.tanh().recip(),

		// Inverse hyperbolic
		inverse_hyperbolic_asinh: "asinh(0)" => 0.,
		inverse_hyperbolic_acosh: "acosh(1)" => 0.,
		inverse_hyperbolic_atanh: "atanh(0)" => 0.,
		inverse_hyperbolic_acsch: "acsch(1)" => 1f64.asinh(),
		inverse_hyperbolic_asech: "asech(1)" => 1f64.acosh(),
		inverse_hyperbolic_acoth: "acoth(2)" => 0.5f64.atanh(),

		// Basic if statements
		if_true_condition: "if(1,5,3)" => 5.,
		if_false_condition: "if(0, 5, 3)" => 3.,

		// Arithmetic conditions
		if_arithmetic_true: "if(2+2-4, 1 , 0)" => 0.,
		if_arithmetic_false: "if(3*2-5, 1, 0)" => 1.,

		// Nested arithmetic
		if_complex_arithmetic: "if((5+3)*(2-1), 10, 20)" => 10.,
		if_with_division: "if(8/4-2 == 0, 15, 25)" => 15.,
		if_with_division_ne: "if(8/4-2 ≠ 0, 15, 25)" => 25.,

		// Constants in conditions
		if_with_pi: "if(pi > 3, 1, 0)" => 1.,
		if_with_e: "if(e < 3, 1, 0)" => 1.,

		// Functions in conditions
		if_with_sqrt: "if(sqrt(16) == 4, 1, 0)" => 1.,
		if_with_sin: "if(sin(pi) == 0.0, 1, 0)" => 0.,

		// Logical NOT (prefix !)
		logical_not_zero: "!0" => 1.,
		logical_not_nonzero: "!5" => 0.,
		logical_not_expression: "!(2 - 2)" => 1.,

		// Log / exp / pow / root
		log_ln: "ln(e)" => 1.,
		log_log10: "log(100)" => 2.,
		log_log2: "log2(8)" => 3.,
		log_change_of_base: "log(8, 2)" => 3.,
		exp_function: "exp(1)" => std::f64::consts::E,
		root_square: "root(9, 2)" => 3.,
		root_cube: "root(8, 3)" => 2.,

		// Nested if statements
		nested_if: "if(1, if(0, 1, 2), 3)" => 2.,
		nested_if_complex: "if(2-2 == 0, if(1, 5, 6), if(1, 7, 8))" => 5.,

		// Mixed operations in conditions and blocks
		if_complex_condition: "if(sqrt(16) + sin(pi) < 5, 2*pi, 3*e)" => 2. * std::f64::consts::PI,
		if_complex_blocks: "if(1, 2*sqrt(16) + sin(pi/2), 3*cos(0) + 4)" => 9.,

		// Mapping helpers
		mapping_trunc: "trunc(3.7)" => 3.,
		mapping_fract: "fract(3.25)" => 0.25,
		mapping_sign_pos: "sign(5)" => 1.,
		mapping_sign_neg: "sign(-5)" => -1.,

		// Geometry / mapping extras
		geometry_hypot: "hypot(3, 4)" => 5.,

		// Minimum and maximum accept two or more arguments
		mapping_min: "min(3, 7)" => 3.,
		mapping_max: "max(3, 7)" => 7.,
		mapping_min_variadic: "min(5, 2, 8, 4)" => 2.,
		mapping_max_variadic: "max(5, 2, 8, 4)" => 8.,
		mapping_min_skips_nan: "min(sqrt(-1), 5)" => 5.,
		mapping_max_skips_nan: "max(sqrt(-1), 5)" => 5.,
		mapping_min_all_nan: "min(sqrt(-1))" => f64::NAN,
		mapping_max_all_nan: "max(sqrt(-1))" => f64::NAN,

		// Typeset math symbol aliases
		alias_minus_sign: "5 − 3" => 2.,
		alias_unary_minus_sign: "−5 + 6" => 1.,
		alias_multiplication_sign: "3 × 4" => 12.,
		alias_dot_operator: "3 ⋅ 4" => 12.,
		alias_division_sign: "8 ÷ 2" => 4.,
		alias_logical_and: "if(1 ∧ 1, 2, 3)" => 2.,
		alias_logical_or: "if(0 ∨ 1, 2, 3)" => 2.,
		alias_logical_not: "¬0" => 1.,

		// Variadic generalizations and statistics
		geometry_hypot_variadic: "hypot(2, 3, 6)" => 7.,
		gcd_variadic: "gcd(24, 18, 60)" => 6.,
		lcm_variadic: "lcm(4, 6, 10)" => 60.,
		statistics_mean: "mean(1, 2, 3, 6)" => 3.,
		statistics_median_odd: "median(5, 1, 3)" => 3.,
		statistics_median_even: "median(4, 1, 3, 2)" => 2.5,
		statistics_variance: "variance(2, 4, 4, 4, 5, 5, 7, 9)" => 32. / 7.,
		statistics_variance_population: "variancepop(2, 4, 4, 4, 5, 5, 7, 9)" => 4.,
		statistics_stdev: "stdev(2, 4, 4, 4, 5, 5, 7, 9)" => (32_f64 / 7.).sqrt(),
		statistics_stdev_population: "stdevpop(2, 4, 4, 4, 5, 5, 7, 9)" => 2.,
		statistics_stdev_population_single: "stdevpop(5)" => 0.,
		statistics_geomean: "geomean(1, 4, 16)" => 4.,
		statistics_harmmean: "harmmean(1, 4, 4)" => 2.,
		statistics_rms: "rms(3, 4)" => 12.5_f64.sqrt(),
		statistics_mode: "mode(2, 3, 3, 1, 2)" => 2.,
		statistics_count: "count(1, 2, 3)" => 3.,
		logical_xor_odd_parity: "xor(1, 1, 1)" => 1.,
		logical_xor_even_parity: "xor(1, 0, 1)" => 0.,
		mapping_remap: "remap(5, 0, 10, 0, 100)" => 50.,

		// GCD / LCM
		gcd_simple: "gcd(24, 18)" => 6.,
		lcm_simple: "lcm(4, 6)" => 12.,
		gcd_negative_operand: "gcd(-24, 18)" => 6.,
		lcm_negative_operand: "lcm(-4, 6)" => 12.,

		// Combinatorics over whole numbers
		combinatorics_choose: "choose(5, 2)" => 10.,
		combinatorics_choose_symmetric: "choose(30, 28)" => 435.,
		combinatorics_choose_beyond_n: "choose(3, 5)" => 0.,
		combinatorics_pick: "pick(5, 2)" => 20.,
		combinatorics_pick_all: "pick(4, 4)" => 24.,
		// A result beyond f64 stops at infinity instead of stepping through quadrillions of terms
		combinatorics_choose_overflows: "choose(9007199254740992, 4503599627370496)" => f64::INFINITY,
		combinatorics_pick_overflows: "pick(9007199254740992, 9007199254740992)" => f64::INFINITY,

		// Truth values are the numbers 1 and 0
		constant_truth_values: "true + true - false" => 2.,

		// The `\` prefix names the language's own constants and functions, so LaTeX habits like `2\pi` evaluate
		builtin_prefix_constant: "\\tau / \\pi" => 2.,
		builtin_prefix_function: "\\sqrt(16)" => 4.,
		builtin_prefix_implicit_multiplication: "2\\pi" => 2. * std::f64::consts::PI,

		// atan2
		trig_atan2_axis: "atan2(1, 0)" => std::f64::consts::FRAC_PI_2,

		// Comparison operators combined with logical AND
		comparison_operators: "if(1 <= 2 && 1 ≤ 2 && 2 >= 1 && 2 ≥ 1, 1., 0.)" => 1.,

		// Logical AND / OR
		logical_and_true: "if(1 <= 2 && 2 < 3, 1., 0.)" => 1.,
		logical_and_false: "if(1 <= 2 && 3 < 2, 1., 0.)" => 0.,
		logical_or_true_left: "if(1 > 2 || 2 < 3, 1., 0.)" => 1.,
		logical_or_true_right: "if(2 < 1 || 2 < 3, 1., 0.)" => 1.,
		logical_or_false: "if(1 > 2 || 3 < 2, 1., 0.)" => 0.,
		logical_precedence_and_over_or: "if(0 == 1 || 1 == 1 && 0 == 0, 1., 0.)" => 1.,

		// Edge cases
		if_zero: "if(0.0, 1, 2)" => 2.,

		// Complex nested expressions
		if_nested_expr: "if((sqrt(16) + 2) * (sin(pi) + 1), 3 + 4 * 2, 5 - 2 / 1)" => 11.,

		// Overflow-safe evaluation
		factorial_overflows_to_infinity: "171!" => f64::INFINITY,
		factorial_huge_input: "10000000000000000000000!" => f64::INFINITY,
		lcm_huge_no_overflow: "lcm(1099511627776, 1099511627775)" => 1099511627776. * 1099511627775.,
		gcd_non_finite: "gcd(inf, 6)" => f64::NAN,
		long_literal: "10000000000000000000000" => 1e22,
		huge_exponent_saturates: "1e4294967296" => f64::INFINITY,

		// Odd integer roots of negative values are real
		root_negative_odd: "root(-8, 3)" => -2.,
		root_negative_odd_reciprocal: "root(-8, -3)" => -0.5,
		root_negative_even: "root(-4, 2)" => f64::NAN,

		// NaN poisons conditions and logic instead of acting as a boolean
		if_nan_condition: "if(sqrt(-1), 1, 2)" => f64::NAN,
		nan_and: "sqrt(-1) && 1" => f64::NAN,
		nan_or: "sqrt(-1) || 1" => f64::NAN,
		nan_not: "!sqrt(-1)" => f64::NAN,

		// Logic and equality span real and complex operands
		mixed_equality: "1 == i" => 0.,
		complex_equality: "i == i" => 1.,
		mixed_and: "1 && i" => 1.,
		mixed_nan_and: "sqrt(-1) && i" => f64::NAN,

		// Correctly rounded literals via std parsing
		seventeen_digit_literal: "999999999999999999" => 1e18,
		long_fraction_literal: "0.1111111111111111111111111111111111111111" => 1. / 9.,

		// Integer functions reject inputs beyond f64's exact integer range
		gcd_beyond_exact_integers: "gcd(10000000000000000000, 2)" => f64::NAN,

		// Implicit multiplication with parenthesized and negative-coefficient operands
		implicit_multiplication_parenthesized_negative: "2 (-3)" => -6.,
		implicit_multiplication_glued_parenthesized_negative: "2(-3)" => -6.,
		implicit_multiplication_negative_coefficient: "-3(2)" => -6.,

		// Unary plus
		unary_plus: "+5" => 5.,
		unary_plus_spaced_addition: "1 +2" => 3.,
		unary_plus_exponent: "2^+3" => 8.,

		// Base-suffixed logarithm and root function names
		log_suffixed: "log10(100)" => 2.,
		log_suffixed_underscore: "log_10(100)" => 2.,
		log_suffixed_fractional_base: "log3.25(5)" => 5f64.ln() / 3.25f64.ln(),
		root_suffixed: "root2(9)" => 3.,
		root_suffixed_underscore: "root_3(8)" => 2.,

		// Change of base widens into the complex plane, including through the base-suffixed spellings
		log_complex_change_of_base: "log(i, 2)" => Complex::new(0., std::f64::consts::FRAC_PI_2 / std::f64::consts::LN_2),
		log_complex_suffixed_base: "log3(i)" => Complex::new(0., std::f64::consts::FRAC_PI_2 / 3f64.ln()),
	}

	#[test]
	fn names_are_unicode_identifiers() {
		// Any script's letters begin a name, and combining marks extend one, so a decomposed `é` is a single name
		let decomposed_e_acute = format!("e{}", char::from_u32(0x301).unwrap());
		for input in ["λ + 1", "あ", "א", "x_2", decomposed_e_acute.as_str()] {
			assert!(ast::Node::try_parse_from_str(input).is_ok(), "expected `{input}` to parse");
		}

		// Symbols, emoji, digits of any script, lone marks, and invisible formatting characters cannot begin one,
		// and neither can an underscore, whose leading position Rust allows by a special case that we do not
		let lone_acute_mark = char::from_u32(0x301).unwrap().to_string();
		let right_to_left_override = format!("{}foo", char::from_u32(0x202E).unwrap());
		let flag = format!("{}{}", char::from_u32(0x1F1FA).unwrap(), char::from_u32(0x1F1F8).unwrap());
		for input in ["👍", "2👍", "٣", "²", "_foo", lone_acute_mark.as_str(), right_to_left_override.as_str(), flag.as_str()] {
			assert!(ast::Node::try_parse_from_str(input).is_err(), "expected `{input}` to be a parse error");
		}

		// Neither middle dot continues a name, so `a·b` is an error rather than one variable of that name
		for middle_dot in [0xB7, 0x387] {
			let input = format!("a{}b", char::from_u32(middle_dot).unwrap());
			assert!(ast::Node::try_parse_from_str(&input).is_err(), "expected `{input}` to be a parse error");
		}

		// A name ending in a combining mark is an operand like any other, so a spaced number after it multiplies
		for input in ["x 2".to_string(), format!("{decomposed_e_acute} 2")] {
			assert!(ast::Node::try_parse_from_str(&input).is_ok(), "expected `{input}` to parse");
		}
	}

	#[test]
	fn sigils_do_not_begin_names() {
		// Punctuation never begins a name, so these stay available as future namespace prefixes
		for input in ["# + 1", "#foo", "$", "$foo", "~foo * 2", "@foo", "2 ~ 3"] {
			assert!(ast::Node::try_parse_from_str(input).is_err(), "expected `{input}` to be a parse error");
		}
	}

	#[test]
	fn value_accessors_read_reals_only() {
		let value = evaluate("2.6").unwrap().unwrap();
		assert_eq!(value.as_f32(), Some(2.6_f32));
		assert_eq!(value.as_u8(), Some(3));
		assert_eq!(value.as_i32(), Some(3));

		let negative = evaluate("-2.6").unwrap().unwrap();
		assert_eq!(negative.as_i8(), Some(-3));
		assert_eq!(negative.as_u8(), None);

		assert_eq!(evaluate("300").unwrap().unwrap().as_u8(), None);
		assert_eq!(evaluate("i").unwrap().unwrap().as_i64(), None);
		assert_eq!(evaluate("inf").unwrap().unwrap().as_u64(), None);
	}
}
