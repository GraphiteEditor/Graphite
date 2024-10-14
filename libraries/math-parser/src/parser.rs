use std::num::{ParseFloatError, ParseIntError};

use lazy_static::lazy_static;
use pest::{
	iterators::Pairs,
	pratt_parser::{Assoc, Op, PrattParser},
	Parser,
};
use pest_derive::Parser;
use thiserror::Error;

use crate::{
	ast::{BinaryOp, Literal, Node, UnaryOp, Unit},
	value::{Number, Value},
};

#[derive(Parser)]
#[grammar = "./grammer.pest"] // Point to the grammar file
struct ExprParser;

lazy_static! {
	static ref PRATT_PARSER: PrattParser<Rule> = {
		PrattParser::new()
			.op(Op::infix(Rule::add, Assoc::Left) | Op::infix(Rule::sub, Assoc::Left))
			.op(Op::infix(Rule::mul, Assoc::Left) | Op::infix(Rule::div, Assoc::Left) | Op::infix(Rule::paren, Assoc::Left) | Op::infix(Rule::pow, Assoc::Right))
			.op(Op::postfix(Rule::fac) | Op::postfix(Rule::EOI))
			.op(Op::prefix(Rule::neg)
				| Op::prefix(Rule::sqrt)
				| Op::prefix(Rule::sin)
				| Op::prefix(Rule::cos)
				| Op::prefix(Rule::tan)
				| Op::prefix(Rule::csc)
				| Op::prefix(Rule::sec)
				| Op::prefix(Rule::cot)
				| Op::prefix(Rule::invsin)
				| Op::prefix(Rule::invcos)
				| Op::prefix(Rule::invtan)
				| Op::prefix(Rule::invcsc)
				| Op::prefix(Rule::invsec)
				| Op::prefix(Rule::invcot))
	};
}

#[derive(Error, Debug)]
pub enum TypeError {
	#[error("Invalid BinOp: {0:?} {1:?} {2:?}")]
	InvalidBinaryOp(Option<Unit>, BinaryOp, Option<Unit>),

	#[error("Invalid UnaryOp: {0:?}")]
	InvalidUnaryOp(Option<Unit>, UnaryOp),
}

#[derive(Error, Debug)]
pub enum ParseError {
	#[error("ParseIntError: {0}")]
	ParseIntError(#[from] ParseIntError),
	#[error("ParseFloatError: {0}")]
	ParseFloatError(#[from] ParseFloatError),

	#[error("TypeError: {0}")]
	TypeError(#[from] TypeError),

	#[error("PestError: {0}")]
	PestError(#[from] pest::error::Error<Rule>),
}

impl Node {
	pub fn from_str(s: &str) -> Result<Node, ParseError> {
		let pairs = ExprParser::parse(Rule::program, s)?.next().expect("program should have atleast one child").into_inner();
		Ok(parse_expr(pairs)?.0)
	}
}

struct NodeMetadata {
	pub unit: Option<Unit>,
}

impl NodeMetadata {
	pub fn new(unit: Option<Unit>) -> Self {
		Self { unit }
	}
}

fn parse_expr(pairs: Pairs<Rule>) -> Result<(Node, NodeMetadata), ParseError> {
	PRATT_PARSER
		.map_primary(|primary| {
			Ok(match primary.as_rule() {
				Rule::int => {
					let value = primary.as_str().parse::<u64>()? as f64;
					(Node::Lit(Literal::Float(value)), NodeMetadata::new(None))
				}
				Rule::var => {
					let name = primary.as_str().to_string();

					(Node::Var(name), NodeMetadata::new(None))
				}
				Rule::fn_call => {
					let mut pairs = primary.into_inner();
					let name = pairs.next().expect("fn_call always has 2 children").as_str().to_string();

					(
						Node::FnCall {
							name,
							expr: Box::new(parse_expr(pairs.next().expect("fn_call always has two children").into_inner())?.0),
						},
						NodeMetadata::new(None),
					)
				}
				Rule::global_var => {
					let name = primary.as_str().split_at(1).1.to_string();

					(Node::GlobalVar(name), NodeMetadata::new(None))
				}
				Rule::expr => parse_expr(primary.into_inner())?,
				Rule::float => {
					let value = primary.as_str().parse::<f64>()?;
					(Node::Lit(Literal::Float(value)), NodeMetadata::new(None))
				}
				rule => unreachable!("Expr::parse expected int, expr, ident, found {:?}", rule),
			})
		})
		.map_prefix(|op, rhs| {
			let (rhs, rhs_metadata) = rhs?;
			let op = match op.as_rule() {
				Rule::neg => UnaryOp::Neg,
				Rule::sqrt => UnaryOp::Sqrt,
				Rule::sin => UnaryOp::Sin,
				Rule::cos => UnaryOp::Cos,
				Rule::tan => UnaryOp::Tan,
				Rule::csc => UnaryOp::Csc,
				Rule::sec => UnaryOp::Sec,
				Rule::cot => UnaryOp::Cot,
				Rule::invsin => UnaryOp::InvSin,
				Rule::invcos => UnaryOp::InvCos,
				Rule::invtan => UnaryOp::InvTan,
				Rule::invcsc => UnaryOp::InvCsc,
				Rule::invsec => UnaryOp::InvSec,
				Rule::invcot => UnaryOp::InvCot,
				_ => unreachable!(),
			};

			let node = Node::UnaryOp { expr: Box::new(rhs), op };

			let unit = match rhs_metadata.unit {
				Some(unit) => match op {
					UnaryOp::Sqrt => Some(Unit {
						scale: unit.scale.sqrt(),
						length: unit.length / 2.0,
						mass: unit.mass / 2.0,
						time: unit.mass / 2.0,
					}),
					UnaryOp::Neg => Some(unit),
					op => return Err(ParseError::TypeError(TypeError::InvalidUnaryOp(Some(unit), op))),
				},
				None => None,
			};

			Ok((node, NodeMetadata::new(unit)))
		})
		.map_postfix(|lhs, op| {
			let (lhs_node, lhs_metadata) = lhs?;

			let op = match op.as_rule() {
				Rule::fac => UnaryOp::Fac,
				_ => unreachable!(),
			};

			if lhs_metadata.unit.is_some() {
				return Err(ParseError::TypeError(TypeError::InvalidUnaryOp(lhs_metadata.unit, op)));
			}

			Ok((Node::UnaryOp { expr: Box::new(lhs_node), op }, lhs_metadata))
		})
		.map_infix(|lhs, op, rhs| {
			let (lhs, lhs_metadata) = lhs?;
			let (rhs, rhs_metadata) = rhs?;

			let op = match op.as_rule() {
				Rule::add => BinaryOp::Add,
				Rule::sub => BinaryOp::Sub,
				Rule::mul => BinaryOp::Mul,
				Rule::div => BinaryOp::Div,
				Rule::pow => BinaryOp::Pow,
				Rule::paren => BinaryOp::Mul,
				_ => unreachable!(),
			};

			let unit = match (lhs_metadata.unit, rhs_metadata.unit) {
				(Some(lhs_unit), Some(rhs_unit)) => match op {
					BinaryOp::Mul => Some(Unit {
						scale: lhs_unit.scale * rhs_unit.scale,
						length: lhs_unit.length + rhs_unit.length,
						mass: lhs_unit.mass + rhs_unit.mass,
						time: lhs_unit.time + rhs_unit.time,
					}),
					BinaryOp::Div => Some(Unit {
						scale: lhs_unit.scale / rhs_unit.scale,
						length: lhs_unit.length - rhs_unit.length,
						mass: lhs_unit.mass - rhs_unit.mass,
						time: lhs_unit.time - rhs_unit.time,
					}),
					BinaryOp::Add | BinaryOp::Sub => {
						if lhs_unit == rhs_unit {
							Some(lhs_unit)
						} else {
							return Err(ParseError::TypeError(TypeError::InvalidBinaryOp(Some(lhs_unit), op, Some(rhs_unit))));
						}
					}
					BinaryOp::Pow => {
						return Err(ParseError::TypeError(TypeError::InvalidBinaryOp(Some(lhs_unit), op, Some(rhs_unit))));
					}
				},

				(Some(lhs_unit), None) => match op {
					BinaryOp::Add | BinaryOp::Sub => return Err(ParseError::TypeError(TypeError::InvalidBinaryOp(Some(lhs_unit), op, None))),
					BinaryOp::Pow => {
						if let Ok(Value::Number(Number::Real(val))) = rhs.eval() {
							Some(Unit {
								scale: lhs_unit.scale.powf(val),
								length: lhs_unit.length * val as f32,
								mass: lhs_unit.mass * val as f32,
								time: lhs_unit.time * val as f32,
							})
						} else {
							return Err(ParseError::TypeError(TypeError::InvalidBinaryOp(Some(lhs_unit), op, None)));
						}
					}
					_ => return Err(ParseError::TypeError(TypeError::InvalidBinaryOp(Some(lhs_unit), op, None))),
				},
				(None, Some(rhs_unit)) => match op {
					BinaryOp::Add | BinaryOp::Sub | BinaryOp::Pow => return Err(ParseError::TypeError(TypeError::InvalidBinaryOp(None, op, Some(rhs_unit)))),
					_ => Some(rhs_unit),
				},
				(None, None) => None,
			};

			let node = Node::BinOp {
				lhs: Box::new(lhs),
				op,
				rhs: Box::new(rhs),
			};

			Ok((node, NodeMetadata::new(unit)))
		})
		.parse(pairs)
}

#[cfg(test)]
mod tests {
	use super::*;
	macro_rules! test_parser {
	($($name:ident: $input:expr => $expected:expr),* $(,)?) => {
		$(
			#[test]
			fn $name() {
				let result = Node::from_str($input).unwrap();
				assert_eq!(result, $expected);
			}
		)*
	};
	}

	test_parser! {
		test_parse_int_literal: "42" => Node::Lit(Literal::Int(42)),
		test_parse_float_literal: "3.14" => Node::Lit(Literal::Float(3.14)),
		test_parse_ident: "x" => Node::Var("x".to_string()),
		test_parse_unary_neg: "-42" => Node::UnaryOp {
			expr: Box::new(Node::Lit(Literal::Int(42))),
			op: UnaryOp::Neg,
		},
		test_parse_binary_add: "1 + 2" => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Int(1))),
			op: BinaryOp::Add,
			rhs: Box::new(Node::Lit(Literal::Int(2))),
		},
		test_parse_binary_mul: "3 * 4" => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Int(3))),
			op: BinaryOp::Mul,
			rhs: Box::new(Node::Lit(Literal::Int(4))),
		},
		test_parse_binary_pow: "2 ^ 3" => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Int(2))),
			op: BinaryOp::Pow,
			rhs: Box::new(Node::Lit(Literal::Int(3))),
		},
		test_parse_unary_sqrt: "sqrt(16)" => Node::UnaryOp {
			expr: Box::new(Node::Lit(Literal::Int(16))),
			op: UnaryOp::Sqrt,
		},
		test_parse_sqr_ident: "sqr(16)" => Node::FnCall {
			 name:"sqr".to_string(),
			 expr: Box::new(Node::Lit(Literal::Int(16)))
		},
		test_parse_global_var: "$variable_one1 - 11" => Node::BinOp {

			 lhs: Box::new(Node::GlobalVar("variable_one1".to_string())),
			 op:  BinaryOp::Sub,
			 rhs: Box::new(Node::Lit(Literal::Int(11)) )
		},
		test_parse_complex_expr: "(1 + 2)  3 - 4 ^ 2" => Node::BinOp {
			lhs: Box::new(Node::BinOp {
				lhs: Box::new(Node::BinOp {
					lhs: Box::new(Node::Lit(Literal::Int(1))),
					op: BinaryOp::Add,
					rhs: Box::new(Node::Lit(Literal::Int(2))),
				}),
				op: BinaryOp::Mul,
				rhs: Box::new(Node::Lit(Literal::Int(3))),
			}),
			op: BinaryOp::Sub,
			rhs: Box::new(Node::BinOp {
				lhs: Box::new(Node::Lit(Literal::Int(4))),
				op: BinaryOp::Pow,
				rhs: Box::new(Node::Lit(Literal::Int(2))),
			}),
		}
	}
}
