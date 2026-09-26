use crate::ast::{BinaryOp, Case, Literal, Node, Syntax, UnaryOp};
use crate::context::{FunctionProvider, NothingMap};
use crate::lexer::{Lexer, Span, Token};
use crate::sort::sorted;
use chumsky::error::{EmptyErr, LabelError};
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

/// Builds a parse error from a plain message, for a failure that no "expected ..., found ..." phrasing describes.
pub trait CustomError {
	fn custom(span: Span, message: &'static str) -> Self;
}

impl CustomError for EmptyErr {
	fn custom(_: Span, _: &'static str) -> Self {
		EmptyErr::default()
	}
}

impl<'src> CustomError for Rich<'src, Token<'src>, Span> {
	fn custom(span: Span, message: &'static str) -> Self {
		Rich::custom(span, message)
	}
}

impl Node {
	pub fn try_parse_from_str(src: &str) -> Result<Node, ParseError> {
		Self::try_parse_with_functions(src, &NothingMap)
	}

	/// Parses the source for a host that supplies functions, which shadow any builtin of the same name.
	pub fn try_parse_with_functions(src: &str, functions: &impl FunctionProvider) -> Result<Node, ParseError> {
		sorted(parse(src)?, functions).map_err(|error| ParseError(vec![error.to_string()]))
	}
}

/// Parses the source as written, before its sorts are read.
fn parse(src: &str) -> Result<Syntax, ParseError> {
	// Parse with zero-cost errors first (several times faster), then re-parse invalid input with rich errors to build the messages
	if let Ok(ast) = parser::<Lexer, extra::Default>().parse(Lexer::new(src)).into_result() {
		return Ok(ast);
	}

	match parser::<Lexer, extra::Err<Rich<Token, Span>>>().parse(Lexer::new(src)).into_result() {
		Ok(ast) => Ok(ast),
		Err(parse_errs) => Err(ParseError(
			parse_errs
				.into_iter()
				.map(|e| match e.found() {
					Some(Token::Percent) => format!("`%` is reserved for percentages, so the remainder is written `mod(a, b)`, at {}", e.span()),
					Some(Token::If) => format!("`if` joins a case's value to its condition, like `{{a if x > 0, b otherwise}}`, at {}", e.span()),
					Some(Token::Otherwise) => format!("`otherwise` ends the one case with no condition, like `{{a if x > 0, b otherwise}}`, at {}", e.span()),
					Some(Token::Where) => format!("`where` is a reserved word, so it can't be a name, at {}", e.span()),
					_ => format!("{e} at {}", e.span()),
				})
				.collect(),
		)),
	}
}

pub fn parser<'src, I, E>() -> impl Parser<'src, I, Syntax, E>
where
	I: ValueInput<'src, Token = Token<'src>, Span = Span>,
	E: extra::ParserExtra<'src, I>,
	E::Error: LabelError<'src, I, &'static str> + CustomError,
{
	recursive(|expr| {
		let constant = select! {
			Token::Integer(integer) => Syntax::Lit(Literal::Integer(integer)),
			Token::Float(float) => Syntax::Lit(Literal::Float(float)),
		};

		let args = expr.clone().separated_by(just(Token::Comma)).collect::<Vec<_>>().delimited_by(just(Token::LParen), just(Token::RParen));

		// Each case is a value then its condition, except the one `otherwise` case, which may stand anywhere since case order means nothing
		let case = expr.clone().then(choice((just(Token::If).ignore_then(expr.clone()).map(Some), just(Token::Otherwise).map(|_| None))));
		let piecewise = case
			.separated_by(just(Token::Comma))
			.at_least(1)
			.collect::<Vec<(Syntax, Option<Syntax>)>>()
			.delimited_by(just(Token::LBrace), just(Token::RBrace))
			// Emitted rather than failed, since a failure at the atom's start would be relabeled as a missing atom
			.validate(|written_cases, extra, emitter| {
				let mut cases = Vec::new();
				let mut otherwise = None;
				for (value, condition) in written_cases {
					match condition {
						Some(condition) => cases.push(Case { value, condition }),
						None if otherwise.is_none() => otherwise = Some(Box::new(value)),
						None => emitter.emit(CustomError::custom(extra.span(), "A piecewise has at most one `otherwise` case")),
					}
				}
				Syntax::Piecewise { cases, otherwise }
			});

		// A matrix literal lists whole values as rows with `;` or as columns with `,`, one separator kind per literal
		let entries = |separator: Token<'src>, by_rows: bool| {
			expr.clone()
				.separated_by(just(separator))
				.at_least(1)
				.collect::<Vec<Syntax>>()
				.delimited_by(just(Token::LBracket), just(Token::RBracket))
				.map(move |entries| Syntax::Matrix { entries, by_rows })
		};
		let matrix = choice((entries(Token::Semicolon, true), entries(Token::Comma, false))).validate(|node, extra, emitter| {
			if let Syntax::Matrix { entries, .. } = &node
				&& entries.len() > 4
			{
				emitter.emit(CustomError::custom(extra.span(), "A matrix literal has at most four entries, one per part `1`, `i`, `j`, `k`"));
			}
			node
		});

		let ident = select! {Token::Ident(s) => s}.labelled("ident");

		// An ident followed by parenthesized args is a function call, otherwise a variable
		let call_or_var = ident.then(args.or_not()).map(|(name, args): (&str, Option<Vec<Syntax>>)| match args {
			Some(args) => Syntax::FnCall { name: name.to_string(), expr: args },
			None => Syntax::Var(name.to_string()),
		});

		let parens = expr.clone().delimited_by(just(Token::LParen), just(Token::RParen));
		let magnitude = expr.clone().delimited_by(just(Token::BarOpen), just(Token::BarClose)).map(|expr| Syntax::UnaryOp {
			op: UnaryOp::Magnitude,
			expr: Box::new(expr),
		});

		let atom = choice((constant, piecewise, matrix, call_or_var, parens, magnitude)).labelled("atom");

		let add_op = choice((just(Token::Plus).to(BinaryOp::Add), just(Token::Minus).to(BinaryOp::Sub)));
		let mul_op = choice((just(Token::Star).to(BinaryOp::Mul), just(Token::Slash).to(BinaryOp::Div)));
		let pow_op = just(Token::Caret).to(BinaryOp::Pow);
		let unary_op = choice((
			just(Token::Minus).to(UnaryOp::Neg),
			just(Token::Plus).to(UnaryOp::Pos),
			just(Token::Bang).to(UnaryOp::Not),
			just(Token::Not).to(UnaryOp::Not),
		));
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

		// The postfix operators, the factorial `x!` and the transpose `A^T`
		let postfix_op = choice((just(Token::Bang).to(UnaryOp::Fac), just(Token::Transpose).to(UnaryOp::Transpose)));
		let postfix = atom.clone().foldl(postfix_op.repeated(), |expr, op| Syntax::UnaryOp { op, expr: Box::new(expr) });

		// Exponentiation is right-associative (`2^2^3` is `2^(2^3)`) and the exponent may carry unary signs like `2^-3`
		let pow = recursive(|pow| {
			let exponent = unary_op.clone().repeated().foldr(pow, |op, expr| Syntax::UnaryOp { op, expr: Box::new(expr) });
			postfix.clone().then(pow_op.ignore_then(exponent).or_not()).map(|(base, exponent)| match exponent {
				Some(exponent) => Syntax::BinOp {
					lhs: Box::new(base),
					op: BinaryOp::Pow,
					rhs: Box::new(exponent),
				},
				None => base,
			})
		});

		let unary = unary_op.clone().repeated().foldr(pow.clone(), |op, expr| Syntax::UnaryOp { op, expr: Box::new(expr) });

		// Juxtaposed factors like `2pi` or `2sqrt(4)` multiply implicitly at the same precedence as `*` and `/`.
		// The implicit operand is a `pow`, not a full unary, so `2 -3` stays a subtraction; the lexer rejects a number right after another number (`10 000` is not `10*000`).
		let implicit_mul = pow.map(|rhs| (BinaryOp::Mul, rhs));
		let product = unary.clone().foldl(choice((mul_op.then(unary), implicit_mul)).repeated(), |lhs, (op, rhs)| Syntax::BinOp {
			lhs: Box::new(lhs),
			op,
			rhs: Box::new(rhs),
		});

		let add = product.clone().foldl(add_op.then(product).repeated(), |lhs, (op, rhs)| Syntax::BinOp {
			lhs: Box::new(lhs),
			op,
			rhs: Box::new(rhs),
		});

		// A range binds looser than arithmetic, so `0..2pi` reaches `2π`, and tighter than comparison
		let range = add.clone().then(just(Token::DotDot).ignore_then(add).or_not()).map(|(from, to)| match to {
			Some(to) => Syntax::Range {
				from: Box::new(from),
				to: Box::new(to),
			},
			None => from,
		});

		// A chain like `0 <= x < 1` is one predicate over its adjacent pairs, not an implicit `(0 <= x) < 1` (which is only read that way when its parentheses are written out), and must read in one direction
		let cmp = range
			.clone()
			.then(cmp_op.then(range).repeated().collect::<Vec<_>>())
			.try_map(|(first, mut rest): (Syntax, Vec<(BinaryOp, Syntax)>), span| {
				// A lone comparison is an ordinary binary operation
				if rest.len() <= 1 {
					return Ok(match rest.pop() {
						Some((op, second)) => Syntax::BinOp {
							lhs: Box::new(first),
							op,
							rhs: Box::new(second),
						},
						None => first,
					});
				}

				let ops: Vec<BinaryOp> = rest.iter().map(|(op, _)| *op).collect();
				if !BinaryOp::chain_in_one_direction(&ops) {
					return Err(CustomError::custom(
						span,
						"A comparison chain must read in one direction: all ascending (`<`, `<=`, `==`), all descending (`>`, `>=`, `==`), or all `!=`",
					));
				}

				Ok(Syntax::Comparison { first: Box::new(first), rest })
			});

		let and = cmp.clone().foldl(and_op.then(cmp).repeated(), |lhs, (op, rhs)| Syntax::BinOp {
			lhs: Box::new(lhs),
			op,
			rhs: Box::new(rhs),
		});

		and.clone().foldl(or_op.then(and).repeated(), |lhs, (op, rhs)| Syntax::BinOp {
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

					let result = match parse($input) {
						Ok(expr) => expr,
						Err(err) => panic!("failed to parse `{}`: {err}", $input),
					};
					assert_eq!(result, $expected);
				}
			)*
		};
	}

	test_parser! {
		test_parse_int_literal: "42" => Syntax::Lit(Literal::Integer(42)),
		test_parse_float_literal: "3.14" => Syntax::Lit(Literal::Float(#[allow(clippy::approx_constant)] 3.14)),
		test_parse_ident: "x" => Syntax::Var("x".to_string()),
		test_matrix_rows: "[1;i]" => Syntax::Matrix {
			entries: vec![Syntax::Lit(Literal::Integer(1)), Syntax::Var("i".to_string())],
			by_rows: true,
		},
		test_matrix_columns: "[i,j]" => Syntax::Matrix {
			entries: vec![Syntax::Var("i".to_string()), Syntax::Var("j".to_string())],
			by_rows: false,
		},
		test_range_below_arithmetic: "-1..2pi" => Syntax::Range {
			from: Box::new(Syntax::UnaryOp {
				op: UnaryOp::Neg,
				expr: Box::new(Syntax::Lit(Literal::Integer(1))),
			}),
			to: Box::new(Syntax::BinOp {
				lhs: Box::new(Syntax::Lit(Literal::Integer(2))),
				op: BinaryOp::Mul,
				rhs: Box::new(Syntax::Var("pi".to_string())),
			}),
		},
		test_range_above_comparison: "0..1 == I" => Syntax::BinOp {
			lhs: Box::new(Syntax::Range {
				from: Box::new(Syntax::Lit(Literal::Integer(0))),
				to: Box::new(Syntax::Lit(Literal::Integer(1))),
			}),
			op: BinaryOp::Eq,
			rhs: Box::new(Syntax::Var("I".to_string())),
		},
		test_transpose_then_inverse: "A^T^-1" => Syntax::BinOp {
			lhs: Box::new(Syntax::UnaryOp {
				op: UnaryOp::Transpose,
				expr: Box::new(Syntax::Var("A".to_string())),
			}),
			op: BinaryOp::Pow,
			rhs: Box::new(Syntax::UnaryOp {
				op: UnaryOp::Neg,
				expr: Box::new(Syntax::Lit(Literal::Integer(1))),
			}),
		},
		test_parse_unary_neg: "-42" => Syntax::UnaryOp {
			expr: Box::new(Syntax::Lit(Literal::Integer(42))),
			op: UnaryOp::Neg,
		},
		test_parse_binary_add: "1 + 2" => Syntax::BinOp {
			lhs: Box::new(Syntax::Lit(Literal::Integer(1))),
			op: BinaryOp::Add,
			rhs: Box::new(Syntax::Lit(Literal::Integer(2))),
		},
		test_parse_binary_mul: "3 * 4" => Syntax::BinOp {
			lhs: Box::new(Syntax::Lit(Literal::Integer(3))),
			op: BinaryOp::Mul,
			rhs: Box::new(Syntax::Lit(Literal::Integer(4))),
		},
		test_parse_binary_pow: "2 ^ 3" => Syntax::BinOp {
			lhs: Box::new(Syntax::Lit(Literal::Integer(2))),
			op: BinaryOp::Pow,
			rhs: Box::new(Syntax::Lit(Literal::Integer(3))),
		},
		test_parse_sqrt_call: "sqrt(16)" => Syntax::FnCall {
			name: "sqrt".to_string(),
			expr: vec![Syntax::Lit(Literal::Integer(16))],
		},
		test_parse_ii_call: "ii(16)" => Syntax::FnCall {
			name: "ii".to_string(),
			expr: vec![Syntax::Lit(Literal::Integer(16))]
		},
		// `i` is a name a binding may shadow, so only the evaluator can read this call as `i` times its argument
		test_parse_i_mul: "i(16)" => Syntax::FnCall {
			name: "i".to_string(),
			expr: vec![Syntax::Lit(Literal::Integer(16))],
		},
		test_parse_complex_expr: "(1 + 2) * 3 - 4 ^ 2" => Syntax::BinOp {
			lhs: Box::new(Syntax::BinOp {
				lhs: Box::new(Syntax::BinOp {
					lhs: Box::new(Syntax::Lit(Literal::Integer(1))),
					op: BinaryOp::Add,
					rhs: Box::new(Syntax::Lit(Literal::Integer(2))),
				}),
				op: BinaryOp::Mul,
				rhs: Box::new(Syntax::Lit(Literal::Integer(3))),
			}),
			op: BinaryOp::Sub,
			rhs: Box::new(Syntax::BinOp {
				lhs: Box::new(Syntax::Lit(Literal::Integer(4))),
				op: BinaryOp::Pow,
				rhs: Box::new(Syntax::Lit(Literal::Integer(2))),
			}),
		},
		test_piecewise_expr: "{0 otherwise, x + 3 if x < 0}" => Syntax::Piecewise {
			cases: vec![Case {
				value: Syntax::BinOp {
					lhs: Box::new(Syntax::Var("x".to_string())),
					op: BinaryOp::Add,
					rhs: Box::new(Syntax::Lit(Literal::Integer(3))),
				},
				condition: Syntax::BinOp {
					lhs: Box::new(Syntax::Var("x".to_string())),
					op: BinaryOp::Lt,
					rhs: Box::new(Syntax::Lit(Literal::Integer(0))),
				},
			}],
			otherwise: Some(Box::new(Syntax::Lit(Literal::Integer(0)))),
		}
	}
}
