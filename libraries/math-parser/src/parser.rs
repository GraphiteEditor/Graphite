use crate::ast::{BinaryOp, Literal, Node, UnaryOp, Unit};
use crate::context::EvalContext;
use crate::value::{Complex, Number, Value};
use chumsky::container::Seq;
use chumsky::{Parser, prelude::*};
use lazy_static::lazy_static;
use num_complex::ComplexFloat;
use std::num::{ParseFloatError, ParseIntError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError<'src> {
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
			just("i").map(|_| Node::Lit(Literal::Complex(Complex::new(0.0, 1.0)))), // Assuming `Complex` impl
			just("G").map(|_| Node::Lit(Literal::Float(9.80665))),                  // Standard gravity on Earth
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

		let atom = choice((conditional, float, constant, call, parens, var)).boxed();

		let add_op = choice((just('+').to(BinaryOp::Add), just('-').to(BinaryOp::Sub))).padded();
		let mul_op = choice((just('*').to(BinaryOp::Mul), just('/').to(BinaryOp::Div))).padded();
		let pow_op = just('^').to(BinaryOp::Pow).padded();
		let unary_op = choice((just('-').to(UnaryOp::Neg), just("sqrt").to(UnaryOp::Sqrt))).padded();
		let cmp_op = choice((
			just("<").to(BinaryOp::Lt),
			just("<=").to(BinaryOp::Leq),
			just(">").to(BinaryOp::Gt),
			just(">=").to(BinaryOp::Geq),
			just("==").to(BinaryOp::Eq),
		));

		let unary = unary_op.repeated().foldr(atom, |op, expr| Node::UnaryOp { op, expr: Box::new(expr) });

		let cmp = unary.clone().foldl(cmp_op.padded().then(unary).repeated(), |lhs: Node, (op, rhs)| Node::BinOp {
			lhs: Box::new(lhs),
			op,
			rhs: Box::new(rhs),
		});

		let pow = cmp.clone().foldl(pow_op.then(cmp).repeated(), |lhs, (op, rhs)| Node::BinOp {
			lhs: Box::new(lhs),
			op,
			rhs: Box::new(rhs),
		});

		let product = pow
			.clone()
			.foldl(mul_op.then(pow).repeated(), |lhs, (op, rhs)| Node::BinOp {
				lhs: Box::new(lhs),
				op,
				rhs: Box::new(rhs),
			})
			.boxed();

		let sum = product.clone().foldl(add_op.then(product).repeated(), |lhs, (op, rhs)| Node::BinOp {
			lhs: Box::new(lhs),
			op,
			rhs: Box::new(rhs),
		});

		sum.padded()
	})
}

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
