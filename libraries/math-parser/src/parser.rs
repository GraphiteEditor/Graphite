use crate::ast::{BinaryOp, Literal, Node, UnaryOp, Unit};
use crate::context::EvalContext;
use crate::lexer::{Span, Token, TokenStream, lexer};
use crate::value::{Complex, Number, Value};
use chumsky::container::Seq;
use chumsky::input::{BorrowInput, ValueInput};
use chumsky::{Parser, prelude::*};
use lazy_static::lazy_static;
use num_complex::ComplexFloat;
use std::num::{ParseFloatError, ParseIntError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError<'src> {
	#[error("lexical error(s): {0:#?}")]
	Lex(Vec<Rich<'src, char>>),

	#[error("syntax error(s): {0:#?}")]
	Parse(Vec<Rich<'src, Token<'src>, Span>>),
}

impl Node {
	/// Lex + parse the source and either return an AST `Node`
	/// or a typed `ParseError`.
	pub fn try_parse_from_str(src: &str) -> Result<Node, ParseError> {
		// ── stage 1: lexing ──────────────────────────────────────────────
		let (tokens_opt, lex_errs) = lexer().parse(src).into_output_errors();
		if !lex_errs.is_empty() {
			return Err(ParseError::Lex(lex_errs));
		}
		let tokens = TokenStream::new(tokens_opt.expect("lexer always returns tokens with recovery"));

		match parser().parse(tokens.map((0..src.len()).into(), |(t, s)| (t, s))).into_result() {
			Ok(ast) => Ok(ast),
			Err(errs) => Err(ParseError::Parse(errs)),
		}
	}
}

pub fn parser<'src, I>() -> impl Parser<'src, I, Node, extra::Err<Rich<'src, Token<'src>, Span>>>
where
	I: ValueInput<'src, Token = Token<'src>, Span = Span>,
{
	recursive(|expr| {
		let constant = select! {Token::Const(x) => Node::Lit(x)};

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

		let call = select! {Token::Call(s) => s}
			.then(args)
			.try_map(|(name, args): (&str, Vec<Node>), span| Ok(Node::FnCall { name: name.to_string(), expr: args }));

		let parens = expr.clone().clone().delimited_by(just(Token::LParen), just(Token::RParen));
		let var = select! { Token::Var(name) => Node::Var(name.to_string()) };

		let atom = choice((constant, if_expr, call, parens, var)).boxed();

		let add_op = choice((just(Token::Plus).to(BinaryOp::Add), just(Token::Minus).to(BinaryOp::Sub)));
		let mul_op = choice((just(Token::Star).to(BinaryOp::Mul), just(Token::Slash).to(BinaryOp::Div)));
		let pow_op = just(Token::Caret).to(BinaryOp::Pow);
		let unary_op = just(Token::Minus).to(UnaryOp::Neg);
		let cmp_op = choice((
			just(Token::Lt).to(BinaryOp::Lt),
			just(Token::Le).to(BinaryOp::Leq),
			just(Token::Gt).to(BinaryOp::Gt),
			just(Token::Ge).to(BinaryOp::Geq),
			just(Token::EqEq).to(BinaryOp::Eq),
		));

		let unary = unary_op.repeated().foldr(atom, |op, expr| Node::UnaryOp { op, expr: Box::new(expr) });

		let cmp = unary.clone().foldl(cmp_op.then(unary).repeated(), |lhs: Node, (op, rhs)| Node::BinOp {
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

		product.clone().foldl(add_op.then(product).repeated(), |lhs, (op, rhs)| Node::BinOp {
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
					let result = Node::try_parse_from_str($input).unwrap();
					assert_eq!(result, $expected);
				}
			)*
		};
	}

	test_parser! {
		test_parse_int_literal: "42" => Node::Lit(Literal::Float(42.0)),
		test_parse_float_literal: "3.14" => Node::Lit(Literal::Float(#[allow(clippy::approx_constant)] 3.14)),
		test_parse_ident: "#x" => Node::Var("x".to_string()),
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
		test_parse_unary_sqrt: "@sqrt(16)" => Node::FnCall {
			name: "sqrt".to_string(),
			expr: vec![Node::Lit(Literal::Float(16.0))],
		},
		test_parse_i_call: "@i(16)" => Node::FnCall {
			 name:"i".to_string(),
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
		test_conditional_expr: "if (#x+3, 0, 1)" => Node::Conditional{
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
