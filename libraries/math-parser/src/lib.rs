pub mod ast;
pub mod constants;
pub mod context;
pub mod executer;
pub mod lexer;
pub mod matrix;
pub mod object;
pub mod parser;
pub mod quaternion;
pub mod reducer;
pub mod sort;
pub mod value;

use context::EvalContext;
use executer::EvalError;
use object::Object;
use parser::ParseError;

pub fn evaluate(expression: &str) -> Result<Result<Object, EvalError>, ParseError> {
	let expr = ast::Node::try_parse_from_str(expression);
	let context = EvalContext::default();
	expr.map(|node| node.eval(&context))
}

#[cfg(test)]
mod tests {
	use super::*;
	use matrix::{Affine2, Linear2, Matrix};
	use quaternion::Quaternion;
	use value::{Complex, Number, Rung, Value, Vector1, Vector2, Vector3, Weighted};

	const EPSILON: f64 = 1e-10_f64;

	#[test]
	fn malformed_juxtaposed_numbers_fail_to_parse() {
		// Two numbers cannot be glued together by a stray decimal point (they must not parse as implicit multiplication)
		for input in ["1.5.5", "1..", ".5.5", "1...5", "1.. .5"] {
			assert!(evaluate(input).is_err(), "expected `{input}` to be a parse error");
		}
	}

	#[test]
	fn a_number_before_euler_multiplies_unless_an_exponent_follows() {
		// The `e` of scientific notation needs a digit after it or after its sign, so `2e` reaches Euler's number like `2pi` reaches pi
		let real = |input: &str| evaluate(input).unwrap().unwrap().as_real().unwrap();
		let e = std::f64::consts::E;
		assert_eq!(real("2e"), 2. * e);
		assert_eq!(real("2e^2"), 2. * e * e);
		assert_eq!(real("2e - 1"), 2. * e - 1.);
		assert_eq!(real("2e-pi"), 2. * e - std::f64::consts::PI);
		assert_eq!(real("2exp(1)"), 2. * e);
		assert_eq!(real("2e-1"), 0.2);
		assert_eq!(real("2E+1"), 20.);
		assert_eq!(real("2.5e3"), 2500.);
	}

	#[test]
	fn unrecognized_characters_fail_to_parse() {
		// Unrecognized trailing input must be rejected rather than silently dropped after a valid prefix
		for input in ["2@", "5#", "2 $ 3", "sqrt(4)@", "5 & 3", "5 | 3", "2 = 3", "\\", "2 \\ 3", "\\2", "\\_foo"] {
			assert!(evaluate(input).is_err(), "expected `{input}` to be a parse error");
		}
	}

	#[test]
	fn percent_is_not_the_remainder() {
		// `%` is reserved for percentages, so a C-style remainder is a parse error naming the function that computes it
		for input in ["5 % 3", "x % 2", "%", "50%"] {
			let error = evaluate(input).unwrap_err().to_string();
			assert!(error.contains("`mod(a, b)`"), "`{input}` gave the error `{error}`");
		}
	}

	#[test]
	fn error_spans_begin_at_the_token() {
		for (input, expected) in [("2 %", "at 2..3"), ("x  if 1", "at 3..5")] {
			let error = evaluate(input).unwrap_err().to_string();
			assert!(error.ends_with(expected), "`{input}` gave the error `{error}`");
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
		for input in ["2 3", "10 000", "1 .5", "2 3 + 1", "1e5 3", "2. 3", "sqrt(4).5", "sqrt(4) .5", "pi.5", "5!.5", "|2| .5"] {
			assert!(evaluate(input).is_err(), "expected `{input}` to be a parse error");
		}
	}

	#[test]
	fn lone_double_bar_is_or() {
		// Two opening bars with nothing after them could never close, so a lone `||` is the Or operator itself
		for source in ["||", " || ", "\t||\n"] {
			assert_eq!(lexer::Lexer::new(source).collect::<Vec<_>>(), [lexer::Token::OrOr], "`{source}`");
		}
	}

	#[test]
	fn unbalanced_magnitude_bars_fail_to_parse() {
		// Each leaves a magnitude unclosed or empty, or an Or missing an operand, like `|1|2||`, which closes after the 1 and leaves `2 ||` dangling
		for input in [
			"|1|2||",
			"|2",
			"|1||2||",
			"2|",
			"|",
			"||",
			"|||",
			"||||",
			"| |",
			"|1|2|",
			"1 |",
			"|1| |",
			"||1|",
			"|1||",
			"|1 || |",
			"|| 1",
			"1 ||",
			"|1|||0|",
			"|0|||1|",
			"|2*||-3||",
			"(|1)|",
			"|(1|)",
			"| 1 | | 2",
			"|2|-3|",
			"0 || || 1",
			"|| || 1",
			"|1| ||",
			"||1||2||",
			"|1 ∨|",
			"∨ 1",
			"|∨|",
			"|1|||",
			"1 ||| 0",
			"||| |",
			"|0 || |1|",
		] {
			assert!(evaluate(input).is_err(), "expected `{input}` to be a parse error");
		}
	}

	#[test]
	fn piecewise_syntax_errors() {
		for input in ["{}", "{1 if 1,}", "{1, 2}", "{1 if 1 if 2}", "otherwise", "{1 if 1, 2 otherwise} if 1"] {
			assert!(evaluate(input).is_err(), "expected `{input}` to be a parse error");
		}

		// A misplaced keyword is told where it belongs
		for (input, expected) in [
			("x if 1", "`if` joins a case's value to its condition, like `{a if x > 0, b otherwise}`"),
			("{1 if 1 otherwise}", "`otherwise` ends the one case with no condition, like `{a if x > 0, b otherwise}`"),
			("{1 otherwise, 2 otherwise}", "A piecewise has at most one `otherwise` case"),
			("where", "`where` is a reserved word, so it can't be a name"),
		] {
			let error = evaluate(input).unwrap_err().to_string();
			assert!(error.starts_with(expected), "`{input}` gave the error `{error}`");
		}
	}

	#[test]
	fn piecewise_reads_like_cases_notation() {
		struct X(f64);
		impl context::ValueProvider for X {
			fn get_value(&self, name: &str) -> Option<Value> {
				(name == "x").then(|| Value::from_f64(self.0))
			}
		}
		let eval = |source: &str, x: f64| ast::Node::try_parse_from_str(source).unwrap().eval(&EvalContext::new(X(x), context::NothingMap));

		// A comparison chain is one condition
		for (x, expected) in [(-1., 0.), (0.5, 0.5), (2., 1.)] {
			assert_eq!(eval("{0 if x < 0, x if 0 <= x < 1, 1 otherwise}", x).unwrap().as_real(), Some(expected));
		}

		// Both cases hold at zero, so one inequality must be written strict
		assert_eq!(eval("{x if x >= 0, -x if x <= 0}", -3.).unwrap().as_real(), Some(3.));
		assert!(matches!(eval("{x if x >= 0, -x if x <= 0}", 0.), Err(EvalError::OverlappingCases)));
	}

	#[test]
	fn keywords_are_never_names() {
		struct Bindings;
		impl context::ValueProvider for Bindings {
			fn get_value(&self, name: &str) -> Option<Value> {
				matches!(name, "if" | "otherwise" | "where" | "iff").then(|| Value::from_f64(5.))
			}
		}
		let context = EvalContext::new(Bindings, context::NothingMap);

		// A host binding cannot claim a keyword, while a name that merely begins with one is ordinary
		for input in ["if + 1", "otherwise + 1", "where + 1"] {
			assert!(ast::Node::try_parse_from_str(input).is_err(), "expected `{input}` to be a parse error");
		}
		assert_eq!(ast::Node::try_parse_from_str("iff + 1").unwrap().eval(&context).unwrap().as_real(), Some(6.));

		// The `\` prefix asks for a builtin, so `\if` is a name, to the bar classifier as well
		assert!(matches!(evaluate("|\\if|").unwrap(), Err(EvalError::MissingValue(name)) if name == "\\if"));
	}

	#[test]
	fn piecewise_cases_are_disjoint() {
		// Two holding cases are an error even where their values agree, as is no holding case without `otherwise`
		assert!(matches!(evaluate("{0 if 0 >= 0, 0 if 0 <= 0}").unwrap(), Err(EvalError::OverlappingCases)));
		assert!(matches!(evaluate("{1 if 1 < 0, 2 if 1 > 5}").unwrap(), Err(EvalError::NoCaseHolds)));

		// Every condition is evaluated, so one that fails fails the whole expression even beside a holding case
		assert!(matches!(evaluate("{1 if 1, 2 if 2}").unwrap(), Err(EvalError::NotATruthValue)));
		assert!(matches!(evaluate("{1 if 1, 2 if 0/0 == 0}").unwrap(), Err(EvalError::Indeterminate)));
		assert!(matches!(evaluate("{1 if 1, 2 if 1, 3 if 0/0 == 0}").unwrap(), Err(EvalError::Indeterminate)));
	}

	#[test]
	fn comparison_chains_stay_in_one_direction() {
		for input in ["1 < 2 > 1", "1 < 2 != 3", "1 == 2 != 2"] {
			let error = evaluate(input).unwrap_err().to_string();
			let expected = "A comparison chain must read in one direction: all ascending (`<`, `<=`, `==`), all descending (`>`, `>=`, `==`), or all `!=`";
			assert_eq!(error, format!("{expected} at 0..{}", input.len()), "`{input}`");
		}
	}

	#[test]
	fn logic_requires_truth_values() {
		// A logical operand must be exactly 0 or 1, so a general number becomes a truth value only through a comparison
		for input in ["!5", "2 && 1", "0.5 || 0", "{1 if 3, 0 otherwise}", "1 && i", "{1 if sqrt(-1), 2 otherwise}"] {
			assert!(matches!(evaluate(input).unwrap(), Err(EvalError::NotATruthValue)), "expected `{input}` to need a truth value");
		}
	}

	#[test]
	fn indeterminate_forms_are_errors() {
		// No operation returns NaN: an indeterminate form is an evaluation error, as is a domain failure with no complex answer
		for input in [
			"0/0",
			"inf - inf",
			"0 * inf",
			"0 * inf i",
			"inf j * 0",
			"inf i - inf i",
			"inf / inf",
			"sin(inf)",
			"gcd(inf, 6)",
			"gcd(2.5, 5)",
			"lcm(4, 1.5)",
			"snap(5, 0)",
			"mod(5, 0)",
			"mod(5i, 0)",
			"choose(2.5, 1.5)",
			"pick(3, 0.5)",
			"choose(3, -1)",
			"(-1)!",
			"(-2)!",
			"(-inf)!",
		] {
			assert!(evaluate(input).unwrap().is_err(), "expected `{input}` to be an evaluation error");
		}
	}

	#[test]
	fn several_infinite_parts_give_no_direction() {
		// An infinity in two bases does not say which is larger, so the direction is an indeterminate form
		for input in [
			"normalize(inf i + inf j)",
			"angle(inf i - inf k, i)",
			"rotor(pi, inf i + inf j)",
			"axis(inf i + inf j)",
			"project(1, inf i + inf j)",
			"ln(inf i + inf j)",
		] {
			assert!(matches!(evaluate(input).unwrap(), Err(EvalError::Indeterminate)), "expected `{input}` to be indeterminate");
		}
	}

	#[test]
	fn display_writes_the_nonzero_parts_with_their_bases() {
		for (input, expected) in [
			("sqrt(-4)", "2i"),
			("1 - 2i", "1-2i"),
			("(1 + i) / 2", "0.5+0.5i"),
			("2i + 3j", "2i+3j"),
			("i * i", "-1"),
			("1/0", "∞"),
			("-inf i", "-∞i"),
			("1 + inf i", "1+∞i"),
		] {
			assert_eq!(evaluate(input).unwrap().unwrap().to_string(), expected, "`{input}`");
		}
	}

	#[test]
	fn factorial_extends_through_the_gamma_function() {
		// Compared relatively, since the reference values span hundreds of orders of magnitude
		for (input, expected) in [
			("10.5!", Complex::from(11_899_423.083962247)),
			("50.25!", Complex::from(8.112744267987253e64)),
			("170.5!", Complex::from(9.483367566824801e307)),
			("(-2.5)!", Complex::from(2.3632718012073544)),
			("(-10.25)!", Complex::from(6.950338494377042e-6)),
			("(-100.5)!", Complex::from(3.3704592739067173e-157)),
			("(-170.25)!", Complex::from(2.8837604712815413e-305)),
			("(-3.5+4i)!", Complex::new(-2.8327740563089983e-5, 5.018195008922803e-5)),
			("(-20.5+i)!", Complex::new(-5.085633633020186e-19, 7.443957006169107e-20)),
			("(300i)!", Complex::new(-2.1461927376275517e-204, -9.332698946010592e-204)),
			("(-1+227i)!", Complex::new(-1.4098280082608946e-157, -2.309687847859193e-156)),
			("(-1.5+300i)!", Complex::new(-9.760049091627542e-208, 1.5632983579858933e-207)),
		] {
			let Value::Number(actual) = evaluate(input).unwrap().unwrap().into_value().unwrap();
			let actual = actual.as_complex().unwrap();
			assert!((actual - expected).norm() / expected.norm() < 1e-12, "`{input}`: expected {expected}, got {actual}");
		}
	}

	#[test]
	fn combinatorics_extend_past_the_whole_numbers() {
		// Compared relatively like the factorial: a count up to a few thousand multiplies out directly, and a larger one goes
		// through the gamma function, whose logarithms in the tens of thousands cost a couple of digits
		for (input, expected, tolerance) in [
			("choose(0.5, 200)", Complex::from(-9.992306256589706e-5), 1e-12),
			("choose(10.5, 100)", Complex::from(-7.091868840771164e-17), 1e-12),
			("choose(-2.5, 7)", Complex::from(-17.80517578125), 1e-12),
			("choose(-0.5, 50)", Complex::from(0.07958923738717877), 1e-12),
			("pick(-2.5, 7)", Complex::from(-89738.0859375), 1e-12),
			("choose(1e22, 2)", Complex::from(5e43), 1e-12),
			("choose(1.5 + 2i, 3)", Complex::new(-1.0625, -1.4166666666666667), 1e-12),
			("pick(1.5 + 2i, 3)", Complex::new(-6.375, -8.5), 1e-12),
			("choose(0.5, 5000)", Complex::from(-7.979444083790532e-7), 1e-10),
			("choose(10.5, 8000)", Complex::from(-4.9672984268570704e-39), 1e-10),
			("choose(-0.5, 5000)", Complex::from(0.007978646139382154), 1e-10),
			("choose(1.5 + 2i, 5000)", Complex::new(-2.541871408941716e-8, -8.84744846043224e-9), 1e-10),
		] {
			let Value::Number(actual) = evaluate(input).unwrap().unwrap().into_value().unwrap();
			let actual = actual.as_complex().unwrap();
			assert!((actual - expected).norm() / expected.norm() < tolerance, "`{input}`: expected {expected}, got {actual}");
		}
	}

	#[test]
	fn simple_complex_quotients_are_exact() {
		// Smith's algorithm rounds nothing on these, where dividing by the norm twice loses an ulp
		for (input, expected) in [
			("(1 + i) / (1 - i)", Complex::new(0., 1.)),
			("2 / (1 - i)", Complex::new(1., 1.)),
			("(3 + 4i) / (1 + 2i)", Complex::new(2.2, -0.4)),
			("harmmean(i, 1)", Complex::new(1., 1.)),
		] {
			assert_eq!(evaluate(input).unwrap().unwrap(), Object::from(expected), "`{input}`");
		}
	}

	#[test]
	fn small_combinatorics_are_exact() {
		// The direct product leaves no rounding behind on these, where the gamma function would
		for (input, expected) in [("choose(2.5, 2)", 1.875), ("pick(2.5, 2)", 3.75), ("choose(-0.5, 2)", 0.375)] {
			assert_eq!(evaluate(input).unwrap().unwrap().as_real(), Some(expected), "`{input}`");
		}
		assert_eq!(evaluate("choose(i, 2)").unwrap().unwrap(), Object::from(Complex::new(-0.5, -0.5)));
	}

	#[test]
	fn host_values_are_admitted_where_read() {
		struct Host;
		impl context::ValueProvider for Host {
			fn get_value(&self, name: &str) -> Option<Value> {
				match name {
					"x" => Some(Value::from_f64(f64::NAN)),
					"z" => Some(Value::from_f64(-0.)),
					_ => None,
				}
			}
		}
		impl context::FunctionProvider for Host {
			fn run_function(&self, name: &str, _: &[Value]) -> Option<Value> {
				(name == "f").then(|| Value::from_f64(f64::NAN))
			}
		}
		let eval = |source: &str| ast::Node::try_parse_from_str(source).unwrap().eval(&EvalContext::new(Host, Host));

		// A NaN from a host binding or function is an error where it is read, and nowhere else
		assert!(matches!(eval("x + 1"), Err(EvalError::NotANumber(name)) if name == "x"));
		assert!(matches!(eval("x(2)"), Err(EvalError::NotANumber(name)) if name == "x"));
		assert!(matches!(eval("f(1)"), Err(EvalError::NotANumber(name)) if name == "f"));
		assert_eq!(eval("{2 if 1, x otherwise}").unwrap().as_real(), Some(2.));

		// A host's signed zero is plain zero, like every other value
		assert_eq!(eval("1/z").unwrap().as_real(), Some(f64::INFINITY));
	}

	#[test]
	fn statistics_without_an_answer_are_errors() {
		// No value repeats, so no mode exists
		assert!(evaluate("mode(1, 2, 3)").unwrap().is_err());

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
			("rms(1e-320)", 1e-320),
			("lerp(-1e308, 1e308, 0.5)", 0.),
			("lerp(-1e308, 1e308, 1)", 1e308),
		] {
			assert_eq!(evaluate(input).unwrap().unwrap().as_real(), Some(expected), "`{input}`");
		}

		// Three large operands whose least common multiple exceeds integer storage must not wrap around
		let input = "lcm(9007199254740992, 9007199254740991, 9007199254740990)";
		assert!(!matches!(evaluate(input), Ok(Ok(value)) if value.as_real().is_some_and(f64::is_finite)), "`{input}`");

		// Each part averages over its own scale, so a small part beside huge ones keeps its precision
		let Value::Number(mean) = evaluate("mean(1e308, 1e308, j)").unwrap().unwrap().into_value().unwrap();
		assert_eq!(mean.to_quaternion().y, 1. / 3.);
	}

	#[test]
	fn magnitudes_avoid_intermediate_overflow_and_underflow() {
		// Squaring these parts would overflow or underflow, but their magnitudes fit
		for (input, expected) in [("|3e200 + 4e200j|", 5e200), ("|3e-200 + 4e-200j|", 5e-200), ("|1e-320 j|", 1e-320)] {
			let magnitude = evaluate(input).unwrap().unwrap().as_real().unwrap();
			assert!((magnitude / expected - 1.).abs() < 1e-15, "`{input}` gave {magnitude}");
		}
	}

	#[test]
	fn real_operands_act_on_each_part_as_on_a_real() {
		// A real divisor, modulus, or step treats each part exactly as it would a lone real
		for (input, expected) in [
			("10k / 3", "(10 / 3) k"),
			("mod(7.3 + j, 0.1)", "mod(7.3, 0.1) + mod(1, 0.1) j"),
			("snap(2.6 - 1.4k, 0.3)", "snap(2.6, 0.3) + snap(-1.4, 0.3) k"),
		] {
			assert_eq!(evaluate(input).unwrap().unwrap(), evaluate(expected).unwrap().unwrap(), "`{input}`");
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
	fn rename_identifiers_is_token_exact() {
		let a_to_x = |name: &str| name.eq_ignore_ascii_case("a").then(|| "x".to_string());

		// Only whole identifiers rename, so function names and other tokens that merely contain the letter stay untouched
		assert_eq!(crate::lexer::rename_identifiers("2 - 0.2A", a_to_x).as_deref(), Some("2 - 0.2x"));
		assert_eq!(crate::lexer::rename_identifiers("atan(a) + tau", a_to_x).as_deref(), Some("atan(x) + tau"));
		assert_eq!(
			crate::lexer::rename_identifiers("sqrt(A + B) - B^2", |name| name.eq_ignore_ascii_case("b").then(|| "(3)".to_string())).as_deref(),
			Some("sqrt(A + (3)) - (3)^2")
		);
		assert_eq!(crate::lexer::rename_identifiers("logb + b", |name| (name == "b").then(|| "c".to_string())).as_deref(), Some("logb + c"));

		// A string that fails to lex reports `None` rather than renaming unreliably
		assert_eq!(crate::lexer::rename_identifiers("a + \u{200b}b", a_to_x), None);
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

	fn run_end_to_end_test(input: &str, expected: Object) {
		let expr = match ast::Node::try_parse_from_str(input) {
			Ok(expr) => expr,
			Err(err) => panic!("failed to parse `{input}`: {err}"),
		};
		let context = EvalContext::default();

		let actual = match expr.eval(&context) {
			Ok(v) => v,
			Err(err) => panic!("failed to evaluate `{input}` because of error {err}"),
		};

		// Storage is never observable, so a value compares by its four parts and a matrix by its twenty entries, with infinities matched exactly
		let entries = |object: &Object| match object {
			Object::Value(Value::Number(number)) => number.to_quaternion().parts().to_vec(),
			Object::Matrix(matrix) => matrix.rows.iter().chain([&matrix.translation]).flat_map(|row| row.parts()).collect::<Vec<f64>>(),
		};
		assert!(actual.as_matrix().is_some() == expected.as_matrix().is_some(), "`{input}`: expected {expected}, got {actual}");
		if let (Some(actual), Some(expected)) = (actual.as_matrix(), expected.as_matrix()) {
			assert_eq!(actual.axes, expected.axes, "`{input}`: the axes the matrix acts on");
		}
		for (index, (actual, expected)) in entries(&actual).into_iter().zip(entries(&expected)).enumerate() {
			if actual.is_infinite() || expected.is_infinite() {
				assert!(actual == expected, "`{input}` → part {index}: expected {expected:?}, got {actual:?}");
			} else {
				let difference = (actual - expected).abs();
				assert!(difference < EPSILON, "`{input}` → part {index}: expected {expected}, got {actual}, Δ={difference}");
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
		modulo_pos_pos: "mod(3.2, 2)" => 1.2,
		modulo_pos_neg: "mod(3.2, -2)" => -0.8,
		modulo_neg_neg: "mod(-3.2, -2)" => -1.2,
		modulo_neg_pos: "mod(-3.2, 2)" => 0.8,
		modulo_neg_multiple: "mod(-4, 2)" => 0.,
		modulo_integer_wrap: "mod(-7, 3)" => 2.,
		modulo_angle_wrap: "mod(-pi/2, tau)" => 1.5 * std::f64::consts::PI,
		modulo_complex: "mod(5.5i, 2)" => Complex::new(0., 1.5),
		modulo_complex_both_parts: "mod(3 + 5.5i, 2)" => Complex::new(1., 1.5),
		modulo_complex_modulus: "mod(7, 2i)" => -1.,
		modulo_quaternion: "mod(j + 3k, 2)" => Quaternion::new(0., 0., 1., 1.),
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
		implicit_multiplication_trailing_number_after_magnitude: "|2| 3" => 6.,

		// Factorial (postfix !)
		factorial_simple: "5!" => 120.,
		factorial_nested: "(3 + 2)!" => 120.,
		factorial_zero: "0!" => 1.,
		factorial_chain: "3!!" => 720., // (3!)! = 6! = 720
		factorial_half: "0.5!" => std::f64::consts::PI.sqrt() / 2.,
		factorial_negative_half: "(-0.5)!" => std::f64::consts::PI.sqrt(),
		factorial_fractional: "2.5!" => 3.323350970447842,
		factorial_negative_fractional: "(-1.5)!" => -2. * std::f64::consts::PI.sqrt(),
		factorial_imaginary: "i!" => Complex::new(0.498015668118356, -0.1549498283018107),
		factorial_complex_left_half_plane: "(-1.5 + 2i)!" => Complex::new(-0.03903884916211552, -0.03516787606268694),
		factorial_infinity: "inf!" => f64::INFINITY,

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
		constant_infinity: "{inf if inf == ∞, 0 otherwise}" => f64::INFINITY,
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

		// Basic piecewise cases
		piecewise_true_condition: "{5 if 1, 3 otherwise}" => 5.,
		piecewise_false_condition: "{5 if 0, 3 otherwise}" => 3.,

		// Arithmetic conditions
		piecewise_arithmetic_true: "{1 if 2+2-4, 0 otherwise}" => 0.,
		piecewise_arithmetic_false: "{1 if 3*2-5, 0 otherwise}" => 1.,

		// Nested arithmetic
		piecewise_complex_arithmetic: "{10 if (5+3)*(2-1) > 0, 20 otherwise}" => 10.,
		piecewise_with_division: "{15 if 8/4-2 == 0, 25 otherwise}" => 15.,
		piecewise_with_division_ne: "{15 if 8/4-2 ≠ 0, 25 otherwise}" => 25.,

		// Constants in conditions
		piecewise_with_pi: "{1 if pi > 3, 0 otherwise}" => 1.,
		piecewise_with_e: "{1 if e < 3, 0 otherwise}" => 1.,

		// Functions in conditions
		piecewise_with_sqrt: "{1 if sqrt(16) == 4, 0 otherwise}" => 1.,
		piecewise_with_sin: "{1 if sin(pi) == 0.0, 0 otherwise}" => 0.,

		// Logical NOT (prefix !)
		logical_not_zero: "!0" => 1.,
		logical_not_one: "!1" => 0.,
		logical_not_expression: "!(2 - 2)" => 1.,

		// Log / exp / pow / root
		log_ln: "ln(e)" => 1.,
		log_log10: "log(100)" => 2.,
		log_log2: "log2(8)" => 3.,
		log_change_of_base: "log(8, 2)" => 3.,
		exp_function: "exp(1)" => std::f64::consts::E,
		root_square: "root(9, 2)" => 3.,
		root_cube: "root(8, 3)" => 2.,

		// Nested piecewise cases
		nested_piecewise: "{{1 if 0, 2 otherwise} if 1, 3 otherwise}" => 2.,
		nested_piecewise_complex: "{{5 if 1, 6 otherwise} if 2-2 == 0, {7 if 1, 8 otherwise} otherwise}" => 5.,

		// Mixed operations in conditions and blocks
		piecewise_complex_condition: "{2*pi if sqrt(16) + sin(pi) < 5, 3*e otherwise}" => 2. * std::f64::consts::PI,
		piecewise_complex_blocks: "{2*sqrt(16) + sin(pi/2) if 1, 3*cos(0) + 4 otherwise}" => 9.,

		// Cases are unordered, so `otherwise` may stand anywhere, and only the holding case's value is evaluated
		piecewise_sign: "{-1 if -3 < 0, 0 if -3 == 0, 1 if -3 > 0}" => -1.,
		piecewise_otherwise_first: "{1 otherwise, 2 if 1 > 2}" => 1.,
		piecewise_only_otherwise: "{4 otherwise}" => 4.,
		piecewise_skips_other_values: "{1 if 1, 0/0 otherwise}" => 1.,
		piecewise_vector_value: "{3i + 4j if 1, 0 otherwise}" => Quaternion::new(0., 3., 4., 0.),

		// A piecewise is an operand like a parenthesized expression
		piecewise_implicit_multiplication: "2{3 if 1, 4 otherwise}" => 6.,
		piecewise_power: "{3 if 1, 4 otherwise}^2" => 9.,
		piecewise_bars_after_if: "|{-1 if |-2| > 1, 2 otherwise}|" => 1.,
		piecewise_negated: "-{1 if 1, 2 otherwise}" => -1.,
		piecewise_as_argument: "max({1 if 1, 2 otherwise}, 7)" => 7.,
		piecewise_before_magnitude: "{2 if 1, 3 otherwise}|-3|" => 6.,
		piecewise_multiline: "{\n\t1 if 0,\n\t2 otherwise\n}" => 2.,

		// Mapping helpers
		mapping_trunc: "trunc(3.7)" => 3.,
		mapping_fract: "fract(3.25)" => 0.25,
		mapping_sign_pos: "sign(5)" => 1.,
		mapping_sign_neg: "sign(-5)" => -1.,
		mapping_snap: "snap(7, 5)" => 5.,
		mapping_snap_half_away_from_zero: "snap(7.5, 5)" => 10.,
		mapping_snap_negative: "snap(-7.5, 5)" => -10.,
		mapping_snap_negative_step: "snap(7, -5)" => 5.,
		mapping_snap_integer_half_away_from_zero: "snap(-7, 2)" => -8.,
		mapping_snap_fractional_step: "snap(0.37, 0.1)" => 0.4,
		mapping_snap_complex: "snap(1.4 + 2.6i, 1)" => Complex::new(1., 3.),
		mapping_snap_complex_step: "snap(2.2 + 0.1i, 1 + i)" => 2.,

		// Geometry / mapping extras
		geometry_hypot: "hypot(3, 4)" => 5.,

		// Minimum and maximum accept two or more arguments
		mapping_min: "min(3, 7)" => 3.,
		mapping_max: "max(3, 7)" => 7.,
		mapping_min_variadic: "min(5, 2, 8, 4)" => 2.,
		mapping_max_variadic: "max(5, 2, 8, 4)" => 8.,

		// Typeset math symbol aliases
		alias_minus_sign: "5 − 3" => 2.,
		alias_unary_minus_sign: "−5 + 6" => 1.,
		alias_multiplication_sign: "3 × 4" => 12.,
		alias_dot_operator: "3 ⋅ 4" => 12.,
		alias_division_sign: "8 ÷ 2" => 4.,
		alias_logical_and: "{2 if 1 ∧ 1, 3 otherwise}" => 2.,
		alias_logical_or: "{2 if 0 ∨ 1, 3 otherwise}" => 2.,
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
		statistics_mean_complex: "mean(i, 3i)" => Complex::new(0., 2.),
		statistics_mean_lands_real: "mean(1 + i, 3 - i)" => 2.,
		statistics_variance_complex: "variance(i, -i)" => 2.,
		statistics_stdev_population_complex: "stdevpop(1 + i, 1 - i)" => 1.,
		statistics_rms_complex: "rms(3i, 4)" => 12.5_f64.sqrt(),
		statistics_geomean_single: "geomean(-3)" => -3.,
		statistics_geomean_negative_pair: "geomean(-1, -4)" => 2.,
		statistics_geomean_climbs: "geomean(-1, 2)" => Complex::new(0., 2_f64.sqrt()),
		statistics_geomean_complex: "geomean(i, 1)" => Complex::new(std::f64::consts::FRAC_1_SQRT_2, std::f64::consts::FRAC_1_SQRT_2),
		statistics_harmmean_negative: "harmmean(-1, -3)" => -1.5,
		statistics_harmmean_cancelling: "harmmean(-1, 1)" => f64::INFINITY,
		statistics_harmmean_complex: "harmmean(i, 1)" => Complex::new(1., 1.),
		statistics_geomean_infinite: "geomean(inf, 1)" => f64::INFINITY,
		statistics_harmmean_infinite_operand: "harmmean(inf, 1)" => 2.,
		statistics_harmmean_all_infinite: "harmmean(-inf, -inf)" => f64::NEG_INFINITY,
		statistics_harmmean_huge: "harmmean(1e308, 1e308)" => 1e308,
		statistics_variance_quaternion: "variance(j, -j)" => 2.,
		statistics_rms_quaternion: "rms(3j, 4k)" => 12.5_f64.sqrt(),
		logical_xor_odd_parity: "xor(1, 1, 1)" => 1.,
		logical_xor_even_parity: "xor(1, 0, 1)" => 0.,
		mapping_remap: "remap(5, 0..10, 0..100)" => 50.,

		// GCD / LCM
		gcd_simple: "gcd(24, 18)" => 6.,
		lcm_simple: "lcm(4, 6)" => 12.,
		gcd_negative_operand: "gcd(-24, 18)" => 6.,
		lcm_negative_operand: "lcm(-4, 6)" => 12.,
		gcd_beyond_the_reals_limit: "gcd(10000000000000000000, 2)" => 2.,

		// Combinatorics over any top and a whole count
		combinatorics_choose: "choose(5, 2)" => 10.,
		combinatorics_choose_symmetric: "choose(30, 28)" => 435.,
		combinatorics_choose_beyond_n: "choose(3, 5)" => 0.,
		combinatorics_choose_fractional_top: "choose(2.4, 1)" => 2.4,
		combinatorics_choose_half: "choose(0.5, 2)" => -0.125,
		combinatorics_choose_negative_top: "choose(-1, 3)" => -1.,
		combinatorics_choose_negative_top_larger: "choose(-5, 3)" => -35.,
		combinatorics_choose_zero_count: "choose(2.5, 0)" => 1.,
		combinatorics_choose_complex_top: "choose(i, 2)" => Complex::new(-0.5, -0.5),
		combinatorics_pick: "pick(5, 2)" => 20.,
		combinatorics_pick_all: "pick(4, 4)" => 24.,
		combinatorics_pick_fractional_top: "pick(2.5, 2)" => 3.75,
		combinatorics_pick_negative_top: "pick(-5, 3)" => -210.,
		combinatorics_pick_complex_top: "pick(i, 2)" => Complex::new(-1., -1.),
		combinatorics_choose_quaternion_top: "choose(j, 2)" => Quaternion::new(-0.5, 0., -0.5, 0.),
		// A result beyond f64 stops at infinity instead of stepping through quadrillions of terms
		combinatorics_choose_overflows: "choose(9007199254740992, 4503599627370496)" => f64::INFINITY,
		combinatorics_pick_overflows: "pick(9007199254740992, 9007199254740992)" => f64::INFINITY,
		combinatorics_pick_fractional_overflows: "pick(0.5, 200)" => f64::NEG_INFINITY,
		combinatorics_pick_complex_overflows: "|pick(1.5 + 2i, 300)|" => f64::INFINITY,
		combinatorics_choose_infinite_top: "choose(inf, 2)" => f64::INFINITY,
		combinatorics_pick_negative_infinite_top: "pick(-inf, 3)" => f64::NEG_INFINITY,

		// Truth values are the numbers 1 and 0
		constant_truth_values: "true + true - false" => 2.,

		// The `\` prefix names the language's own constants and functions, so LaTeX habits like `2\pi` evaluate
		builtin_prefix_constant: "\\tau / \\pi" => 2.,
		builtin_prefix_function: "\\sqrt(16)" => 4.,
		builtin_prefix_implicit_multiplication: "2\\pi" => 2. * std::f64::consts::PI,

		// atan2
		trig_atan2_axis: "atan2(1, 0)" => std::f64::consts::FRAC_PI_2,

		// Comparison operators combined with logical AND
		comparison_operators: "{1. if 1 <= 2 && 1 ≤ 2 && 2 >= 1 && 2 ≥ 1, 0. otherwise}" => 1.,

		// Logical AND / OR
		logical_and_true: "{1. if 1 <= 2 && 2 < 3, 0. otherwise}" => 1.,
		logical_and_false: "{1. if 1 <= 2 && 3 < 2, 0. otherwise}" => 0.,
		logical_or_true_left: "{1. if 1 > 2 || 2 < 3, 0. otherwise}" => 1.,
		logical_or_true_right: "{1. if 2 < 1 || 2 < 3, 0. otherwise}" => 1.,
		logical_or_false: "{1. if 1 > 2 || 3 < 2, 0. otherwise}" => 0.,
		logical_precedence_and_over_or: "{1. if 0 == 1 || 1 == 1 && 0 == 0, 0. otherwise}" => 1.,

		// Edge cases
		piecewise_zero: "{1 if 0.0, 2 otherwise}" => 2.,

		// Complex nested expressions
		piecewise_nested_expr: "{3 + 4 * 2 if (sqrt(16) + 2) * (sin(pi) + 1) > 5, 5 - 2 / 1 otherwise}" => 11.,

		// Overflow-safe evaluation
		factorial_overflows_to_infinity: "171!" => f64::INFINITY,
		factorial_huge_input: "10000000000000000000000!" => f64::INFINITY,
		factorial_fractional_overflows_to_infinity: "171.5!" => f64::INFINITY,
		factorial_huge_fractional_input: "4503599627370495.5!" => f64::INFINITY,
		factorial_complex_overflows_to_infinity: "|(171 + 0.5i)!|" => f64::INFINITY,
		factorial_complex_underflows_to_zero: "(-190.5 + 0.5i)!" => 0.,
		factorial_huge_negative_underflows_to_zero: "(-740.5)!" => 0.,
		lcm_huge_no_overflow: "lcm(1099511627776, 1099511627775)" => 1099511627776. * 1099511627775.,
		long_literal: "10000000000000000000000" => 1e22,
		huge_exponent_saturates: "1e4294967296" => f64::INFINITY,
		power_beyond_integer_storage: "2^127" => 2f64.powi(127),

		// Odd integer roots of negative values are real, while even ones climb into the complex plane
		root_negative_odd: "root(-8, 3)" => -2.,
		root_negative_odd_reciprocal: "root(-8, -3)" => -0.5,
		root_negative_even: "root(-4, 2)" => Complex::new(0., 2.),
		root_complex_degree: "root(8, 3i)" => Complex::new(0.7692389013639721, -0.6389612763136348),
		root_complex_degree_lands_real: "root(i, i)" => std::f64::consts::FRAC_PI_2.exp(),

		// Equality spans real and complex operands
		mixed_equality: "1 == i" => 0.,
		complex_equality: "i == i" => 1.,

		// Value identity: a zero imaginary part or a signed zero never changes a result, so a result landing on the real line is real
		value_identity_product: "sqrt(-4) * sqrt(-4)" => -4.,
		value_identity_conjugate_product: "(1 + i)(1 - i)" => 2.,
		value_identity_ordering: "(2 + 0i) < 3" => 1.,
		value_identity_branch_selection: "root(-8 + 0i, 3)" => -2.,
		value_identity_signed_zero: "1/(-0)" => f64::INFINITY,
		value_identity_ceiling: "1/ceil(-0.5)" => f64::INFINITY,

		// Division by zero heads to infinity part by part with each part's own sign, like an overflow, rather than turning indeterminate
		division_by_zero_imaginary: "i / 0" => Complex::new(0., f64::INFINITY),
		division_by_zero_complex: "(1 + i) / 0" => Complex::new(f64::INFINITY, f64::INFINITY),
		division_by_zero_complex_signs: "(1 - i) / 0" => Complex::new(f64::INFINITY, f64::NEG_INFINITY),
		division_scaled_tiny: "(1e-200i) / (1e-200i)" => 1.,
		division_scaled_huge: "(1e200 + 1e200i) / (1e200 + 1e200i)" => 1.,
		division_scaled_max: "(1e308 + 1e308i) / (1e308 + 1e308i)" => 1.,
		division_scaled_max_dividend: "(1e308 + 1e308i) / (1 + i)" => 1e308,
		division_scaled_max_dividend_signs: "(1e308 - 1e308i) / (1 + i)" => Complex::new(0., -1e308),
		division_by_zero_quaternion: "|j / 0|" => f64::INFINITY,
		division_scaled_quaternion: "(1e-200 j) / (1e-200 j)" => 1.,
		division_by_subnormal_quaternion: "1 / (1e-320 j)" => Quaternion::new(0., 0., f64::NEG_INFINITY, 0.),
		division_by_infinite_quaternion: "1 / (j / 0)" => 0.,

		// A zero part is an absent axis, so an infinite vector scales, turns, and divides without the NaN of `∞ · 0`
		infinite_imaginary: "inf i" => Complex::new(0., f64::INFINITY),
		infinite_imaginary_typeset: "-∞j" => Quaternion::new(0., 0., f64::NEG_INFINITY, 0.),
		infinite_imaginary_turned: "inf i * i" => f64::NEG_INFINITY,
		infinite_quaternion_turned: "inf j * k" => Complex::new(0., f64::INFINITY),
		infinite_imaginary_squared: "(inf i)^2" => f64::NEG_INFINITY,
		infinite_imaginary_quotient: "inf i / i" => f64::INFINITY,
		infinite_quaternion_quotient: "inf j / j" => f64::INFINITY,
		infinite_power_direction: "(1e200 i)^3" => Complex::new(0., f64::NEG_INFINITY),

		// Functions of one variable find the direction of a tiny or infinite vector part without overflow or NaN
		subnormal_quaternion_logarithm: "ln(1e-320 j)" => Quaternion::new(1e-320_f64.ln(), 0., std::f64::consts::FRAC_PI_2, 0.),
		infinite_quaternion_logarithm: "ln(inf j)" => Quaternion::new(f64::INFINITY, 0., std::f64::consts::FRAC_PI_2, 0.),
		infinite_quaternion_root: "sqrt(inf j)" => Quaternion::new(f64::INFINITY, 0., f64::INFINITY, 0.),

		// Overflowing terms that cancel rerun at a smaller scale, keeping only the product's true infinities
		overflowing_complex_product: "(1e200 + 1e200i) * (1e200 + 1e200i)" => Complex::new(0., f64::INFINITY),
		overflowing_complex_square: "(1e200 + 1e200i)^2" => Complex::new(0., f64::INFINITY),
		overflowing_quaternion_square: "(1e200 + 1e200j)^2" => Quaternion::new(0., 0., f64::INFINITY, 0.),
		overflowing_product_keeps_moderate_parts: "(1e200 + 1e200 i + 1e-150 k) * (1e200 - 1e200 i + 1e-150 k)" => Quaternion::new(f64::INFINITY, 0., -2. * (1e200 * 1e-150), 2. * (1e200 * 1e-150)),

		// A whole exponent on a complex base multiplies out exactly
		power_imaginary_square: "i^2" => -1.,
		power_imaginary_cube: "i^3" => Complex::new(0., -1.),
		power_imaginary_zero: "i^0" => 1.,
		power_imaginary_huge: "i^(2^70)" => 1.,
		power_complex_square: "(3 + 4i)^2" => Complex::new(-7., 24.),
		power_complex_negative: "(1 + i)^-2" => Complex::new(0., -0.5),
		power_complex_negative_tiny: "(1e-200i)^-1" => Complex::new(0., -1e200),
		power_complex_negative_underflow: "|(1e-200i)^-2|" => f64::INFINITY,
		power_complex_overflow: "|(1.5 + 2i)^1000|" => f64::INFINITY,
		power_quaternion_square: "j^2" => -1.,
		power_quaternion_mixed_square: "(i + j)^2" => -2.,
		power_quaternion_fourth: "k^4" => 1.,
		power_quaternion_negative: "j^-1" => Quaternion::new(0., 0., -1., 0.),

		// Domain climbs: a real input whose answer is complex resolves into the `1, i` plane
		climb_sqrt: "sqrt(-4)" => Complex::new(0., 2.),
		climb_ln: "ln(-1)" => Complex::new(0., std::f64::consts::PI),
		climb_log_base: "log(-1, 10)" => Complex::new(0., std::f64::consts::PI / std::f64::consts::LN_10),
		climb_asin: "|sin(asin(2))|" => 2.,
		climb_acosh: "|cosh(acosh(0.5))|" => 0.5,
		climb_power: "|(-8)^(1/3)|" => 2.,

		// Magnitude bars: absolute value on the reals and the Euclidean magnitude beyond, with `||` reading as Or unless two magnitudes are open
		magnitude_real: "|-3|" => 3.,
		mapping_abs: "abs(-3)" => 3.,
		mapping_abs_per_part: "abs(-3 - 4i)" => Complex::new(3., 4.),
		magnitude_complex: "|3 + 4i|" => 5.,
		magnitude_implicit_multiplication: "2|-3|" => 6.,
		magnitude_juxtaposed: "|-2| |-3|" => 6.,
		magnitude_then_subtraction: "|-2|-3" => -1.,
		magnitude_nested: "|2*|-3||" => 6.,
		magnitude_nested_open: "||-3| - 5|" => 2.,
		magnitude_nested_both_ends: "|1 + ||-2| - 3||" => 2.,
		magnitude_double_bars: "||-3||" => 3.,
		magnitude_or_inside: "|1||0|" => 1.,
		magnitude_factorial: "|-3|!" => 6.,

		// Magnitude bars around single values
		magnitude_zero: "|0|" => 0.,
		magnitude_negative_zero: "|-0|" => 0.,
		magnitude_positive: "|5|" => 5.,
		magnitude_inner_spaces: "| - 2 |" => 2.,
		magnitude_inner_tab_and_newline: "|\t-2\n|" => 2.,
		magnitude_fraction: "|-2.5|" => 2.5,
		magnitude_leading_dot_fraction: "|-.5|" => 0.5,
		magnitude_scientific: "|-1e-3|" => 0.001,
		magnitude_infinity_word: "|-inf|" => f64::INFINITY,
		magnitude_infinity_symbol: "|-∞|" => f64::INFINITY,
		magnitude_imaginary_unit: "|-i|" => 1.,
		magnitude_complex_negative_parts: "|-3 - 4i|" => 5.,
		magnitude_of_parenthesized: "|(-3)|" => 3.,
		magnitude_in_parentheses: "(|-3|)" => 3.,
		magnitude_of_difference: "|2 - 5|" => 3.,

		// Magnitude bars among operators, where each bar after an operand closes and each bar after an operator opens
		magnitude_negated: "-|-3|" => -3.,
		magnitude_squared: "|-3|^2" => 9.,
		magnitude_as_exponent: "2^|-3|" => 8.,
		magnitude_power_of_magnitudes: "|-2|^|-3|" => 8.,
		magnitude_product_spaced: "|-2| * |-3|" => 6.,
		magnitude_product_unspaced: "|-2|*|-3|" => 6.,
		magnitude_sum_spaced: "|-2| + |-3|" => 5.,
		magnitude_sum_unspaced: "|-2|+|-3|" => 5.,
		magnitude_difference_spaced: "|-2| - |-3|" => -1.,
		magnitude_difference_unspaced: "|-2|-|-3|" => -1.,
		magnitude_quotient: "|-2| / |-4|" => 0.5,
		magnitude_modulo: "mod(|-7|, |-4|)" => 3.,
		magnitude_factorial_inside: "|3!|" => 6.,
		magnitude_factorial_then_sum: "|-3|! + 1" => 7.,
		magnitude_less_than_spaced: "|-3| < |-5|" => 1.,
		magnitude_less_than_unspaced: "|-3|<|-5|" => 1.,
		magnitude_equality: "|-3| == 3" => 1.,

		// Magnitude bars juxtaposed with other operands multiply
		magnitude_then_number: "|-2|3" => 6.,
		magnitude_after_number: "3|-2|" => 6.,
		magnitude_then_parenthesized: "|-2|(3)" => 6.,
		magnitude_after_parenthesized: "(3)|-2|" => 6.,
		magnitude_then_constant: "|-1|pi" => std::f64::consts::PI,
		magnitude_after_constant: "pi|-1|" => std::f64::consts::PI,
		magnitude_three_juxtaposed: "|2| |3| |4|" => 24.,
		magnitude_juxtaposed_then_sum: "|-2| |-3| + 1" => 7.,
		magnitude_number_then_two_magnitudes: "2 |-3| |-4|" => 24.,
		magnitude_difference_then_implicit_product: "|-2|-3|-4|" => -10.,

		// Magnitude bars around function calls and function arguments
		magnitude_as_argument: "sqrt(|-16|)" => 4.,
		magnitude_of_complex_result: "|sqrt(-16)|" => 4.,
		magnitude_as_both_arguments: "max(|-3|, |2|)" => 3.,
		magnitude_as_two_argument_log: "log(|-100|, |-10|)" => 2.,
		magnitude_of_call: "|max(-3, -5)|" => 3.,
		magnitude_of_call_with_magnitude_argument: "|log(|-100|)|" => 2.,
		magnitude_of_piecewise: "|{-2 if 1, 3 otherwise}|" => 2.,
		magnitude_in_piecewise: "{|-7| if |-1| == 1, 0 otherwise}" => 7.,

		// Nested magnitude bars, spaced and unspaced, including runs of three bars
		magnitude_nested_closing_spaced: "|1 - |2 - 5| |" => 2.,
		magnitude_nested_closing_unspaced: "|1 - |2 - 5||" => 2.,
		magnitude_nested_both_spaced: "| |-3| |" => 3.,
		magnitude_nested_negated: "|-|-3||" => 3.,
		magnitude_nested_negated_sum: "|-|-3| + 1|" => 2.,
		magnitude_nested_open_product: "||-3|*2|" => 6.,
		magnitude_nested_open_difference: "||-3| - |-5||" => 2.,
		magnitude_triple_nested: "|||-3|||" => 3.,
		magnitude_triple_nested_differences: "|1 - |1 - |1 - 5|||" => 2.,
		magnitude_triple_open_run: "|||1|-2|-3|" => 2.,
		magnitude_product_of_nested: "|2*||-3|||" => 6.,

		// Or among magnitude bars, where `||` after an operand is Or unless two magnitudes are open
		or_spaced: "1 || 0" => 1.,
		or_unspaced: "0||1" => 1.,
		or_both_false: "0 || 0" => 0.,
		or_typeset: "1 ∨ 0" => 1.,
		or_of_parenthesized: "(1)||(0)" => 1.,
		or_of_parenthesized_magnitudes: "(|1|)||(|0|)" => 1.,
		or_of_magnitudes_spaced: "|1| || |0|" => 1.,
		or_of_magnitudes_typeset: "|1|∨|0|" => 1.,
		or_then_magnitude_spaced: "0 || |-1|" => 1.,
		or_then_magnitude_unspaced: "0|||-1|" => 1.,
		or_then_magnitude_bar_run_spaced: "0 ||| -1|" => 1.,
		or_magnitude_then_bar_run: "|0| ||| -1 |" => 1.,
		or_of_magnitude_first: "|-1| || 0" => 1.,
		or_of_double_bars: "||1|| || ||0||" => 1.,
		or_of_double_bars_unspaced: "||1||||||0||" => 1.,
		or_inside_both_false: "|0||0|" => 0.,
		or_inside_then_product: "|0 || 1| * 5" => 5.,
		or_inside_after_product: "2 * |0 || 1|" => 2.,
		or_inside_parenthesized: "|(1 || 0)|" => 1.,
		or_parenthesized_plus_magnitude: "(1 || 0) + |-2|" => 3.,
		or_inside_after_and: "1 && |0 || 1|" => 1.,
		or_after_and_inside: "|1 && 0| || 1" => 1.,
		or_after_comparison_of_magnitude: "|3 - 5| > 1 || 0" => 1.,
		or_as_condition: "{|-2| if 0 || 1, |-3| otherwise}" => 2.,
		not_of_magnitude: "!|0|" => 1.,
		not_of_magnitude_then_or: "!|0| || 0" => 1.,
		not_inside_magnitude: "|!0|" => 1.,
		not_inside_magnitude_with_or: "| !0 || 0 |" => 1.,

		// Ordered chains are one predicate over adjacent pairs, like interval notation, rather than `(a < b) < c`, while `!=` chains require every pair to differ
		chain_interval: "0 <= 0.5 < 1" => 1.,
		chain_fails_on_one_pair: "1 < 2 < 2" => 0.,
		chain_not_c_style: "3 < 2 < 1" => 0.,
		chain_descending: "3 > 2 >= 2" => 1.,
		chain_equality: "1 == 1 == 1" => 1.,
		chain_distinct: "1 != 2 != 3" => 1.,
		chain_distinct_all_pairs: "1 != 2 != 1" => 0.,

		// Correctly rounded literals via std parsing
		seventeen_digit_literal: "999999999999999999" => 1e18,
		long_fraction_literal: "0.1111111111111111111111111111111111111111" => 1. / 9.,

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

		// Quaternions: `*` is the Hamilton product on every rung, with `ijk = -1` and the bases anticommuting
		quaternion_basis_squares: "j*j + k*k" => -2.,
		quaternion_basis_product: "i*j*k" => -1.,
		quaternion_anticommutation: "i*j - j*i" => Quaternion::new(0., 0., 0., 2.),
		quaternion_square: "(1 + 2i + 3j + 4k)^2" => Quaternion::new(-28., 4., 6., 8.),
		quaternion_division_identity: "(3i + 4j) / (3i + 4j)" => 1.,
		quaternion_vector_square: "(2j) * (2j) + 4" => 0.,
		quaternion_magnitude: "|1 + 2i + 2j + 4k|" => 5.,
		quaternion_conjugate: "conj(1 + 2i + 3j + 4k)" => Quaternion::new(1., -2., -3., -4.),

		// Functions of one variable act in the value's own complex plane, spanned by `1` and its normalized vector part
		quaternion_sqrt_in_plane: "sqrt(2j)" => Quaternion::new(1., 0., 1., 0.),
		quaternion_exp_in_plane: "exp(pi k)" => -1.,
		quaternion_log_in_plane: "log(k, e)" => Quaternion::new(0., 0., 0., std::f64::consts::FRAC_PI_2),
		quaternion_factorial_in_plane: "j!" => Quaternion::new(0.498015668118356, 0., -0.1549498283018107, 0.),

		// Mapping, extremum, and interpolation functions act part by part, and magnitudes feed `hypot`
		quaternion_floor_componentwise: "floor(1.5i + 2.5j)" => Quaternion::new(0., 1., 2., 0.),
		quaternion_snap_componentwise: "snap(0.4 + 1.6j - 2.8k, 2)" => Quaternion::new(0., 0., 2., -2.),
		quaternion_min_componentwise: "min(1 + 5j, 3 + 2j)" => Quaternion::new(1., 0., 2., 0.),
		quaternion_clamp_componentwise: "clamp(5i - 5j, (-i - j)..(i + j))" => Quaternion::new(0., 1., -1., 0.),
		quaternion_lerp: "lerp(2i, 4j, 0.5)" => Quaternion::new(0., 1., 2., 0.),
		quaternion_mean_pointwise: "mean(2i, 4j)" => Quaternion::new(0., 1., 2., 0.),
		quaternion_abs_per_part: "abs(-1 - 2i + 3j - 4k)" => Quaternion::new(1., 2., 3., 4.),
		quaternion_hypot_magnitudes: "hypot(3i + 4j, 12)" => 13.,

		// A real is a quaternion with zero vector parts, so as a bound it holds a vector's parts to zero
		quaternion_max_with_zero: "max(2i - 3j, 0)" => Complex::new(0., 2.),
		quaternion_max_with_real: "max(0.5i + 2j, 1)" => Quaternion::new(1., 0.5, 2., 0.),
		quaternion_clamp_spans_the_weight_alone: "clamp(3 + 3i, 1..2)" => Complex::new(2., 3.),

		// Vector functions: the dot product spans all four parts, the cross product only the vector parts
		vector_dot: "dot(3i + 4j, i)" => 3.,
		vector_dot_with_weight: "dot(1 + 2i, 3 + 4i)" => 11.,
		vector_cross: "cross(i, j)" => Quaternion::K,
		vector_cross_ignores_weight: "cross(5 + i, 7 + j)" => Quaternion::K,
		vector_normalize: "normalize(3i + 4j)" => Quaternion::new(0., 0.6, 0.8, 0.),
		vector_distance: "distance(i, j)" => 2f64.sqrt(),
		vector_project: "project(3i + 4j, i)" => Quaternion::new(0., 3., 0., 0.),
		vector_reject: "reject(3i + 4j, i)" => Quaternion::new(0., 0., 4., 0.),
		vector_reflect: "reflect(i + j, j)" => Quaternion::new(0., 1., -1., 0.),
		vector_perp: "perp(3i + 4j)" => Quaternion::new(0., -4., 3., 0.),

		// Angles are signed by the turn seen from `+k`, an inclination is positive, and a rotor's angle is `2 angle(1, q)`
		vector_angle_counterclockwise: "angle(i, j)" => std::f64::consts::FRAC_PI_2,
		vector_angle_clockwise: "angle(j, i)" => -std::f64::consts::FRAC_PI_2,
		vector_angle_opposite: "angle(i, -i)" => std::f64::consts::PI,
		vector_angle_inclination: "angle(k, i)" => std::f64::consts::FRAC_PI_2,
		vector_angle_of_rotor: "2 angle(1, rotor(1))" => 1.,

		// Rotation is the rotor sandwich about `k` unless an axis is given, and never touches the weight
		vector_rotate: "rotate(i, pi/2)" => Quaternion::J,
		vector_rotate_about_axis: "rotate(i, pi/2, j)" => Quaternion::new(0., 0., 0., -1.),
		vector_rotate_keeps_weight: "rotate(5 + i, pi)" => Quaternion::new(5., -1., 0., 0.),
		vector_rotor: "rotor(pi)" => Quaternion::K,
		vector_axis: "axis(rotor(1, 2j))" => Quaternion::J,
		vector_slerp: "slerp(1, k, 0.5)" => Quaternion::new(std::f64::consts::FRAC_1_SQRT_2, 0., 0., std::f64::consts::FRAC_1_SQRT_2),

		// Tiny, huge, and infinite values keep their directions
		vector_normalize_huge: "normalize(1e308 + 1e308i + 1e308j + 1e308k)" => Quaternion::splat(0.5),
		vector_normalize_infinite: "normalize(inf j)" => Quaternion::J,
		vector_normalize_infinite_beside_finite: "normalize(5i - inf j)" => -Quaternion::J,
		vector_angle_tiny: "angle(1e-200 i, 1e-200 j)" => std::f64::consts::FRAC_PI_2,
		vector_angle_huge: "angle(1e200 i, 1e200 i + 1e200 j)" => std::f64::consts::FRAC_PI_4,
		vector_angle_infinite: "angle(inf i, j)" => std::f64::consts::FRAC_PI_2,
		vector_angle_infinite_beside_finite: "angle(inf i + 5j, i)" => 0.,
		vector_project_tiny: "project(1e-200 j, 1e-200 j) / 1e-200" => Quaternion::J,
		vector_project_huge: "project(1e200 j, 1e200 j) / 1e200" => Quaternion::J,
		vector_reject_huge: "reject(1e200 i + 1e200 j, 1e200 j) / 1e200" => Quaternion::I,
		vector_slerp_tiny: "slerp(1e-310 i, 1e-310 j, 0.5) / 1e-310" => Quaternion::new(0., std::f64::consts::FRAC_1_SQRT_2, std::f64::consts::FRAC_1_SQRT_2, 0.),
		vector_slerp_huge: "slerp(1e308 + 1e308i + 1e308j + 1e308k, 1e308 - 1e308i + 1e308j + 1e308k, 0.5) / 1e308" => Quaternion::new(2. / 3_f64.sqrt(), 0., 2. / 3_f64.sqrt(), 2. / 3_f64.sqrt()),

		// An infinite vector's zero parts are absent axes here too, and overflowing terms that cancel do so
		vector_dot_infinite_perpendicular: "dot(inf i, j)" => 0.,
		vector_dot_infinite_parallel: "dot(inf i, i)" => f64::INFINITY,
		vector_dot_overflow_cancels: "dot(1e200 i + 1e200 j, 1e200 i - 1e200 j)" => 0.,
		vector_cross_infinite: "cross(inf i, j)" => Quaternion::new(0., 0., 0., f64::INFINITY),
		vector_cross_huge_with_itself: "cross(1e200 i + 1e200 j, 1e200 i + 1e200 j)" => 0.,
		vector_cross_beside_huge_weights: "cross(1.7e308 + 5i + 1e200 j + 1e200 k, 1.7e308 + i + 1e200 j + 1e200 k) / 1e200" => Quaternion::new(0., 0., -4., 4.),
		vector_project_infinite: "project(inf i, i)" => Complex::new(0., f64::INFINITY),
		vector_project_onto_infinite: "project(i, inf i)" => Complex::new(0., 1.),
		vector_reject_infinite: "reject(inf i, j)" => Complex::new(0., f64::INFINITY),

		// Matrix literals build rows or columns, landing on the rung their count names
		matrix_identity_literal: "[1;i;j;k]" => Matrix::IDENTITY,
		matrix_short_rows_pad: "[i;j]" => Matrix::from_rows(&[Quaternion::I, Quaternion::J]).unwrap(),
		matrix_one_row_is_the_weight: "[3i + 4j]" => Matrix::from_rows(&[Quaternion::new(0., 3., 4., 0.)]).unwrap(),
		matrix_columns_transpose_rows: "[1,i]" => Matrix::from_columns(&[Quaternion::ONE, Quaternion::I]).unwrap(),
		matrix_swizzle: "[k;j;i] (1i + 2j + 3k)" => Quaternion::new(0., 3., 2., 1.),
		matrix_real_part: "[1] (3 + 4i)" => 3.,
		matrix_coefficient: "[j] (1i + 2j + 3k)" => 2.,
		matrix_argand_shuffle: "[1;i] (3 + 4i)" => Quaternion::new(0., 3., 4., 0.),
		matrix_argand_shuffle_inverse: "[i;j;0;0] (3i + 4j)" => Complex::new(3., 4.),
		matrix_splat: "[1;1] 5" => Quaternion::new(0., 5., 5., 0.),
		matrix_positional_construction: "[3;4] 1" => Quaternion::new(0., 3., 4., 0.),
		matrix_scaled_pick: "[1;i;2j;-k] (1 + 2i + 3j + 4k)" => Quaternion::new(1., 2., 6., -4.),
		matrix_projection: "[i;j;0] (1i + 2j + 3k)" => Quaternion::new(0., 1., 2., 0.),
		matrix_dot_product: "[3i + 4j] (i + j)" => 7.,
		matrix_columns_apply: "[j, i] (3i + 4j)" => Quaternion::new(0., 4., 3., 0.),
		matrix_skew_literal: "[i + 0.5j, j] i" => Quaternion::new(0., 1., 0.5, 0.),

		// Products: a matrix applies to a value and composes with a matrix, and a value on the left acts as `L_q`
		matrix_composition: "[i;j] [j;i] (i + 2j)" => Quaternion::new(0., 2., 1., 0.),
		matrix_scaled_by_value: "(2 I) (3i)" => Complex::new(0., 6.),
		matrix_left_multiplication: "(i I) j" => Quaternion::K,
		matrix_implicit_scalar: "2[i;j] (i + j)" => Quaternion::new(0., 2., 2., 0.),
		matrix_call_syntax_applies: "I(3i)" => Complex::new(0., 3.),
		matrix_zero_entry_beside_infinity: "[1] (inf i)" => 0.,
		matrix_infinite_part_passes: "[i] (inf i)" => f64::INFINITY,

		// Sums attach a translation, which `A 0` reads back
		matrix_translation: "(I + 5i + 4j) 0" => Quaternion::new(0., 5., 4., 0.),
		matrix_translation_first: "(5i + I + 4j) 0" => Quaternion::new(0., 5., 4., 0.),
		matrix_translation_subtracted: "(I - 2(-2.5i - 2j)) 0" => Quaternion::new(0., 5., 4., 0.),
		matrix_linear_part: "(I + 5i) - (I + 5i) 0" => Matrix::IDENTITY,
		matrix_translation_function: "translation(I + 5i)" => Complex::new(0., 5.),
		matrix_linear_function: "linear(I + 5i)" => Matrix::IDENTITY,
		matrix_pointwise_sum: "I + I" => Matrix::linear([Quaternion::new(2., 0., 0., 0.), Quaternion::new(0., 2., 0., 0.), Quaternion::new(0., 0., 2., 0.), Quaternion::new(0., 0., 0., 2.)]),
		matrix_pointwise_difference: "I - I" => Matrix::ZERO,
		matrix_value_minus_matrix: "(5i - I) 0" => Complex::new(0., 5.),
		matrix_affine_composition: "((I + i) (I + j)) 0" => Quaternion::new(0., 1., 1., 0.),

		// Division is times-inverse on every sort, and whole powers compose
		matrix_inverse_application: "[1;2i;3j;k]^-1 (2i + 3j)" => Quaternion::new(0., 1., 1., 0.),
		matrix_square: "[1;2i;3j;k]^2 (i + j)" => Quaternion::new(0., 4., 9., 0.),
		matrix_zeroth_power: "[1;2i;3j;k]^0" => Matrix::IDENTITY,
		matrix_over_matrix: "(I + 5i) / (I + 5i)" => Matrix::IDENTITY,
		matrix_over_value: "I / 2" => 0.5,
		value_over_matrix: "(2 / I) 3" => 6.,
		matrix_transpose: "[1;i]^T" => Matrix::from_columns(&[Quaternion::ONE, Quaternion::I]).unwrap(),
		matrix_transpose_then_inverse: "[1;2i;3j;k]^T^-1 (2i + 3j)" => Quaternion::new(0., 1., 1., 0.),
		matrix_determinant: "det([1;2i;3j;k])" => 6.,
		matrix_determinant_of_padded_literal: "det([2i;3j])" => 0.,
		matrix_determinant_of_left_multiplication: "det(matrix(3 + 4i))" => 625.,

		// Builders
		matrix_of_value_multiplies: "matrix(1 + i) (2 + j)" => Quaternion::new(2., 2., 1., 1.),
		matrix_rotation: "rotation(pi/2) i" => Quaternion::J,
		matrix_rotation_about_axis: "rotation(pi/2, i) j" => Quaternion::K,
		matrix_rotation_keeps_weight: "rotation(pi) (1 + i)" => Complex::new(1., -1.),
		matrix_rotation_matches_rotate: "rotation(1, i + j) (2i + 3k) - rotate(2i + 3k, 1, i + j)" => 0.,
		matrix_uniform_scale: "scale(2) (1 + i + j)" => Quaternion::new(1., 2., 2., 0.),
		matrix_componentwise_product: "scale(3i + 2j) (5i + 7j)" => Quaternion::new(0., 15., 14., 0.),
		matrix_componentwise_quotient: "scale(3i + 2j + k)^-1 (6i + 4j)" => Quaternion::new(0., 2., 2., 0.),
		matrix_shear: "shear(i, j, 0.5) (2j)" => Quaternion::new(0., 1., 2., 0.),
		matrix_shear_infinite: "shear(i, j, inf) (2j)" => Quaternion::new(0., f64::INFINITY, 2., 0.),
		matrix_shear_infinite_leaves_other_axes: "shear(i, j, inf) i" => Complex::new(0., 1.),

		// Inverses scale their entries first, so a huge or tiny map inverts, and a whole power reads its exponent exactly
		matrix_inverse_of_huge_scale: "scale(1e103)^-1 (1e103 i)" => Complex::new(0., 1.),
		matrix_inverse_of_tiny_scale: "scale(1e-110)^-1 (1e-110 i)" => Complex::new(0., 1.),
		matrix_power_past_exact_reals: "(-I)^(2^53 + 1) i" => Complex::new(0., -1.),

		// A map keeps the axes it acts on through its linear part and a real scale factor
		matrix_linear_keeps_axes: "linear((i + j)..(3i + 3j))" => Matrix::range(Quaternion::ZERO, Quaternion::new(0., 2., 2., 0.)),
		inside_linear_part_of_range: "inside(5 + i, linear(0..(2i + 2j)))" => 1.,
		inside_scaled_range: "inside(1.5 + 7i, 2 (0..1))" => 1.,
		outside_left_multiplication: "inside(3, matrix(2))" => 0.,

		// Comparisons are pointwise, a piecewise may take matrix values, and `\I` reaches the identity past any binding
		matrix_equality: "I == [1;i;j;k]" => 1.,
		matrix_inequality: "I != [i;j]" => 1.,
		matrix_equality_chain: "I == [1;i;j;k] == I" => 1.,
		matrix_distinct_chain: "I != [i;j] != [1]" => 1.,
		matrix_in_piecewise: "{I if 1, [i;j] otherwise} k" => Quaternion::K,
		matrix_builtin_identity_prefix: "\\I k" => Quaternion::K,

		// A range sends parameter 0 to its first corner and 1 to its second on the parts the corners have, leaving the others untouched
		range_application: "(0..10) 0.5" => 5.,
		range_reversed: "(10..0) 0.25" => 7.5,
		range_reaches_past_products: "(0..2pi) 0.5" => std::f64::consts::PI,
		range_unit_is_identity: "0..1 == I" => 1.,
		range_normalization: "(2..4)^-1 (3)" => 0.5,
		range_leaves_other_parts: "(0..10) (0.5 + 3i)" => Complex::new(5., 3.),
		box_scales_its_axes: "(0..(3i + 4j)) (0.5i + 0.5j)" => Quaternion::new(0., 1.5, 2., 0.),
		box_leaves_the_weight: "(0..(3i + 4j)) (1 + 0.5i)" => Complex::new(1., 1.5),
		box_from_a_corner: "((2i + 2j)..(4i + 6j)) (0.5i + 0.5j)" => Quaternion::new(0., 3., 4., 0.),
		box_over_every_axis: "(0..(5 + 6i + 7j + 8k)) (1 + i + j + k)" => Quaternion::new(5., 6., 7., 8.),
		box_parameter_is_per_axis: "(0..(5 + 6i + 7j + 8k)) 1" => 5.,
		range_composes_with_a_rotation: "(rotation(pi/2) (0..(2i + 2j))) (i + j)" => Quaternion::new(0., -2., 2., 0.),
		range_shifted_by_a_translation: "((0..(i + j)) + 5i) (i + j)" => Quaternion::new(0., 6., 1., 0.),

		// Insideness, clamping, and remapping read the range's parameter on the axes it spans, boundary included
		inside_range: "inside(0.5, 0..1)" => 1.,
		inside_range_boundary: "inside(1, 0..1)" => 1.,
		outside_range: "inside(1.5, 0..1)" => 0.,
		inside_range_ignores_other_parts: "inside(0.5 + 7i, 0..1)" => 1.,
		inside_box: "inside(2i + 3j, 0..(4i + 4j))" => 1.,
		outside_box_on_one_axis: "inside(2i + 5j, 0..(4i + 4j))" => 0.,
		inside_box_ignores_the_weight: "inside(9 + 2i + 3j, 0..(4i + 4j))" => 1.,
		inside_rotated_box: "inside(0.1i + 0.5j, rotation(pi/4) (0..(i + j)))" => 1.,
		outside_rotated_box: "inside(0.9i + 0.5j, rotation(pi/4) (0..(i + j)))" => 0.,
		inside_parallelogram: "inside(2i + j, [2i, i + j] + i)" => 1.,
		outside_parallelogram: "inside(i + j, [2i, i + j] + i)" => 0.,
		clamp_to_range: "clamp(1.5, 0..1)" => 1.,
		clamp_within_range: "clamp(0.25, 0..1)" => 0.25,
		clamp_leaves_other_parts: "clamp(-3 + 7i, 0..1)" => Complex::new(0., 7.),
		clamp_to_box: "clamp(2i + 3j, 0..(i + j))" => Quaternion::new(0., 1., 1., 0.),
		clamp_within_box: "clamp(0.5i, 0..(i + j))" => Complex::new(0., 0.5),
		clamp_to_rotated_box: "clamp(2i, rotation(pi/2) (0..(i + j)))" => 0.,
		remap_between_ranges: "remap(0.25i + 0.5j, 0..(i + j), 0..(2i + 4j))" => Quaternion::new(0., 0.5, 2., 0.),
		remap_reversing: "remap(2, 0..10, 100..0)" => 80.,
		inside_huge_box: "inside(5e200 i, 0..(1e201 i + 1e201 j + 1e201 k))" => 1.,
		inside_tiny_box: "inside(5e-111 i, 0..(1e-110 i + 1e-110 j + 1e-110 k))" => 1.,
		remap_from_huge_box: "remap(5e200 i, 0..(1e201 i + 1e201 j + 1e201 k), 0..(i + j + k))" => Complex::new(0., 0.5),
	}

	#[test]
	fn range_syntax() {
		// A range's corners are values, a range is a matrix, and a range cannot chain
		let message = |input: &str| evaluate(input).unwrap_err().to_string();
		assert_eq!(message("I..1"), "A matrix stands where a value is needed");
		assert_eq!(message("sin(0..1)"), "A matrix stands where a value is needed");
		assert_eq!(message("inside(0..1, 0..1)"), "A matrix stands where a value is needed");
		assert_eq!(message("inside(1, 2)"), "A value stands where a matrix is needed");
		assert_eq!(message("0..1 < 2"), "The operator has no meaning for a matrix");
		assert!(evaluate("0..1..2").is_err());

		// A number's decimal point still lexes beside a range, and whitespace around `..` is free
		assert_eq!(evaluate("(1.5..2.5) 0.5").unwrap().unwrap().as_real(), Some(2.));
		assert_eq!(evaluate("(1 .. 3) 0.5").unwrap().unwrap().as_real(), Some(2.));
		assert_eq!(evaluate("(-1..1) 0.75").unwrap().unwrap().as_real(), Some(0.5));

		// A singular range has no interior to test or clamp against, while its application still stands
		for input in ["inside(5, 5..5)", "inside(0, 0..0)", "clamp(1, 3..3)", "remap(1, 2..2, 0..1)"] {
			assert!(matches!(evaluate(input).unwrap(), Err(EvalError::SingularRange)), "`{input}`");
		}
		assert_eq!(evaluate("(5..5) 0.5").unwrap().unwrap().as_real(), Some(5.));
		assert!(matches!(evaluate("inside(1)").unwrap(), Err(EvalError::TypeError)));

		// The axes a range spans come from its corners, so `0..1` and `I` agree entry for entry yet test different parts
		assert_eq!(evaluate("inside(2i, 0..1)").unwrap().unwrap().as_bool(), Some(true));
		assert_eq!(evaluate("inside(2i, I)").unwrap().unwrap().as_bool(), Some(false));
		assert_eq!(evaluate("inside(0.5i, 0..1i)").unwrap().unwrap().as_bool(), Some(true));
		assert_eq!(evaluate("inside(0.5i + 3j, 0..1i)").unwrap().unwrap().as_bool(), Some(true));
		assert_eq!(evaluate("inside(0.5i + 3j, 0..(i + j))").unwrap().unwrap().as_bool(), Some(false));

		// A range reads as a Transform when its corners leave the weight and `z` alone
		let affine = |input: &str| evaluate(input).unwrap().unwrap().into_matrix().unwrap().as_affine2();
		assert_eq!(
			affine("(i + j)..(3i + 4j)"),
			Some(Affine2 {
				linear: Linear2([[2., 0.], [0., 3.]]),
				translation: [1., 1.]
			})
		);
		assert_eq!(affine("0..10"), None);
	}

	#[test]
	fn vector_functions_without_a_direction_are_errors() {
		// A zero vector has no direction to normalize, measure an angle from, rotate about, or project onto
		for input in ["normalize(0)", "angle(0, i)", "axis(1)", "rotate(i, 1, 0)", "rotor(1, 0)", "project(i, 0)", "reflect(i, 0)"] {
			assert!(evaluate(input).unwrap().is_err(), "expected `{input}` to be an evaluation error");
		}
	}

	#[test]
	fn vectors_have_no_order() {
		// Ordering is real-only, and logic needs truth values
		for input in ["i < j", "!j", "(1 + j) && 1", "median(j, k)"] {
			assert!(evaluate(input).unwrap().is_err(), "expected `{input}` to be an evaluation error");
		}
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
	fn vector_queries_require_the_other_parts_to_be_zero() {
		let value = |source: &str| evaluate(source).unwrap().unwrap().into_value().unwrap();

		// A query succeeds exactly when the parts outside its rung are zero, so a real is a particle but not a vector
		assert_eq!(value("3i + 4j").as_vector2(), Some(Vector2([3., 4.])));
		assert_eq!(value("3i + 4j").as_vector1(), None);
		assert_eq!(value("3i + 4j").as_vector3(), Some(Vector3([3., 4., 0.])));
		assert_eq!(value("3i + 4j").as_particle2(), Some(Weighted { w: 0., vector: Vector2([3., 4.]) }));
		assert_eq!(value("1 + 2i").as_particle1(), Some(Weighted { w: 1., vector: Vector1(2.) }));
		assert_eq!(value("1 + 2i").as_vector1(), None);
		assert_eq!(value("2").as_vector1(), None);
		assert_eq!(value("2").as_particle1(), Some(Weighted { w: 2., vector: Vector1(0.) }));
		assert_eq!(value("i * j").as_vector3(), Some(Vector3([0., 0., 1.])));
		assert_eq!(value("i * j").as_vector2(), None);
		assert_eq!(value("1 + i + j + k").as_particle3(), Weighted { w: 1., vector: Vector3([1., 1., 1.]) });

		// Zero sits on every rung
		assert_eq!(value("0").as_vector2(), Some(Vector2([0., 0.])));
	}

	#[test]
	fn vectors_bind_from_the_host() {
		struct VectorBindings;
		impl context::ValueProvider for VectorBindings {
			fn get_value(&self, name: &str) -> Option<Value> {
				match name {
					"v" => Some(Value::from(Vector2([3., 4.]))),
					"p" => Some(Value::from(Weighted { w: 2., vector: Vector1(1.) })),
					_ => None,
				}
			}
		}
		let eval = |source: &str| {
			ast::Node::try_parse_from_str(source)
				.unwrap()
				.eval(&EvalContext::new(VectorBindings, context::NothingMap))
				.unwrap()
				.into_value()
				.unwrap()
		};

		// A bound vector takes part in the algebra like any literal, and the result queries back at its rung
		assert_eq!(eval("|v|").as_real(), Some(5.));
		assert_eq!(eval("2v").as_vector2(), Some(Vector2([6., 8.])));
		assert_eq!(eval("v * conj(v)").as_real(), Some(25.));
		assert_eq!(eval("p * p").as_particle1(), Some(Weighted { w: 3., vector: Vector1(4.) }));
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

		// An integer reads exactly into every type with room for it, as does a whole real past integer storage
		assert_eq!(evaluate("2^62").unwrap().unwrap().as_i64(), Some(1 << 62));
		assert_eq!(evaluate("2^63").unwrap().unwrap().as_u64(), Some(1 << 63));
		assert_eq!(evaluate("2^63").unwrap().unwrap().as_i32(), None);

		// The range ends exclusively at the power of two past each type
		assert_eq!(evaluate("2^63").unwrap().unwrap().as_i64(), None);
		assert_eq!(evaluate("2^64").unwrap().unwrap().as_u64(), None);

		// A truth value is exactly 0 or 1
		assert_eq!(evaluate("2 > 1").unwrap().unwrap().as_bool(), Some(true));
		assert_eq!(evaluate("0.5").unwrap().unwrap().as_bool(), None);

		// A host's value reads by its content, whatever storage built it
		assert_eq!(Value::from(Quaternion::ONE).as_bool(), Some(true));
		assert_eq!(Value::from(Complex::new(2., 0.)).as_i64(), Some(2));
	}

	#[test]
	fn huge_quaternion_inverse() {
		// The norm overflows, but the inverse's parts are representable
		let inverse = Quaternion::splat(1e308).inverse();
		for (part, expected) in inverse.parts().into_iter().zip([1., -1., -1., -1.]) {
			assert!((part * 1e308 * 4. - expected).abs() < EPSILON, "expected {expected}, got {}", part * 1e308 * 4.);
		}
	}

	#[test]
	fn integer_arithmetic_is_exact_beyond_the_reals_limit() {
		let evaluate_i64 = |source: &str| evaluate(source).unwrap().unwrap().as_i64();

		// Whole numbers stay in integer storage, so a step past 2^53 that the reals cannot represent is kept
		assert_eq!(evaluate_i64("2^53 + 1"), Some((1_i64 << 53) + 1));
		assert_eq!(evaluate_i64("9007199254740993 - 9007199254740992"), Some(1));
		assert_eq!(evaluate_i64("10^18 / 4"), Some(250_000_000_000_000_000));
		assert_eq!(evaluate_i64("mod(-2^53 - 1, 2)"), Some(1));
		assert_eq!(evaluate_i64("mod(2^53 + 1, -2)"), Some(-1));
		assert_eq!(evaluate_i64("20!"), Some(2_432_902_008_176_640_000));
		assert_eq!(evaluate_i64("-2147483648 * -2147483648"), Some(1 << 62));
		assert_eq!(evaluate_i64("3037000499 * 3037000499"), Some(9_223_372_030_926_249_001));
		assert_eq!(evaluate_i64("gcd(2^62, 2^40 * 3)"), Some(1 << 40));
		assert_eq!(evaluate_i64("choose(66, 33)"), Some(7_219_428_434_016_265_740));

		// Selection and rounding return the integer itself, and a literal spelled as a real is the same whole number
		assert_eq!(evaluate_i64("max(2^53 + 1, 2^53)"), Some((1_i64 << 53) + 1));
		assert_eq!(evaluate_i64("min(2^53 + 1, 2^53 + 2)"), Some((1_i64 << 53) + 1));
		assert_eq!(evaluate_i64("clamp(2^53 + 1, 0..2^60)"), Some((1_i64 << 53) + 1));
		assert_eq!(evaluate_i64("floor(2^53 + 1)"), Some((1_i64 << 53) + 1));
		assert_eq!(evaluate_i64("snap(2^53 + 1, 1)"), Some((1_i64 << 53) + 1));
		assert_eq!(evaluate_i64("snap(2^60 + 3, 2)"), Some((1_i64 << 60) + 4));
		assert_eq!(evaluate_i64("lerp(2^53 + 1, 2^53 + 3, 1)"), Some((1_i64 << 53) + 3));
		assert_eq!(evaluate_i64("mean(2^53 + 1, 2^53 + 3)"), Some((1_i64 << 53) + 2));
		assert_eq!(evaluate_i64("9007199254740993 + 1.0"), Some((1_i64 << 53) + 2));

		// Integer storage reaches its own lower bound, whose magnitude no literal can spell
		assert_eq!(evaluate_i64("-9223372036854775808 + 1"), Some(i64::MIN + 1));

		// A whole real reads as an integer operand only within the widest storage
		assert_eq!(evaluate_i64("gcd(2^126, 6)"), Some(2));
		assert!(evaluate("gcd(2^127, 2)").unwrap().is_err());

		// A reversed range clamps to its own ends
		assert_eq!(evaluate_i64("clamp(15, 10..0)"), Some(10));
		assert_eq!(evaluate_i64("clamp(-5, 10..0)"), Some(0));

		// A fractional quotient, a power, and a factorial past integer storage continue in the reals
		assert_eq!(evaluate("7 / 2").unwrap().unwrap().as_real(), Some(3.5));
		assert_eq!(evaluate("2^63").unwrap().unwrap().as_real(), Some(9_223_372_036_854_775_808.));
		assert_eq!(evaluate("34!").unwrap().unwrap().as_real(), Some((1..=34).fold(1., |accumulated, k| accumulated * k as f64)));
	}

	#[test]
	fn mixed_storage_compares_by_value() {
		let evaluate_bool = |source: &str| evaluate(source).unwrap().unwrap().as_bool();

		// An integer beside a real past integer storage compares by value
		assert_eq!(evaluate_bool("9223372036854775807 < 2^63"), Some(true));
		assert_eq!(evaluate_bool("9223372036854775807 == 2^63"), Some(false));
		assert_eq!(evaluate_bool("-9223372036854775807 - 1 == -2^63"), Some(true));
		assert_eq!(evaluate_bool("-9223372036854775807 - 1 > -2^64"), Some(true));

		// An integer past the reals' 2^53 limit orders exactly against a fraction
		assert_eq!(evaluate_bool("9007199254740993 > 0.5"), Some(true));
		assert_eq!(evaluate_bool("-9007199254740993 < -0.5"), Some(true));

		// A whole real a host supplies without canonicalizing compares the same way
		assert_ne!(Value::from_i64((1 << 53) + 1), Value::from_f64((1_i64 << 53) as f64));
		assert_eq!(Value::from_i64(1 << 53), Value::from_f64((1_i64 << 53) as f64));

		// NaN is unordered against an integer, as against a real
		let nan = Number::Real(f64::NAN);
		assert_eq!(Number::Integer(0).binary_op(ast::BinaryOp::Leq, nan), Some(Number::from_bool(false)));
		assert_eq!(nan.binary_op(ast::BinaryOp::Geq, Number::Integer(0)), Some(Number::from_bool(false)));
	}

	#[test]
	fn matrix_sort_errors() {
		// Sorts are fixed by spelling, so a matrix or a value standing where the other belongs fails the parse
		let message = |input: &str| evaluate(input).unwrap_err().to_string();
		for input in ["[I]", "sin(I)", "max(I, 1)", "I + [I]", "{1 if I, 0 otherwise}"] {
			assert_eq!(message(input), "A matrix stands where a value is needed", "`{input}`");
		}
		for input in ["2^T", "det(1)"] {
			assert_eq!(message(input), "A value stands where a matrix is needed", "`{input}`");
		}
		// Matrices have no order, magnitude, factorial, or logic, and compare only with matrices
		for input in ["I < I", "|I|", "I!", "!I", "I && 1", "I == 1", "I^I", "1^I", "I < 1 < 2"] {
			assert_eq!(message(input), "The operator has no meaning for a matrix", "`{input}`");
		}
		assert_eq!(message("{I if 1, 2 otherwise}"), "A piecewise's cases must all be values or all be matrices");
		assert_eq!(message("I(1, 2)"), "Invalid arguments for function call");

		// A fractional power, the inverse of a padded literal (whose zero rows make it singular), and the transpose of a translated matrix fail at evaluation
		assert!(matches!(evaluate("I^0.5").unwrap(), Err(EvalError::OperatorTypeError)));
		for input in ["[i;j]^-1", "I / [i;j]", "[i;j]^-2"] {
			assert!(matches!(evaluate(input).unwrap(), Err(EvalError::SingularMatrix)), "expected `{input}` to be singular");
		}
		assert!(matches!(evaluate("(I + 5i)^T").unwrap(), Err(EvalError::AffineTranspose)));
		assert!(matches!(evaluate("M").unwrap(), Err(EvalError::MissingValue(name)) if name == "M"));
	}

	#[test]
	fn matrix_literal_syntax_errors() {
		for input in ["[]", "[1; i, j]", "[1 2]", "[1;]", "[;1]", "[1,]"] {
			assert!(evaluate(input).is_err(), "expected `{input}` to be a parse error");
		}

		let error = evaluate("[1;i;j;k;1]").unwrap_err().to_string();
		assert!(error.starts_with("A matrix literal has at most four entries"), "{error}");
	}

	#[test]
	fn matrices_bind_and_query_by_case() {
		struct Bindings;
		impl context::ValueProvider for Bindings {
			fn get_value(&self, name: &str) -> Option<Value> {
				(name == "x").then(|| Value::from_f64(2.))
			}
			fn get_matrix(&self, name: &str) -> Option<Matrix> {
				match name {
					// A shear with a translation, as a host's `DAffine2` binds
					"X" => Some(Matrix::from(Affine2 {
						linear: Linear2([[1., 0.], [0.5, 1.]]),
						translation: [5., 4.],
					})),
					// A binding shadows the identity like any constant
					"I" => Some(Matrix::ZERO),
					_ => None,
				}
			}
		}
		let eval = |source: &str| ast::Node::try_parse_from_str(source).unwrap().eval(&EvalContext::new(Bindings, context::NothingMap));

		// `X` maps the plane, leaving the weight and `z` untouched
		assert_eq!(eval("X (2i + 2j)").unwrap(), Object::from(Quaternion::new(0., 8., 6., 0.)));
		assert_eq!(eval("X (1 + k)").unwrap(), Object::from(Quaternion::new(1., 5., 4., 1.)));
		assert_eq!(eval("x X 0").unwrap(), Object::from(Quaternion::new(0., 10., 8., 0.)));
		assert_eq!(eval("I").unwrap(), Object::from(Matrix::ZERO));
		assert_eq!(eval("\\I").unwrap(), Object::from(Matrix::IDENTITY));
		assert!(matches!(eval("Y"), Err(EvalError::MissingValue(name)) if name == "Y"));

		// A query checks its refinement losslessly: the weight untouched for all, `z` untouched for the plane, no translation for a linear map
		let x = eval("X").unwrap().into_matrix().unwrap();
		assert_eq!(
			x.as_affine2(),
			Some(Affine2 {
				linear: Linear2([[1., 0.], [0.5, 1.]]),
				translation: [5., 4.]
			})
		);
		assert_eq!(x.as_linear2(), None);
		assert_eq!(x.as_affine3().map(|affine| affine.translation), Some([5., 4., 0.]));
		assert_eq!(eval("linear(X)").unwrap().into_matrix().unwrap().as_linear2(), Some(Linear2([[1., 0.], [0.5, 1.]])));
		let rotation = eval("rotation(1, i)").unwrap().into_matrix().unwrap();
		assert_eq!(rotation.as_affine2(), None);
		assert!(rotation.as_linear3().is_some());
		assert_eq!(eval("[1;i]").unwrap().into_matrix().unwrap().as_affine3(), None);

		// A matrix is no value to the value readers
		assert_eq!(eval("X").unwrap().as_real(), None);
	}

	#[test]
	fn matrices_display_as_row_literals() {
		for (input, expected) in [
			("I", "[1;1i;1j;1k]"),
			("[i;j]", "[1i;1j]"),
			("[3i + 4j]", "[3i+4j]"),
			("[1;i]^T", "[1i;1j;0;0]"),
			("I - I", "[0]"),
			("I + 5i", "[1;1i;1j;1k] + 5i"),
			("I - 5i", "[1;1i;1j;1k] + (-5i)"),
			("I + 5i + 4j", "[1;1i;1j;1k] + (5i+4j)"),
			("2..4", "[2;1i;1j;1k] + 2"),
			("0..(3i + 4j)", "[1;3i;4j;1k]"),
		] {
			assert_eq!(evaluate(input).unwrap().unwrap().to_string(), expected, "`{input}`");
		}
	}

	#[test]
	fn minimal_rungs_follow_content() {
		let rung = |source: &str| evaluate(source).unwrap().unwrap().as_value().unwrap().rung();

		// The rung is decided by the value rather than by how it was computed or stored
		assert_eq!(rung("1 < 2"), Rung::Bool);
		assert_eq!(rung("sqrt(4)"), Rung::Integer);
		assert_eq!(rung("2.5 * 2"), Rung::Integer);
		assert_eq!(rung("i * i"), Rung::Integer);
		assert_eq!(rung("0.5"), Rung::Number);
		assert_eq!(rung("inf"), Rung::Number);
		assert_eq!(rung("1 + 2i"), Rung::Particle1);

		// The vector rungs are the weightless refinements, each named by its highest nonzero axis, so a domain climb
		// with no real part lands on one
		assert_eq!(rung("sqrt(-4)"), Rung::Vector1);
		assert_eq!(rung("3i"), Rung::Vector1);
		assert_eq!(rung("3i + 4j"), Rung::Vector2);
		assert_eq!(rung("i * j"), Rung::Vector3);
		assert_eq!(rung("1 + 3i + 4j"), Rung::Particle2);
		assert_eq!(rung("1 + k"), Rung::Particle3);
		assert_eq!(rung("j * j"), Rung::Integer);
		assert_eq!(rung("j^2"), Rung::Integer);
	}
}
