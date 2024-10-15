#[macro_use]
extern crate log;

mod ast;
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
	let context = EvalContext::default();
	expr.map(|node| node.eval(&context))
}

#[cfg(test)]
mod tests {
	use super::*;
	#[track_caller]
	fn end_to_end(expression: &str, value: impl Into<Value>) {
		assert_eq!(evaluate(expression).unwrap().unwrap(), value.into());
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
}
