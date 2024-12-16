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

/// A node that evaluates mathematical expressions during graph runtime.
#[node_macro::node(category("Math"))]
fn expression_node(_: (), _primary: (), #[default(1 + 1)] expression: String, #[expose] a: f64, #[expose] b: f64) -> f64 {
	let (node, _unit) = match ast::Node::try_parse_from_str(&expression) {
		Ok(expr) => expr,
		Err(e) => {
			warn!("Invalid expression: `{expression}`\n{e:?}");
			return 0.;
		}
	};
	let context = EvalContext::new(ExpressionNodeContext { a, b }, NothingMap);

	let value = match node.eval(&context) {
		Ok(value) => value,
		Err(e) => {
			warn!("Expression evaluation error: {e:?}");
			return 0.;
		}
	};

	let Value::Number(num) = value;
	match num {
		Number::Real(val) => val,
		Number::Complex(c) => c.re,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_basic_expression() {
		let result = expression_node((), (), "2 + 2".to_string());
		assert_eq!(result, 4.0);
	}

	#[test]
	fn test_complex_expression() {
		let result = expression_node((), (), "(5 * 3) + (10 / 2)".to_string());
		assert_eq!(result, 20.0);
	}

	#[test]
	fn test_default_expression() {
		let result = expression_node((), (), "0".to_string());
		assert_eq!(result, 0.0);
	}

	#[test]
	fn test_invalid_expression() {
		let result = expression_node((), (), "invalid".to_string());
		assert_eq!(result, 0.0);
	}
}
