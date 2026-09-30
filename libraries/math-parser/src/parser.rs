use crate::ast::{BinaryOp, Binding, CallWhere, Case, Literal, Node, Syntax, UnaryOp};
use crate::context::{FunctionProvider, NothingMap};
use crate::lexer::{LexError, Lexer, Span, Token};
use crate::sort::sorted;
use chumsky::cache::{Cache, Cached};
use chumsky::error::{EmptyErr, LabelError, RichPattern, RichReason};
use chumsky::input::ValueInput;
use chumsky::{Parser, prelude::*};
use std::fmt;
use std::ops::Range;

/// One message per parse failure, each tagged with its byte range in the source expression.
#[derive(Debug)]
pub struct ParseError(Vec<ErrorMessage>);

impl ParseError {
	pub fn messages(&self) -> &[ErrorMessage] {
		&self.0
	}
}

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

/// A parse failure's message as prose and the code it quotes, so a host can render the code, which may be the user's own source,
/// safely apart, with the byte range of the source it points at, which a sort error lacks.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorMessage {
	parts: Vec<MessagePart>,
	span: Option<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessagePart {
	Text(String),
	/// Code the message quotes, like a keyword, an example, or the offending source text.
	Code(String),
}

impl ErrorMessage {
	pub fn parts(&self) -> &[MessagePart] {
		&self.parts
	}

	pub fn span(&self) -> Option<Range<usize>> {
		self.span.clone()
	}

	/// A message written with its code between backticks, which only the language's own wording uses, never the source text.
	fn from_prose(prose: &str) -> Self {
		Self::with_code_between(prose, '`')
	}

	/// A message whose code sits between a quote character that no code within can hold.
	fn with_code_between(message: &str, quote: char) -> Self {
		let parts = message.split(quote).enumerate().filter(|(_, part)| !part.is_empty());
		let parts = parts
			.map(|(index, part)| {
				if index % 2 == 0 {
					MessagePart::Text(part.to_string())
				} else {
					MessagePart::Code(part.to_string())
				}
			})
			.collect();
		Self { parts, span: None }
	}

	fn at(self, span: &Span) -> Self {
		Self {
			span: Some(span.start..span.end),
			..self
		}
	}
}

// Code goes between backticks and the span after the prose, the plain-text convention
impl fmt::Display for ErrorMessage {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		for part in &self.parts {
			match part {
				MessagePart::Text(text) => f.write_str(text)?,
				MessagePart::Code(code) => write!(f, "`{code}`")?,
			}
		}
		if let Some(span) = &self.span {
			write!(f, ", at {}..{}", span.start, span.end)?;
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
		sorted(parse(src)?, functions).map_err(|error| ParseError(vec![ErrorMessage::from_prose(&error.to_string())]))
	}
}

/// The parser with zero-cost errors, built once per thread, since building the combinator tree costs as much as parsing a short
/// expression, and cached across input lifetimes by chumsky's cache.
struct FastParser;

impl Cached for FastParser {
	type Parser<'src> = Boxed<'src, 'src, Lexer<'src>, Syntax, extra::Default>;

	fn make_parser<'src>(self) -> <Self as Cached>::Parser<'src> {
		parser().boxed()
	}
}

/// The parser with rich errors, likewise built once per thread.
struct RichParser;

impl Cached for RichParser {
	type Parser<'src> = Boxed<'src, 'src, Lexer<'src>, Syntax, extra::Err<Rich<'src, Token<'src>, Span>>>;

	fn make_parser<'src>(self) -> <Self as Cached>::Parser<'src> {
		parser().boxed()
	}
}

thread_local! {
	static FAST_PARSER: Cache<FastParser> = Cache::new(FastParser);
	static RICH_PARSER: Cache<RichParser> = Cache::new(RichParser);
}

/// Parses the source as written, before its sorts are read.
fn parse(src: &str) -> Result<Syntax, ParseError> {
	// Parse with zero-cost errors first (several times faster), then re-parse invalid input with rich errors to build the messages
	if let Ok(ast) = FAST_PARSER.with(|cache| cache.get().parse(Lexer::new(src)).into_result()) {
		return Ok(ast);
	}

	match RICH_PARSER.with(|cache| cache.get().parse(Lexer::new(src)).into_result()) {
		Ok(ast) => Ok(ast),
		Err(parse_errs) => Err(ParseError(
			parse_errs
				.into_iter()
				.map(|e| {
					// Where `==` could have come next, the unexpected token followed a complete operand
					let after_operand = e.expected().any(|pattern| matches!(pattern, RichPattern::Token(token) if **token == Token::EqEq));

					match e.found() {
						Some(Token::Percent) => ErrorMessage::from_prose("`%` is reserved for percentages, so the remainder is written `mod(a, b)`").at(e.span()),
						Some(Token::If) => ErrorMessage::from_prose("`if` joins a case's value to its condition, like `{a if x > 0, b otherwise}`").at(e.span()),
						Some(Token::Otherwise) => ErrorMessage::from_prose("`otherwise` ends the one case with no condition, like `{a if x > 0, b otherwise}`").at(e.span()),
						Some(Token::Equals) if after_operand => ErrorMessage::from_prose("`=` names a value in a `where` clause, so equality is written `==`").at(e.span()),
						Some(Token::Where) if after_operand => {
							ErrorMessage::from_prose("`where` defines names for the whole expression or within parentheses, like `2 (a + b where a = 1, b = 2)`").at(e.span())
						}
						// The offending source is quoted as its own part, since it may hold anything, backticks included
						Some(Token::Error(error)) => {
							let text = src.get(e.span().start..e.span().end).unwrap_or_default();
							let reason = match error {
								LexError::Unrecognized => "is not recognized",
								LexError::MalformedNumber => "is not a valid number",
								LexError::NumberAfterNumber => "can't follow another number, so write them as one or put `*` between them",
								LexError::LeadingDotAfterOperand => "needs its leading zero after an operand, like `0.5`",
							};
							let reason = ErrorMessage::from_prose(&format!(" {reason}"));
							let parts = std::iter::once(MessagePart::Code(text.to_string())).chain(reason.parts).collect();
							ErrorMessage { parts, span: None }.at(e.span())
						}
						_ => match e.reason() {
							RichReason::Custom(message) => ErrorMessage::from_prose(message).at(e.span()),
							// Chumsky's wording: lowercase "found ... expected ...", no comma, tokens in single quotes, and "a, or b" for two alternatives
							RichReason::ExpectedFound { .. } => {
								let mut message = e.to_string().replacen(" expected ", ", expected ", 1);
								if e.expected().len() == 2 {
									message = message.replacen(", or ", " or ", 1);
								}
								let mut characters = message.chars();
								let sentence_case: String = characters.next().into_iter().flat_map(char::to_uppercase).chain(characters).collect();
								ErrorMessage::with_code_between(&sentence_case, '\'').at(e.span())
							}
						},
					}
				})
				.collect(),
		)),
	}
}

/// A `where` clause: bindings separated by commas, each a value like `a = 1` or a function like `f(t) = t^2`, whose defining
/// expression has no clause of its own unless parenthesized.
fn where_clause<'src, I, E>(expression: impl Parser<'src, I, Syntax, E> + Clone) -> impl Parser<'src, I, Vec<Binding>, E> + Clone
where
	I: ValueInput<'src, Token = Token<'src>, Span = Span>,
	E: extra::ParserExtra<'src, I>,
	E::Error: LabelError<'src, I, &'static str> + CustomError,
{
	// The `\` prefix always reaches the language's own builtin, so no clause can define a name that has it
	let name = select! {Token::Ident(name) => name}.labelled("a name").validate(|name: &str, extra, emitter| {
		if name.starts_with('\\') {
			emitter.emit(CustomError::custom(extra.span(), "A `\\` name is always the builtin, so no `where` clause can define one"));
		}
		name.to_string()
	});

	let parameters = name.separated_by(just(Token::Comma)).at_least(1).collect::<Vec<String>>();
	let binding = name
		.then(parameters.delimited_by(just(Token::LParen), just(Token::RParen)).or_not())
		.then_ignore(just(Token::Equals))
		.then(expression)
		.map(|((name, parameters), value)| Binding {
			name,
			parameters: parameters.unwrap_or_default(),
			value,
		});

	just(Token::Where).ignore_then(binding.separated_by(just(Token::Comma)).at_least(1).collect())
}

/// The expression, within the names its `where` clause defines if it has one.
fn with_bindings((body, bindings): (Syntax, Option<Vec<Binding>>)) -> Syntax {
	match bindings {
		Some(bindings) => Syntax::Where { body: Box::new(body), bindings },
		None => body,
	}
}

pub fn parser<'src, I, E>() -> impl Parser<'src, I, Syntax, E>
where
	I: ValueInput<'src, Token = Token<'src>, Span = Span>,
	E: extra::ParserExtra<'src, I>,
	E::Error: LabelError<'src, I, &'static str> + CustomError,
{
	let expression = recursive(|expr| {
		let constant = select! {
			Token::Integer(integer) => Syntax::Lit(Literal::Integer(integer)),
			Token::Float(float) => Syntax::Lit(Literal::Float(float)),
		};

		let args = expr
			.clone()
			.separated_by(just(Token::Comma))
			.collect::<Vec<_>>()
			.then(where_clause(expr.clone()).or_not())
			.delimited_by(just(Token::LParen), just(Token::RParen));

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

		let ident = select! {Token::Ident(s) => s}.labelled("a name");

		// An ident followed by parenthesized args is a function call, otherwise a variable
		let call_or_var = ident.then(args.or_not()).map(|(name, args)| match args {
			Some((args, None)) => Syntax::FnCall { name: name.to_string(), expr: args },
			Some((arguments, Some(bindings))) => Syntax::CallWhere(Box::new(CallWhere {
				name: name.to_string(),
				arguments,
				bindings,
			})),
			None => Syntax::Var(name.to_string()),
		});

		let parens = expr
			.clone()
			.then(where_clause(expr.clone()).or_not())
			.map(with_bindings)
			.delimited_by(just(Token::LParen), just(Token::RParen));
		let magnitude = expr.clone().delimited_by(just(Token::BarOpen), just(Token::BarClose)).map(|expr| Syntax::UnaryOp {
			op: UnaryOp::Magnitude,
			expr: Box::new(expr),
		});

		let atom = choice((constant, piecewise, matrix, call_or_var, parens, magnitude)).labelled("a value");

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
		let product = unary
			.clone()
			.then(choice((mul_op.then(unary), implicit_mul)).repeated().collect::<Vec<_>>())
			.map(|(first, mut rest): (Syntax, Vec<(BinaryOp, Syntax)>)| {
				// Two factors group only one way, while a longer chain is grouped by the sort pass, which knows the matrices
				if rest.len() <= 1 {
					return match rest.pop() {
						Some((op, second)) => Syntax::BinOp {
							lhs: Box::new(first),
							op,
							rhs: Box::new(second),
						},
						None => first,
					};
				}

				Syntax::Product { first: Box::new(first), rest }
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
			// Emitted rather than failed, since chumsky's `try_map` moves the deepest error found inside it back to its own start, hiding it behind the operator
			.validate(|(first, mut rest): (Syntax, Vec<(BinaryOp, Syntax)>), extra, emitter| {
				// A lone comparison is an ordinary binary operation
				if rest.len() <= 1 {
					return match rest.pop() {
						Some((op, second)) => Syntax::BinOp {
							lhs: Box::new(first),
							op,
							rhs: Box::new(second),
						},
						None => first,
					};
				}

				let ops: Vec<BinaryOp> = rest.iter().map(|(op, _)| *op).collect();
				if !BinaryOp::chain_in_one_direction(&ops) {
					emitter.emit(CustomError::custom(
						extra.span(),
						"A comparison chain must read in one direction: all ascending (`<`, `<=`, `==`), all descending (`>`, `>=`, `==`), or all `!=`",
					));
				}

				Syntax::Comparison { first: Box::new(first), rest }
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
	});

	expression.clone().then(where_clause(expression).or_not()).map(with_bindings)
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
		test_product_chain_stays_flat: "2 * 3 / 4" => Syntax::Product {
			first: Box::new(Syntax::Lit(Literal::Integer(2))),
			rest: vec![(BinaryOp::Mul, Syntax::Lit(Literal::Integer(3))), (BinaryOp::Div, Syntax::Lit(Literal::Integer(4)))],
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
			expr: vec![Syntax::Lit(Literal::Integer(16))],
		},
		// `i` is a name a binding may shadow, so only the evaluator can read this call as `i` times its argument
		test_parse_i_mul: "i(16)" => Syntax::FnCall {
			name: "i".to_string(),
			expr: vec![Syntax::Lit(Literal::Integer(16))],
		},
		test_call_where_clause: "max(a, b where a = 1)" => Syntax::CallWhere(Box::new(CallWhere {
			name: "max".to_string(),
			arguments: vec![Syntax::Var("a".to_string()), Syntax::Var("b".to_string())],
			bindings: vec![Binding {
				name: "a".to_string(),
				parameters: vec![],
				value: Syntax::Lit(Literal::Integer(1)),
			}],
		})),
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
		test_where_clause: "a where a = 1, f(t, u) = t" => Syntax::Where {
			body: Box::new(Syntax::Var("a".to_string())),
			bindings: vec![
				Binding {
					name: "a".to_string(),
					parameters: vec![],
					value: Syntax::Lit(Literal::Integer(1)),
				},
				Binding {
					name: "f".to_string(),
					parameters: vec!["t".to_string(), "u".to_string()],
					value: Syntax::Var("t".to_string()),
				},
			],
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
