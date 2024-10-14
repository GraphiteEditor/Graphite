use std::num::{ParseFloatError, ParseIntError};

use lazy_static::lazy_static;
use num_complex::ComplexFloat;
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
	ParseInt(#[from] ParseIntError),
	#[error("ParseFloatError: {0}")]
	ParseFloat(#[from] ParseFloatError),

	#[error("TypeError: {0}")]
	Type(#[from] TypeError),

	#[error("PestError: {0}")]
	Pest(#[from] Box<pest::error::Error<Rule>>),
}

impl Node {
	pub fn from_str(s: &str) -> Result<Node, ParseError> {
		let pairs = ExprParser::parse(Rule::program, s).map_err(Box::new)?;
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

fn parse_unit(pairs: Pairs<Rule>) -> Result<Unit, ParseError> {
	let mut scale = 1.0;
	let mut length = 0;
	let mut mass = 0;
	let mut time = 0;

	for pair in pairs {
		match pair.as_rule() {
			Rule::nano => scale *= 1e-9,
			Rule::micro => scale *= 1e-6,
			Rule::milli => scale *= 1e-3,
			Rule::centi => scale *= 1e-2,
			Rule::deci => scale *= 1e-1,
			Rule::deca => scale *= 1e1,
			Rule::hecto => scale *= 1e2,
			Rule::kilo => scale *= 1e3,
			Rule::mega => scale *= 1e6,
			Rule::giga => scale *= 1e9,
			Rule::tera => scale *= 1e12,

			Rule::meter => length = 1,
			Rule::gram => mass = 1,
			Rule::second => time = 1,

			_ => unreachable!(), // All possible rules should be covered
		}
	}

	Ok(Unit { scale, length, mass, time })
}

fn parse_lit(mut pairs: Pairs<Rule>) -> Result<(Literal, Option<Unit>), ParseError> {
	let literal = match pairs.next() {
		Some(lit) => match lit.as_rule() {
			Rule::int => {
				let value = lit.as_str().parse::<i32>()?;
				Literal::Int(value)
			}
			Rule::float => {
				let value = lit.as_str().parse::<f64>()?;
				Literal::Float(value)
			}
			_ => unreachable!(),
		},
		None => unreachable!(), // No literal found
	};

	let unit = if let Some(unit_pair) = pairs.next() {
		let unit_pairs = unit_pair.into_inner(); // Get the inner pairs for the unit
		Some(parse_unit(unit_pairs)?)
	} else {
		None // No unit
	};

	Ok((literal, unit))
}

fn parse_expr(pairs: Pairs<Rule>) -> Result<(Node, NodeMetadata), ParseError> {
	PRATT_PARSER
		.map_primary(|primary| {
			Ok(match primary.as_rule() {
				Rule::lit => {
					let (lit, unit) = parse_lit(primary.into_inner())?;

					(Node::Lit(lit), NodeMetadata { unit })
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
				Rule::var => {
					let name = primary.as_str().to_string();

					(Node::Var(name), NodeMetadata::new(None))
				}
				Rule::expr => parse_expr(primary.into_inner())?,
				Rule::float => {
					let value = primary.as_str().parse::<f64>()?;
					(Node::Lit(Literal::Float(value)), NodeMetadata::new(None))
				}
				rule => unreachable!("unexpected rule: {:?}", rule),
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
				rule => unreachable!("unexpected rule: {:?}", rule),
			};

			let node = Node::UnaryOp { expr: Box::new(rhs), op };

			let unit = match rhs_metadata.unit {
				Some(unit) => match op {
					UnaryOp::Sqrt if unit.length % 2 == 0 && unit.mass % 2 == 0 && unit.time % 2 == 0 => Some(Unit {
						scale: unit.scale.sqrt(),
						length: unit.length / 2,
						mass: unit.mass / 2,
						time: unit.time / 2,
					}),
					UnaryOp::Neg => Some(unit),
					op => return Err(ParseError::Type(TypeError::InvalidUnaryOp(Some(unit), op))),
				},
				None => None,
			};

			Ok((node, NodeMetadata::new(unit)))
		})
		.map_postfix(|lhs, op| {
			let (lhs_node, lhs_metadata) = lhs?;

			let op = match op.as_rule() {
				Rule::EOI => return Ok((lhs_node, lhs_metadata)),
				Rule::fac => UnaryOp::Fac,
				rule => unreachable!("unexpected rule: {:?}", rule),
			};

			if lhs_metadata.unit.is_some() {
				return Err(ParseError::Type(TypeError::InvalidUnaryOp(lhs_metadata.unit, op)));
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
				rule => unreachable!("unexpected rule: {:?}", rule),
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
							return Err(ParseError::Type(TypeError::InvalidBinaryOp(Some(lhs_unit), op, Some(rhs_unit))));
						}
					}
					BinaryOp::Pow => {
						return Err(ParseError::Type(TypeError::InvalidBinaryOp(Some(lhs_unit), op, Some(rhs_unit))));
					}
				},

				(Some(lhs_unit), None) => match op {
					BinaryOp::Add | BinaryOp::Sub => return Err(ParseError::Type(TypeError::InvalidBinaryOp(Some(lhs_unit), op, None))),
					BinaryOp::Pow => {
						//TODO: improve error type
						//TODO: support 1 / int
						if let Ok(Value::Number(Number::Real(val))) = rhs.eval() {
							if (val - val as i32 as f64).abs() <= f64::EPSILON {
								Some(Unit {
									scale: lhs_unit.scale.powf(val),
									length: lhs_unit.length * val as i32,
									mass: lhs_unit.mass * val as i32,
									time: lhs_unit.time * val as i32,
								})
							} else {
								return Err(ParseError::Type(TypeError::InvalidBinaryOp(Some(lhs_unit), op, None)));
							}
						} else {
							return Err(ParseError::Type(TypeError::InvalidBinaryOp(Some(lhs_unit), op, None)));
						}
					}
					_ => Some(lhs_unit),
				},
				(None, Some(rhs_unit)) => match op {
					BinaryOp::Add | BinaryOp::Sub | BinaryOp::Pow => return Err(ParseError::Type(TypeError::InvalidBinaryOp(None, op, Some(rhs_unit)))),
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

//TODO: set up Unit test for Units
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
