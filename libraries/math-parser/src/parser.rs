use crate::ast::{BinaryOp, Literal, Node, UnaryOp};
use crate::lexer::{Lexer, Span, Token};
use chumsky::error::LabelError;
use chumsky::input::ValueInput;
use chumsky::{Parser, prelude::*};
use std::fmt;

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
		// Parse with zero-cost errors first (several times faster), then re-parse invalid input with rich errors to build the messages
		if let Ok(ast) = parser::<Lexer, extra::Default>().parse(Lexer::new(src)).into_result() {
			return Ok(ast);
		}

		match parser::<Lexer, extra::Err<Rich<Token, Span>>>().parse(Lexer::new(src)).into_result() {
			Ok(ast) => Ok(ast),
			Err(parse_errs) => Err(ParseError(parse_errs.into_iter().map(|e| format!("{e} at {}", e.span())).collect())),
		}
	}
}

pub fn parser<'src, I, E>() -> impl Parser<'src, I, Node, E>
where
	I: ValueInput<'src, Token = Token<'src>, Span = Span>,
	E: extra::ParserExtra<'src, I>,
	E::Error: LabelError<'src, I, &'static str>,
{
	recursive(|expr| {
		let constant = select! {
			Token::Float(f) => Node::Lit(Literal::Float(f)),
			Token::Const(c) => Node::Lit(c.value())
		};

		let args = expr.clone().separated_by(just(Token::Comma)).collect::<Vec<_>>().delimited_by(just(Token::LParen), just(Token::RParen));

		let if_expr = just(Token::If).ignore_then(args.clone()).try_map(|args: Vec<Node>, span| {
			let [condition, if_block, else_block] = <[Node; 3]>::try_from(args).map_err(|_| LabelError::<I, _>::expected_found(["3 arguments in if(condition, a, b)"], None, span))?;

			Ok(Node::Conditional {
				condition: Box::new(condition),
				if_block: Box::new(if_block),
				else_block: Box::new(else_block),
			})
		});

		let ident = select! {Token::Ident(s) => s}.labelled("ident");

		// An ident followed by parenthesized args is a function call, otherwise a variable
		let call_or_var = ident.then(args.or_not()).map(|(name, args): (&str, Option<Vec<Node>>)| match args {
			Some(args) => Node::FnCall { name: name.to_string(), expr: args },
			None => Node::Var(name.to_string()),
		});

		let parens = expr.clone().delimited_by(just(Token::LParen), just(Token::RParen));

		let atom = choice((constant, if_expr, call_or_var, parens)).labelled("atom");

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
		let postfix = atom.clone().foldl(just(Token::Bang).repeated(), |expr, _| Node::UnaryOp {
			op: UnaryOp::Fac,
			expr: Box::new(expr),
		});

		// Exponentiation is right-associative (`2^2^3` is `2^(2^3)`); the exponent may carry unary signs like `2^-3`.
		let pow = recursive(|pow| {
			let exponent = unary_op.clone().repeated().foldr(pow, |op, expr| Node::UnaryOp { op, expr: Box::new(expr) });
			postfix.clone().then(pow_op.ignore_then(exponent).or_not()).map(|(base, exponent)| match exponent {
				Some(exponent) => Node::BinOp {
					lhs: Box::new(base),
					op: BinaryOp::Pow,
					rhs: Box::new(exponent),
				},
				None => base,
			})
		});

		let unary = unary_op.repeated().foldr(pow.clone(), |op, expr| Node::UnaryOp { op, expr: Box::new(expr) });

		// Juxtaposed factors like `2pi` or `2sqrt(4)` multiply implicitly at the same precedence as `*` and `/`.
		// The implicit right operand is a `pow`, not a full unary, so a following `-` stays a subtraction (`2 -3` means `2 - 3`).
		let implicit_mul = pow.map(|rhs| (BinaryOp::Mul, rhs));
		let product = unary.clone().foldl(choice((mul_op.then(unary), implicit_mul)).repeated(), |lhs, (op, rhs)| Node::BinOp {
			lhs: Box::new(lhs),
			op,
			rhs: Box::new(rhs),
		});

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

		let and = cmp.clone().foldl(and_op.then(cmp).repeated(), |lhs, (op, rhs)| Node::BinOp {
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
	use crate::value::Complex;

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
		test_parse_unary_sqrt: "sqrt(16)" => Node::FnCall {
			name: "sqrt".to_string(),
			expr: vec![Node::Lit(Literal::Float(16.0))],
		},
		test_parse_ii_call: "ii(16)" => Node::FnCall {
			 name:"ii".to_string(),
			 expr: vec![Node::Lit(Literal::Float(16.0))]
		},
		test_parse_i_mul: "i(16)" => Node::BinOp {
		lhs: Box::new(Node::Lit(Literal::Complex(Complex::new(0.0, 1.0)))),
		op: BinaryOp::Mul,
		rhs: Box::new(Node::Lit(Literal::Float(16.0))),
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
		test_conditional_expr: "if (x+3, 0, 1)" => Node::Conditional{
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
