use lazy_static::lazy_static;
use pest::pratt_parser::{Assoc, Op, PrattParser};
use pest::{iterators::Pairs, Parser};
use pest_derive::Parser;
use std::num::{ParseFloatError, ParseIntError};
use thiserror::Error;

use crate::ast::{BinaryOp, Literal, Node, UnaryOp};

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
pub enum ParseError {
	#[error("ParseIntError: {0}")]
	ParseIntError(#[from] ParseIntError),
	#[error("ParseFloatError: {0}")]
	ParseFloatError(#[from] ParseFloatError),

	#[error("PestError: {0}")]
	PestError(#[from] pest::error::Error<Rule>),
}

impl Node {
	pub fn from_str(s: &str) -> Result<Node, ParseError> {
		let pairs = ExprParser::parse(Rule::program, s)?;
		parse_expr(pairs)
	}
}

fn parse_expr(pairs: Pairs<Rule>) -> Result<Node, ParseError> {
	PRATT_PARSER
		.map_primary(|primary| {
			Ok(match primary.as_rule() {
				Rule::int => Node::Lit(Literal::Int(primary.as_str().parse::<u64>()?)),
				Rule::var => {
					let name = primary.as_str().to_string();

					Node::Var(name)
				}
				Rule::fn_call => {
					let mut pairs = primary.into_inner();
					let name = pairs.next().expect("fn_call always has 2 children").as_str().to_string();

					Node::FnCall {
						name,
						expr: Box::new(parse_expr(pairs.next().expect("fn_call always has two children").into_inner())?),
					}
				}
				Rule::global_var => {
					let name = primary.as_str().split_at(1).1.to_string();

					Node::GlobalVar(name)
				}
				Rule::expr => parse_expr(primary.into_inner())?,
				Rule::float => Node::Lit(Literal::Float(primary.as_str().parse::<f64>()?)),
				rule => unreachable!("Expr::parse expected int, expr, ident, found {:?}", rule),
			})
		})
		.map_prefix(|op, rhs| {
			Ok(Node::UnaryOp {
				expr: Box::new(rhs?),
				op: match op.as_rule() {
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
				},
			})
		})
		.map_postfix(|lhs, op| {
			Ok(match &op.as_rule() {
				Rule::EOI => lhs?,
				Rule::fac => Node::UnaryOp {
					expr: Box::new(lhs?),
					op: match op.as_rule() {
						Rule::fac => UnaryOp::Fac,

						_ => unreachable!(),
					},
				},
				_ => unreachable!(),
			})
		})
		.map_infix(|lhs, op, rhs| {
			Ok(match op.as_rule() {
				_ => Node::BinOp {
					lhs: Box::new(lhs?),
					op: match op.as_rule() {
						Rule::add => BinaryOp::Add,
						Rule::sub => BinaryOp::Sub,
						Rule::mul => BinaryOp::Mul,
						Rule::div => BinaryOp::Div,
						Rule::pow => BinaryOp::Pow,
						Rule::paren => BinaryOp::Mul,

						_ => unreachable!(),
					},
					rhs: Box::new(rhs?),
				},
			})
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
