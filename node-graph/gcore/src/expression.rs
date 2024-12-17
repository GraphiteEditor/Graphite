use math_parser::ast;
use math_parser::context::{EvalContext, NothingMap, ValueProvider};
use math_parser::value::{Number, Value};

/// The struct that stores the context for the maths parser.
/// This is currently just limited to supplying `a` and `b` until we add better node graph support and UI for variadic inputs.
struct ExpressionNodeContext {
	a: f64,
	b: f64,
}

impl ValueProvider for ExpressionNodeContext {
	fn get_value(&self, name: &str) -> Option<Value> {
		if name.eq_ignore_ascii_case("a") {
			Some(Value::from_f64(self.a))
		} else if name.eq_ignore_ascii_case("b") {
			Some(Value::from_f64(self.b))
		} else {
			None
		}
	}
}

/// Calculates a mathematical expression with input values "A" and "B"
#[node_macro::node(category("Math"))]
fn math<U: num_traits::float::Float>(
	_: (),
	/// The value of "A" when calculating the expression
	#[implementations(f64, f32)]
	operand_a: U,
	/// A math expression that may incorporate "A" and/or "B", such as "sqrt(A + B) - B^2"
	#[default(A + B)]
	expression: String,
	/// The value of "B" when calculating the expression
	#[implementations(f64, f32)]
	#[default(1.)]
	operand_b: U,
) -> U {
	let (node, _unit) = match ast::Node::try_parse_from_str(&expression) {
		Ok(expr) => expr,
		Err(e) => {
			warn!("Invalid expression: `{expression}`\n{e:?}");
			return U::from(0.).unwrap();
		}
	};
	let context = EvalContext::new(
		ExpressionNodeContext {
			a: operand_a.to_f64().unwrap(),
			b: operand_b.to_f64().unwrap(),
		},
		NothingMap,
	);

	let value = match node.eval(&context) {
		Ok(value) => value,
		Err(e) => {
			warn!("Expression evaluation error: {e:?}");
			return U::from(0.).unwrap();
		}
	};

	let Value::Number(num) = value;
	match num {
		Number::Real(val) => U::from(val).unwrap(),
		Number::Complex(c) => U::from(c.re).unwrap(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_basic_expression() {
		let result = math((), 0., "2 + 2".to_string(), 0.);
		assert_eq!(result, 4.);
	}

	#[test]
	fn test_complex_expression() {
		let result = math((), 0., "(5 * 3) + (10 / 2)".to_string(), 0.);
		assert_eq!(result, 20.);
	}

	#[test]
	fn test_default_expression() {
		let result = math((), 0., "0".to_string(), 0.);
		assert_eq!(result, 0.);
	}

	#[test]
	fn test_invalid_expression() {
		let result = math((), 0., "invalid".to_string(), 0.);
		assert_eq!(result, 0.);
	}
}
