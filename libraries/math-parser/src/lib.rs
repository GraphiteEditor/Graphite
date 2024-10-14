#[macro_use]
extern crate log;

mod ast;
mod parser;
mod value;
use ast::EvalError;
use parser::ParseError;
use value::Value;

pub fn evaluate(expression: &str) -> Result<Result<Value, EvalError>, ParseError> {
	debug!("Evaluating expression {expression}");
	ast::Node::from_str(expression).map(|node| node.eval())
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
}
