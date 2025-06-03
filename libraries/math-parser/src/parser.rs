use crate::ast::{BinaryOp, Literal, Node, UnaryOp, Unit};
use crate::context::EvalContext;
use crate::value::{Complex, Number, Value};
use chumsky::container::Seq;
use chumsky::{Parser, prelude::*};
use lazy_static::lazy_static;
use num_complex::ComplexFloat;
use std::num::{ParseFloatError, ParseIntError};
use thiserror::Error;

/*#[derive(Parser)]
#[grammar = "./grammer.pest"]
struct ExprParser;

lazy_static! {
	static ref PRATT_PARSER: PrattParser<Rule> = {
		PrattParser::new()
			.op(Op::infix(Rule::add, Assoc::Left) | Op::infix(Rule::sub, Assoc::Left))
			.op(Op::infix(Rule::mul, Assoc::Left) | Op::infix(Rule::div, Assoc::Left) | Op::infix(Rule::paren, Assoc::Left))
			.op(Op::infix(Rule::pow, Assoc::Right))
			.op(Op::infix(Rule::geq, Assoc::Left) | Op::infix(Rule::leq, Assoc::Left) | Op::infix(Rule::eq, Assoc::Left))
			.op(Op::postfix(Rule::EOI))
			.op(Op::prefix(Rule::sqrt))
			.op(Op::prefix(Rule::neg))
	};
}*/

#[derive(Error, Debug)]
pub enum TypeError {
	#[error("Invalid BinOp: {0:?} {1:?} {2:?}")]
	InvalidBinaryOp(Unit, BinaryOp, Unit),

	#[error("Invalid UnaryOp: {0:?}")]
	InvalidUnaryOp(Unit, UnaryOp),
}

#[derive(Error, Debug)]
pub enum ParseError<'src> {
	/// One or more syntax/lexing errors produced by Chumsky.
	#[error("Syntax error(s): {0:#?}")]
	Syntax(Vec<Rich<'src, char>>),
}

impl Node {
	pub fn try_parse_from_str(s: &str) -> Result<Node, ParseError> {
		let parsed = chumsky_parser().parse(s);
		if parsed.has_output() {
			Ok(parsed.into_output().unwrap())
		} else {
			Err(ParseError::Syntax(parsed.into_errors()))
		}
	}
}

pub fn chumsky_parser<'a>() -> impl Parser<'a, &'a str, Node, chumsky::extra::Err<chumsky::error::Rich<'a, char>>> {
	recursive(|expr| {
		let float = text::int(10)
			.then(just('.').map(|c: char| c).then(text::int(10)).or_not())
			.then(just('e').or(just('E')).then(one_of("+-").or_not()).then(text::int(10)).or_not())
			.map(|((int_part, opt_frac), opt_exp): ((&str, _), _)| {
				let mut s: String = int_part.to_string();
				if let Some((dot, frac)) = opt_frac {
					s.push(dot);
					s.push_str(frac);
				}
				if let Some(((e, sign), exp)) = opt_exp {
					s.push(e);
					if let Some(sign) = sign {
						s.push(sign);
					}
					s.push_str(exp);
				}
				Node::Lit(Literal::Float(s.parse().unwrap()))
			});

		let constant = choice((
			just("pi").or(just("π")).map(|_| Node::Lit(Literal::Float(std::f64::consts::PI))),
			just("tau").or(just("τ")).map(|_| Node::Lit(Literal::Float(std::f64::consts::TAU))),
			just("e").map(|_| Node::Lit(Literal::Float(std::f64::consts::E))),
			just("phi").or(just("φ")).map(|_| Node::Lit(Literal::Float(1.618_033_988_75))),
			just("inf").or(just("∞")).map(|_| Node::Lit(Literal::Float(f64::INFINITY))),
			just("i").map(|_| Node::Lit(Literal::Complex(Complex::new(0.0, 1.0)))), // Assuming Complex impl
			just("G").map(|_| Node::Lit(Literal::Float(9.80665))),                  // gravity
		));

		let ident = text::ident().padded();

		let var = ident.map(|s: &str| Node::Var(s.to_string()));

		let args = expr.clone().separated_by(just(',')).collect::<Vec<_>>().delimited_by(just('('), just(')'));

		let call = ident.then(args).map(|(name, args): (&str, Vec<Node>)| Node::FnCall { name: name.to_string(), expr: args });

		let parens = expr.clone().clone().delimited_by(just('('), just(')'));

		let conditional = just("if")
			.padded()
			.ignore_then(expr.clone().delimited_by(just('('), just(')')))
			.padded()
			.then(expr.clone().delimited_by(just('{'), just('}')))
			.padded()
			.then_ignore(just("else"))
			.padded()
			.then(expr.clone().delimited_by(just('{'), just('}')))
			.padded()
			.map(|((cond, if_b), else_b): ((Node, _), _)| Node::Conditional {
				condition: Box::new(cond),
				if_block: Box::new(if_b),
				else_block: Box::new(else_b),
			});

		let atom = choice((conditional, float, constant, call, parens, var));

		let unary = choice((just('-').to(UnaryOp::Neg), just("sqrt").to(UnaryOp::Sqrt)))
			.padded()
			.repeated()
			.foldr(atom, |op, expr| Node::UnaryOp { op, expr: Box::new(expr) });

		let pow = unary.clone().foldl(just('^').to(BinaryOp::Pow).padded().then(unary).repeated(), |lhs, (op, rhs)| Node::BinOp {
			lhs: Box::new(lhs),
			op,
			rhs: Box::new(rhs),
		});

		let product = pow
			.clone()
			.foldl(choice((just('*').to(BinaryOp::Mul), just('/').to(BinaryOp::Div))).padded().then(pow).repeated(), |lhs, (op, rhs)| {
				Node::BinOp {
					lhs: Box::new(lhs),
					op,
					rhs: Box::new(rhs),
				}
			});

		let sum = product.clone().foldl(
			choice((just('+').to(BinaryOp::Add), just('-').to(BinaryOp::Sub))).padded().then(product).repeated(),
			|lhs, (op, rhs)| Node::BinOp {
				lhs: Box::new(lhs),
				op,
				rhs: Box::new(rhs),
			},
		);

		let cmp = sum.clone().foldl(
			choice((just("<").to(BinaryOp::Lt), just(">").to(BinaryOp::Gt), just("==").to(BinaryOp::Eq)))
				.padded()
				.then(sum)
				.repeated(),
			|lhs: Node, (op, rhs)| Node::BinOp {
				lhs: Box::new(lhs),
				op,
				rhs: Box::new(rhs),
			},
		);

		cmp.padded()
	})
}

struct NodeMetadata {
	pub unit: Unit,
}

impl NodeMetadata {
	pub fn new(unit: Unit) -> Self {
		Self { unit }
	}
}

/*fn parse_unit(pairs: Pairs<Rule>) -> Result<(Unit, f64), ParseError> {
	let mut scale = 1.0;
	let mut length = 0;
	let mut mass = 0;
	let mut time = 0;

	for pair in pairs {
		println!("found rule: {:?}", pair.as_rule());
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

	Ok((Unit { length, mass, time }, scale))
}

fn parse_const(pair: Pair<Rule>) -> Literal {
	match pair.as_rule() {
		Rule::infinity => Literal::Float(f64::INFINITY),
		Rule::imaginary_unit => Literal::Complex(Complex::new(0.0, 1.0)),
		Rule::pi => Literal::Float(std::f64::consts::PI),
		Rule::tau => Literal::Float(2.0 * std::f64::consts::PI),
		Rule::euler_number => Literal::Float(std::f64::consts::E),
		Rule::golden_ratio => Literal::Float(1.61803398875),
		_ => unreachable!("Unexpected constant: {:?}", pair),
	}
}

fn parse_lit(mut pairs: Pairs<Rule>) -> Result<(Literal, Unit), ParseError> {
	let literal = match pairs.next() {
		Some(lit) => match lit.as_rule() {
			Rule::int => {
				let value = lit.as_str().parse::<i32>()? as f64;
				Literal::Float(value)
			}
			Rule::float => {
				let value = lit.as_str().parse::<f64>()?;
				Literal::Float(value)
			}
			Rule::unit => {
				let (unit, scale) = parse_unit(lit.into_inner())?;
				return Ok((Literal::Float(scale), unit));
			}
			rule => unreachable!("unexpected rule: {:?}", rule),
		},
		None => unreachable!("expected rule"), // No literal found
	};

	if let Some(unit_pair) = pairs.next() {
		let unit_pairs = unit_pair.into_inner(); // Get the inner pairs for the unit
		let (unit, scale) = parse_unit(unit_pairs)?;

		println!("found unit: {:?}", unit);

		Ok((
			match literal {
				Literal::Float(num) => Literal::Float(num * scale),
				Literal::Complex(num) => Literal::Complex(num * scale),
			},
			unit,
		))
	} else {
		Ok((literal, Unit::BASE_UNIT))
	}
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
							expr: pairs.map(|p| parse_expr(p.into_inner()).map(|expr| expr.0)).collect::<Result<Vec<Node>, ParseError>>()?,
						},
						NodeMetadata::new(Unit::BASE_UNIT),
					)
				}
				Rule::constant => {
					let lit = parse_const(primary.into_inner().next().expect("constant should have atleast 1 child"));

					(Node::Lit(lit), NodeMetadata::new(Unit::BASE_UNIT))
				}
				Rule::ident => {
					let name = primary.as_str().to_string();

					(Node::Var(name), NodeMetadata::new(Unit::BASE_UNIT))
				}
				Rule::expr => parse_expr(primary.into_inner())?,
				Rule::float => {
					let value = primary.as_str().parse::<f64>()?;
					(Node::Lit(Literal::Float(value)), NodeMetadata::new(Unit::BASE_UNIT))
				}
				Rule::conditional => {
					let mut inner = primary.into_inner();

					let condition = parse_expr(inner.next().expect("expected conditional to have condition").into_inner())?;
					let if_block = parse_expr(inner.next().expect("expected conditional to have if block").into_inner())?;
					let else_block = parse_expr(inner.next().expect("expected conditional to have else block").into_inner())?;

					(
						Node::Conditional {
							condition: Box::new(condition.0),
							if_block: Box::new(if_block.0),
							else_block: Box::new(else_block.0),
						},
						if_block.1,
					)
				}
				rule => unreachable!("unexpected rule: {:?}", rule),
			})
		})
		.map_prefix(|op, rhs| {
			let (rhs, rhs_metadata) = rhs?;
			let op = match op.as_rule() {
				Rule::neg => UnaryOp::Neg,
				Rule::sqrt => UnaryOp::Sqrt,

				rule => unreachable!("unexpected rule: {:?}", rule),
			};

			let node = Node::UnaryOp { expr: Box::new(rhs), op };
			let unit = rhs_metadata.unit;

			let unit = if !unit.is_base() {
				match op {
					UnaryOp::Sqrt if unit.length % 2 == 0 && unit.mass % 2 == 0 && unit.time % 2 == 0 => Unit {
						length: unit.length / 2,
						mass: unit.mass / 2,
						time: unit.time / 2,
					},
					UnaryOp::Neg => unit,
					op => return Err(ParseError::Type(TypeError::InvalidUnaryOp(unit, op))),
				}
			} else {
				Unit::BASE_UNIT
			};

			Ok((node, NodeMetadata::new(unit)))
		})
		.map_postfix(|lhs, op| {
			let op = match op.as_rule() {
				Rule::EOI => return lhs,
				rule => unreachable!("unexpected rule: {:?}", rule),
			};
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
				Rule::leq => BinaryOp::Leq,
				Rule::geq => BinaryOp::Geq,
				Rule::eq => BinaryOp::Eq,
				rule => unreachable!("unexpected rule: {:?}", rule),
			};

			let (lhs_unit, rhs_unit) = (lhs_metadata.unit, rhs_metadata.unit);

			let unit = match (!lhs_unit.is_base(), !rhs_unit.is_base()) {
				(true, true) => match op {
					BinaryOp::Mul => Unit {
						length: lhs_unit.length + rhs_unit.length,
						mass: lhs_unit.mass + rhs_unit.mass,
						time: lhs_unit.time + rhs_unit.time,
					},
					BinaryOp::Div => Unit {
						length: lhs_unit.length - rhs_unit.length,
						mass: lhs_unit.mass - rhs_unit.mass,
						time: lhs_unit.time - rhs_unit.time,
					},
					BinaryOp::Add | BinaryOp::Sub => {
						if lhs_unit == rhs_unit {
							lhs_unit
						} else {
							return Err(ParseError::Type(TypeError::InvalidBinaryOp(lhs_unit, op, rhs_unit)));
						}
					}
					BinaryOp::Pow => {
						return Err(ParseError::Type(TypeError::InvalidBinaryOp(lhs_unit, op, rhs_unit)));
					}
					BinaryOp::Eq | BinaryOp::Leq | BinaryOp::Geq => {
						if lhs_unit == rhs_unit {
							Unit::base_unit()
						} else {
							return Err(ParseError::Type(TypeError::InvalidBinaryOp(lhs_unit, op, rhs_unit)));
						}
					}
				},

				(true, false) => match op {
					BinaryOp::Add | BinaryOp::Sub => return Err(ParseError::Type(TypeError::InvalidBinaryOp(lhs_unit, op, Unit::BASE_UNIT))),
					BinaryOp::Pow => {
						//TODO: improve error type
						//TODO: support fractions
						if let Ok(Value::Number(Number::Real(val))) = rhs.eval(&EvalContext::default()) {
							if (val - val as i32 as f64).abs() <= f64::EPSILON {
								Unit {
									length: lhs_unit.length * val.round() as i32,
									mass: lhs_unit.mass * val.round() as i32,
									time: lhs_unit.time * val as i32,
								}
							} else {
								return Err(ParseError::Type(TypeError::InvalidBinaryOp(lhs_unit, op, Unit::BASE_UNIT)));
							}
						} else {
							return Err(ParseError::Type(TypeError::InvalidBinaryOp(lhs_unit, op, Unit::BASE_UNIT)));
						}
					}
					_ => lhs_unit,
				},
				(false, true) => match op {
					BinaryOp::Add | BinaryOp::Sub | BinaryOp::Pow => return Err(ParseError::Type(TypeError::InvalidBinaryOp(Unit::BASE_UNIT, op, rhs_unit))),
					_ => rhs_unit,
				},
				(false, false) => Unit::BASE_UNIT,
			};

			let node = Node::BinOp {
				lhs: Box::new(lhs),
				op,
				rhs: Box::new(rhs),
			};

			Ok((node, NodeMetadata::new(unit)))
		})
		.parse(pairs)
}*/

//TODO: set up Unit test for Units
#[cfg(test)]
mod tests {
	use super::*;
	macro_rules! test_parser {
		($($name:ident: $input:expr_2021 => $expected:expr_2021),* $(,)?) => {
			$(
				#[test]
				fn $name() {
					let result = Node::try_parse_from_str($input).unwrap();
					assert_eq!(result, $expected);
				}
			)*
		};
	}

	test_parser! {
		test_parse_int_literal: "42" => Node::Lit(Literal::Float(42.0)),
		test_parse_float_literal: "3.14" => Node::Lit(Literal::Float(#[allow(clippy::approx_constant)] 3.14)),
		test_parse_ident: "x" => Node::Var("x".to_string()),
		test_parse_unary_neg: "-42" => Node::UnaryOp {
			expr: Box::new(Node::Lit(Literal::Float(42.0))),
			op: UnaryOp::Neg,
		},
		test_parse_binary_add: "1 + 2" => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Float(1.0))),
			op: BinaryOp::Add,
			rhs: Box::new(Node::Lit(Literal::Float(2.0))),
		},
		test_parse_binary_mul: "3 * 4" => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Float(3.0))),
			op: BinaryOp::Mul,
			rhs: Box::new(Node::Lit(Literal::Float(4.0))),
		},
		test_parse_binary_pow: "2 ^ 3" => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Float(2.0))),
			op: BinaryOp::Pow,
			rhs: Box::new(Node::Lit(Literal::Float(3.0))),
		},
		test_parse_unary_sqrt: "sqrt(16)" => Node::UnaryOp {
			expr: Box::new(Node::Lit(Literal::Float(16.0))),
			op: UnaryOp::Sqrt,
		},
		test_parse_sqr_ident: "sqr(16)" => Node::FnCall {
			 name:"sqr".to_string(),
			 expr: vec![Node::Lit(Literal::Float(16.0))]
		},

		test_parse_complex_expr: "(1 + 2) * 3 - 4 ^ 2" => Node::BinOp {
			lhs: Box::new(Node::BinOp {
				lhs: Box::new(Node::BinOp {
					lhs: Box::new(Node::Lit(Literal::Float(1.0))),
					op: BinaryOp::Add,
					rhs: Box::new(Node::Lit(Literal::Float(2.0))),
				}),
				op: BinaryOp::Mul,
				rhs: Box::new(Node::Lit(Literal::Float(3.0))),
			}),
			op: BinaryOp::Sub,
			rhs: Box::new(Node::BinOp {
				lhs: Box::new(Node::Lit(Literal::Float(4.0))),
				op: BinaryOp::Pow,
				rhs: Box::new(Node::Lit(Literal::Float(2.0))),
			}),
		},
		test_conditional_expr: "if (x+3) {0} else {1}" => Node::Conditional{
			condition: Box::new(Node::BinOp{
				lhs: Box::new(Node::Var("x".to_string())),
				op: BinaryOp::Add,
				rhs: Box::new(Node::Lit(Literal::Float(3.0))),
			}),
			if_block: Box::new(Node::Lit(Literal::Float(0.0))),
			else_block: Box::new(Node::Lit(Literal::Float(1.0))),
		}
	}
}
