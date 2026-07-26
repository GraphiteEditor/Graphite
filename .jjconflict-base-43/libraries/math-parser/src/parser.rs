use crate::ast::{BinaryOp, Literal, Node, UnaryOp, Unit};
use crate::context::EvalContext;
use crate::lexer::{Lexer, Span, Token};
use crate::value::{Complex, Number, Value};
use chumsky::container::Seq;
use chumsky::input::{BorrowInput, ValueInput};
use chumsky::{Parser, prelude::*};
use lazy_static::lazy_static;
use num_complex::ComplexFloat;
use std::fmt;
use std::num::{ParseFloatError, ParseIntError};
use thiserror::Error;

/// One message per parse failure, each tagged with its byte range in the source expression.
#[derive(Debug)]
pub struct ParseError(Vec<String>);

impl fmt::Display for ParseError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		for (index, error) in self.0.iter().enumerate() {
			if index > 0 {
				writeln!(f)?;
			}
			write!(f, "{error}")?;
		}
		Ok(())
	}
}

impl std::error::Error for ParseError {}

impl Node {
	pub fn try_parse_from_str(src: &str) -> Result<Node, ParseError> {
		let tokens = Lexer::new(src);

		match parser().parse(tokens).into_result() {
			Ok(ast) => Ok(ast),
			Err(parse_errs) => Err(ParseError(parse_errs.into_iter().map(|e| format!("{e} at {}", e.span())).collect())),
		}
	}
}

pub fn parser<'src, I>() -> impl Parser<'src, I, Node, extra::Err<Rich<'src, Token<'src>, Span>>>
where
	I: ValueInput<'src, Token = Token<'src>, Span = Span>,
{
	recursive(|expr| {
		let constant = select! {
			Token::Float(f) => Node::Lit(Literal::Float(f)),
			Token::Const(c) => Node::Lit(c.value())
		};

		let args = expr.clone().separated_by(just(Token::Comma)).collect::<Vec<_>>().delimited_by(just(Token::LParen), just(Token::RParen));

		let if_expr = just(Token::If)
			.ignore_then(args.clone()) // Parses (cond, a, b)
			.try_map(|args: Vec<Node>, span| {
				if args.len() != 3 {
					return Err(Rich::custom(span, "Expected 3 arguments in if(cond, a, b)"));
				}
				let mut iter = args.into_iter();
				let cond = iter.next().unwrap();
				let if_b = iter.next().unwrap();
				let else_b = iter.next().unwrap();
				Ok(Node::Conditional {
					condition: Box::new(cond),
					if_block: Box::new(if_b),
					else_block: Box::new(else_b),
				})
			});

		let ident = select! {Token::Ident(s) => s}.labelled("ident");

		let call = ident.then(args).map(|(name, args): (&str, Vec<Node>)| Node::FnCall { name: name.to_string(), expr: args });

		let parens = expr.clone().delimited_by(just(Token::LParen), just(Token::RParen));
		let var = ident.map(|s| Node::Var(s.to_string()));

		let atom = choice((constant, if_expr, call, parens, var)).labelled("atom").boxed();

		let add_op = choice((just(Token::Plus).to(BinaryOp::Add), just(Token::Minus).to(BinaryOp::Sub)));
		let mul_op = choice((just(Token::Star).to(BinaryOp::Mul), just(Token::Slash).to(BinaryOp::Div), just(Token::Modulo).to(BinaryOp::Modulo)));
		let pow_op = just(Token::Caret).to(BinaryOp::Pow);
		let unary_op = choice((just(Token::Minus).to(UnaryOp::Neg), just(Token::Bang).to(UnaryOp::Not)));
		let and_op = just(Token::AndAnd).to(BinaryOp::And);
		let or_op = just(Token::OrOr).to(BinaryOp::Or);
		let cmp_op = choice((
			just(Token::Lt).to(BinaryOp::Lt),
			just(Token::Le).to(BinaryOp::Leq),
			just(Token::Gt).to(BinaryOp::Gt),
			just(Token::Ge).to(BinaryOp::Geq),
			just(Token::Neq).to(BinaryOp::Neq),
			just(Token::EqEq).to(BinaryOp::Eq),
		));

		// Postfix factorial: expr! → UnaryOp::Fac
		let postfix = atom
			.clone()
			.foldl(just(Token::Bang).repeated(), |expr, _| Node::UnaryOp {
				op: UnaryOp::Fac,
				expr: Box::new(expr),
			})
			.boxed();

		let pow = postfix.clone().foldl(
			pow_op
				.then(unary_op.clone().repeated().foldr(postfix, |op, expr| Node::UnaryOp { op, expr: Box::new(expr) }).boxed())
				.repeated(),
			|lhs, (op, rhs)| Node::BinOp {
				lhs: Box::new(lhs),
				op,
				rhs: Box::new(rhs),
			},
		);

		let unary = unary_op.repeated().foldr(pow, |op, expr| Node::UnaryOp { op, expr: Box::new(expr) }).boxed();

		let product = unary
			.clone()
			.foldl(mul_op.then(unary).repeated(), |lhs, (op, rhs)| Node::BinOp {
				lhs: Box::new(lhs),
				op,
				rhs: Box::new(rhs),
			})
			.boxed();

		let add = product.clone().foldl(add_op.then(product).repeated(), |lhs, (op, rhs)| Node::BinOp {
			lhs: Box::new(lhs),
			op,
			rhs: Box::new(rhs),
		});

		let cmp = add.clone().foldl(cmp_op.then(add).repeated(), |lhs: Node, (op, rhs)| Node::BinOp {
			lhs: Box::new(lhs),
			op,
			rhs: Box::new(rhs),
		});

		// Chain comparisons like `a < b < c` by multiplying the boolean
		// (1. / 0.) results, preserving the existing semantics.
		let chained_cmp = cmp.clone().foldl(cmp.repeated(), |lhs, rhs| Node::BinOp {
			lhs: Box::new(lhs),
			op: BinaryOp::Mul,
			rhs: Box::new(rhs),
		});

		let and = chained_cmp.clone().foldl(and_op.then(chained_cmp).repeated(), |lhs, (op, rhs)| Node::BinOp {
			lhs: Box::new(lhs),
			op,
			rhs: Box::new(rhs),
		});

		and.clone().foldl(or_op.then(and).repeated(), |lhs, (op, rhs)| Node::BinOp {
			lhs: Box::new(lhs),
			op,
			rhs: Box::new(rhs),
		})
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

					let result = match Node::try_parse_from_str($input) {
						Ok(expr) => expr,
						Err(err) => panic!("failed to parse `{}`: {err}", $input),
					};
					assert_eq!(result, $expected);
				}
			)*
		};
	}

	test_parser! {
		test_parse_int_literal: "42" => Node::Lit(Literal::Float(42.)),
		test_parse_float_literal: "3.14" => Node::Lit(Literal::Float(#[allow(clippy::approx_constant)] 3.14)),
		test_parse_ident: "x" => Node::Var("x".to_string()),
		test_parse_unary_neg: "-42" => Node::UnaryOp {
			expr: Box::new(Node::Lit(Literal::Float(42.))),
			op: UnaryOp::Neg,
		},
		test_parse_binary_add: "1 + 2" => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Float(1.))),
			op: BinaryOp::Add,
			rhs: Box::new(Node::Lit(Literal::Float(2.))),
		},
		test_parse_binary_mul: "3 * 4" => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Float(3.))),
			op: BinaryOp::Mul,
			rhs: Box::new(Node::Lit(Literal::Float(4.))),
		},
		test_parse_binary_pow: "2 ^ 3" => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Float(2.))),
			op: BinaryOp::Pow,
			rhs: Box::new(Node::Lit(Literal::Float(3.))),
		},
		test_parse_unary_sqrt: "sqrt(16)" => Node::FnCall {
			name: "sqrt".to_string(),
			expr: vec![Node::Lit(Literal::Float(16.))],
		},
		test_parse_ii_call: "ii(16)" => Node::FnCall {
			name: "ii".to_string(),
			expr: vec![Node::Lit(Literal::Float(16.))]
		},
		test_parse_i_mul: "i(16)" => Node::BinOp {
			lhs: Box::new(Node::Lit(Literal::Complex(Complex::new(0., 1.)))),
			op: BinaryOp::Mul,
			rhs: Box::new(Node::Lit(Literal::Float(16.))),
		},
		test_parse_complex_expr: "(1 + 2) * 3 - 4 ^ 2" => Node::BinOp {
			lhs: Box::new(Node::BinOp {
				lhs: Box::new(Node::BinOp {
					lhs: Box::new(Node::Lit(Literal::Float(1.))),
					op: BinaryOp::Add,
					rhs: Box::new(Node::Lit(Literal::Float(2.))),
				}),
				op: BinaryOp::Mul,
				rhs: Box::new(Node::Lit(Literal::Float(3.))),
			}),
			op: BinaryOp::Sub,
			rhs: Box::new(Node::BinOp {
				lhs: Box::new(Node::Lit(Literal::Float(4.))),
				op: BinaryOp::Pow,
				rhs: Box::new(Node::Lit(Literal::Float(2.))),
			}),
		},
		test_conditional_expr: "if (x+3, 0, 1)" => Node::Conditional{
			condition: Box::new(Node::BinOp{
				lhs: Box::new(Node::Var("x".to_string())),
				op: BinaryOp::Add,
				rhs: Box::new(Node::Lit(Literal::Float(3.))),
			}),
			if_block: Box::new(Node::Lit(Literal::Float(0.))),
			else_block: Box::new(Node::Lit(Literal::Float(1.))),
		}
	}
}
