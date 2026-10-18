//! The roles the lexer and grammar give each stretch of source, for an editor to color and typeset an expression as it is typed.

use crate::constants::{builtin_function, suffixed_function};
use crate::documentation::{Operator, Tooltip, constant_tooltip, function_tooltip, identity_tooltip, keyword_tooltip, matrix_literal_tooltip, range_tooltip, reducer_tooltip};
use crate::lexer::{Constant, Lexer, Token, names_matrix, operand_ended};
use crate::parser::parse_quietly;
use crate::sort::{NameUse, name_uses};
use std::collections::HashMap;

/// What a stretch of source is, as far as its spelling and its neighbors say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
	Number,
	/// A name, with any namespace prefix like the builtin's `\` taking the role of the name it addresses.
	Variable,
	Matrix,
	/// The basis vector `i`, `j`, or `k`.
	Basis,
	/// A called function, or the name a `where` clause defines one by.
	Function,
	Keyword,
	Operator,
	/// A grouping delimiter: a parenthesis, a brace, or a magnitude bar.
	Bracket,
	/// A matrix literal's brackets and separators, and a range's `..`.
	MatrixLiteral,
	Error,
}

/// A number as an editor may adjust it as one value, written from its start to the end of the number's highlight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberLiteral {
	/// Where it begins, at any sign right before the number.
	pub start: usize,
	/// Whether it follows an operand it multiplies, where a sign written before it would subtract instead.
	pub after_operand: bool,
}

/// A stretch of source with its role, the exponent depth it stands at, where its subscript begins, the delimiter it pairs with, the
/// group it separates the parts of, what it shows on hover, and the number literal it's part of.
#[derive(Clone, Debug, PartialEq)]
pub struct Highlight {
	pub start: usize,
	pub end: usize,
	pub role: Role,
	/// How many exponents deep the stretch stands, 0 outside any.
	pub superscript: u8,
	/// The byte offset within the stretch where a name's subscript begins.
	pub subscript: Option<usize>,
	/// The index among the highlights of the delimiter that opens or closes the group this one closes or opens.
	pub partner: Option<usize>,
	/// For a comma or semicolon, the index among the highlights of the opener of the group whose parts it separates.
	pub separates: Option<usize>,
	/// The documentation of a builtin, constant, keyword, operator, or lone reducer, or the drawing of a matrix or range literal.
	pub tooltip: Option<Tooltip>,
	/// For a finite number, the literal it's part of.
	pub literal: Option<NumberLiteral>,
}

/// Reads the source as the lexer does, giving each name the role the sort pass reads it with, telling a matrix literal's separators
/// from a call's, pairing each group's delimiters, and marking each exponent's operand as the grammar groups it. Whitespace is left out.
/// Where `accepts_reducers`, a lone operator or function name like `+` or `max` is one applied across a list.
pub fn highlight(source: &str, accepts_reducers: bool) -> Vec<Highlight> {
	let mut lexer = Lexer::new(source);
	let mut tokens = Vec::new();
	while let Some(spanned) = lexer.next_spanned() {
		tokens.push(spanned);
	}
	let uses: HashMap<usize, NameUse> = parse_quietly(source).map(|syntax| name_uses(syntax, source)).unwrap_or_default().into_iter().collect();

	let plain = |start: usize, end: usize, role: Role| Highlight {
		start,
		end,
		role,
		superscript: 0,
		subscript: None,
		partner: None,
		separates: None,
		tooltip: None,
		literal: None,
	};
	let mut highlights = Vec::with_capacity(tokens.len());
	// Each open group around the token, by its opener's index, the token that closes it, and whether a `where` clause has begun in it
	let mut open: Vec<(usize, Token, bool)> = Vec::new();
	let mut after_operand = false;
	// Where the previous token begins, where it's a sign rather than an addition or subtraction
	let mut sign: Option<usize> = None;
	for (index, (range, token)) in tokens.iter().enumerate() {
		let (start, end) = (range.start, range.end);
		let operator = operator(token, after_operand).map(Operator::tooltip);
		let (signed, follows_operand) = (sign.take(), after_operand);
		if matches!(token, Token::Plus | Token::Minus) && !after_operand {
			sign = Some(start);
		}
		after_operand = operand_ended(token, after_operand);
		let number = Highlight {
			literal: Some(NumberLiteral {
				start: signed.unwrap_or(start),
				after_operand: follows_operand,
			}),
			..plain(start, end, Role::Number)
		};

		match token {
			Token::Float(infinity) if infinity.is_infinite() => highlights.push(Highlight {
				tooltip: Some(constant_tooltip(Constant::Inf)),
				..plain(start, end, Role::Number)
			}),
			Token::Integer(_) | Token::Float(_) => highlights.push(number),
			Token::If | Token::Otherwise | Token::Where => {
				if *token == Token::Where
					&& let Some((.., in_clause)) = open.last_mut()
				{
					*in_clause = true;
				}
				highlights.push(Highlight {
					tooltip: keyword_tooltip(token),
					..plain(start, end, Role::Keyword)
				});
			}
			Token::Ident(name) => {
				let bare = name.strip_prefix('\\').unwrap_or(name);
				let prefix = name.len() - bare.len();
				let role = match uses.get(&start) {
					Some(NameUse::Function | NameUse::LocalFunction) => Role::Function,
					Some(NameUse::Local) if names_matrix(bare) => Role::Matrix,
					Some(NameUse::Local) => Role::Variable,
					// A free name, or any while the expression doesn't parse, goes by its spelling, calling only a builtin
					_ => {
						let called = tokens.get(index + 1).is_some_and(|(_, next)| *next == Token::LParen);
						if names_matrix(bare) {
							Role::Matrix
						} else if called && (builtin_function(bare).is_some() || suffixed_function(bare).is_some()) {
							Role::Function
						} else if matches!(bare, "i" | "j" | "k") {
							Role::Basis
						} else {
							Role::Variable
						}
					}
				};

				// A builtin or constant is documented wherever the name reaches it, which a definition of the name stops
				let tooltip = match (uses.get(&start), role) {
					(Some(NameUse::Local | NameUse::LocalFunction), _) => None,
					(_, Role::Function) => function_tooltip(bare),
					(_, Role::Matrix) => (bare == "I").then(identity_tooltip),
					_ => Constant::from_name(bare).map(constant_tooltip),
				};
				highlights.push(Highlight {
					subscript: subscript_of(bare, role).map(|offset| prefix + offset),
					tooltip,
					..plain(start, end, role)
				});
			}
			// The transpose's `T` is raised like an exponent, with any space between it and the `^` left out
			Token::Transpose => {
				highlights.push(Highlight {
					tooltip: operator.clone(),
					..plain(start, start + 1, Role::Operator)
				});
				highlights.push(Highlight {
					tooltip: operator,
					..plain(end - 1, end, Role::Operator)
				});
			}
			Token::LParen | Token::LBrace | Token::LBracket | Token::BarOpen => {
				let closer = match token {
					Token::LParen => Token::RParen,
					Token::LBrace => Token::RBrace,
					Token::LBracket => Token::RBracket,
					_ => Token::BarClose,
				};
				open.push((highlights.len(), closer, false));
				highlights.push(Highlight {
					tooltip: operator,
					..plain(start, end, if *token == Token::LBracket { Role::MatrixLiteral } else { Role::Bracket })
				});
			}
			Token::RParen | Token::RBrace | Token::RBracket | Token::BarClose => {
				let mut closing = Highlight {
					tooltip: operator,
					..plain(start, end, if *token == Token::RBracket { Role::MatrixLiteral } else { Role::Bracket })
				};

				// A closer pairs with the innermost open group only where it's the kind that closes it, leaving a mismatch unpaired
				if let Some(&(opener, ..)) = open.last().filter(|(_, closer, _)| closer == token) {
					open.pop();
					highlights[opener].partner = Some(highlights.len());
					closing.partner = Some(opener);
				}
				highlights.push(closing);
			}
			// A comma or semicolon separates the parts of the innermost open group, colored as a matrix literal's where it's square, except
			// that a `where` clause's definitions aren't the group's parts
			Token::Comma | Token::Semicolon => {
				let role = if *token == Token::Semicolon || open.last().is_some_and(|(_, closer, _)| *closer == Token::RBracket) {
					Role::MatrixLiteral
				} else {
					Role::Operator
				};
				highlights.push(Highlight {
					separates: open.last().filter(|(.., in_clause)| !in_clause).map(|(opener, ..)| *opener),
					..plain(start, end, role)
				});
			}
			Token::DotDot => highlights.push(plain(start, end, Role::MatrixLiteral)),
			Token::Error(_) => highlights.push(plain(start, end, Role::Error)),
			_ => highlights.push(Highlight {
				tooltip: operator,
				..plain(start, end, Role::Operator)
			}),
		}
	}

	mark_exponents(source, &mut highlights);
	draw_literals(source, &mut highlights);

	if accepts_reducers
		&& let [lone] = highlights.as_mut_slice()
		&& let Some(tooltip) = reducer_tooltip(source)
	{
		lone.tooltip = Some(tooltip);
		if lone.role == Role::Variable {
			lone.role = Role::Function;
		}
	}

	highlights
}

/// The documented operator a token is, as told by whether an operand has just ended before it.
fn operator(token: &Token, after_operand: bool) -> Option<Operator> {
	Some(match token {
		Token::Plus if after_operand => Operator::Add,
		Token::Minus if after_operand => Operator::Subtract,
		Token::Plus => Operator::Positive,
		Token::Minus => Operator::Negate,
		Token::Star => Operator::Multiply,
		Token::Slash => Operator::Divide,
		Token::Caret => Operator::Power,
		Token::Transpose => Operator::Transpose,
		Token::Bang if after_operand => Operator::Factorial,
		Token::Bang | Token::Not => Operator::Not,
		Token::EqEq => Operator::Equal,
		Token::Neq => Operator::NotEqual,
		Token::Lt => Operator::Less,
		Token::Le => Operator::LessOrEqual,
		Token::Gt => Operator::Greater,
		Token::Ge => Operator::GreaterOrEqual,
		Token::AndAnd => Operator::And,
		Token::OrOr => Operator::Or,
		Token::Equals => Operator::Define,
		Token::BarOpen | Token::BarClose => Operator::Magnitude,
		_ => return None,
	})
}

/// Gives a closed matrix literal's brackets and separators the tooltip drawing its matrix, and a range's `..` the one drawing its map.
fn draw_literals(source: &str, highlights: &mut [Highlight]) {
	let text = |highlight: &Highlight| &source[highlight.start..highlight.end];

	for index in 0..highlights.len() {
		match text(&highlights[index]) {
			"[" => {
				let Some(closer) = highlights[index].partner else { continue };
				let separators = (index..closer).filter(|&separator| highlights[separator].separates == Some(index)).collect::<Vec<_>>();

				// The entries between the separators, which are all commas or all semicolons in a literal that parses
				let bounds = [index].into_iter().chain(separators.iter().copied()).chain([closer]).collect::<Vec<_>>();
				let entries = bounds.windows(2).map(|pair| &source[highlights[pair[0]].end..highlights[pair[1]].start]).collect::<Vec<_>>();
				let semicolons = separators.iter().filter(|&&separator| text(&highlights[separator]) == ";").count();
				if semicolons != 0 && semicolons != separators.len() {
					continue;
				}

				let tooltip = matrix_literal_tooltip(&entries, semicolons == separators.len());
				for part in [index, closer].into_iter().chain(separators) {
					highlights[part].tooltip = Some(tooltip.clone());
				}
			}
			".." => {
				if let (Some(from), Some(to)) = (range_corner(source, highlights, index, false), range_corner(source, highlights, index, true)) {
					highlights[index].tooltip = Some(range_tooltip(from, to));
				}
			}
			_ => {}
		}
	}
}

/// The source of a range's corner before or after its `..`, out to whatever binds looser than a range or to the enclosing group's
/// delimiter, crossing any group within it whole.
fn range_corner<'src>(source: &'src str, highlights: &[Highlight], dots: usize, forward: bool) -> Option<&'src str> {
	const LOOSER: [&str; 18] = [",", ";", "<", "<=", ">", ">=", "==", "!=", "≠", "≤", "≥", "&&", "||", "∧", "∨", "=", "..", "|"];
	let text = |highlight: &Highlight| &source[highlight.start..highlight.end];
	let step = |index: usize| if forward { index.checked_add(1) } else { index.checked_sub(1) };

	let mut reached = dots;
	while let Some(next) = step(reached)
		&& let Some(highlight) = highlights.get(next)
		&& highlight.role != Role::Keyword
		&& (!LOOSER.contains(&text(highlight)) || highlight.partner.is_some())
	{
		reached = match highlight.partner {
			Some(partner) if (partner > next) == forward => partner,
			Some(_) => break,
			None if ["(", "[", "{", ")", "]", "}"].contains(&text(highlight)) => break,
			None => next,
		};
	}

	let (first, last) = if forward { (dots + 1, reached) } else { (reached, dots.checked_sub(1)?) };
	(reached != dots).then(|| &source[highlights[first].start..highlights[last].end])
}

/// Where a name's subscript begins: at its first underscore, at a variable's or matrix's trailing digits, or after the `log`
/// or `root` a suffixed builtin's base follows, since any other function's digits are part of its name.
fn subscript_of(name: &str, role: Role) -> Option<usize> {
	if let Some(underscore) = name.find('_') {
		return Some(underscore);
	}
	match role {
		Role::Variable | Role::Matrix => {
			let digits = name.trim_end_matches(|c: char| c.is_ascii_digit()).len();
			(digits < name.len()).then_some(digits)
		}
		Role::Function => suffixed_function(name).map(|suffixed| suffixed.name.len()),
		_ => None,
	}
}

/// Marks each `^`'s operand one exponent deeper than the `^` itself, as the grammar groups it: a number or name, a call or group
/// whole, or the transpose's `T`, with any signs or nots before it and factorials after it. A `^` right after the operand continues
/// the exponent, so `a^b^c` nests `c` within `b`, and a `^` within a group nests its operand likewise.
fn mark_exponents(source: &str, highlights: &mut [Highlight]) {
	let is = |highlight: &Highlight, role: Role, spellings: &[&str]| highlight.role == role && spellings.contains(&&source[highlight.start..highlight.end]);

	for caret in 0..highlights.len() {
		if !is(&highlights[caret], Role::Operator, &["^"]) {
			continue;
		}
		let level = highlights[caret].superscript.saturating_add(1);
		let mut index = caret + 1;

		while highlights.get(index).is_some_and(|prefix| is(prefix, Role::Operator, &["+", "-", "!", "¬"])) {
			highlights[index].superscript = level;
			index += 1;
		}
		let Some(operand) = highlights.get_mut(index) else { continue };
		operand.superscript = level;

		// A name against parentheses is one operand with them, as a call or as a product, as the grammar reads it
		let named = matches!(operand.role, Role::Variable | Role::Matrix | Role::Basis | Role::Function);
		if named && highlights.get(index + 1).is_some_and(|open| is(open, Role::Bracket, &["("])) {
			index += 1;
		}

		// A group runs from its opener to the partner closing it, or to the end while its closer isn't typed yet
		let Some(group) = highlights.get(index) else { continue };
		let last = match group.partner {
			Some(partner) if partner > index => partner,
			None if ["(", "[", "{", "|"].contains(&&source[group.start..group.end]) => highlights.len() - 1,
			_ => index,
		};
		highlights[index..=last].iter_mut().for_each(|highlight| highlight.superscript = level);
		index = last + 1;

		while highlights.get(index).is_some_and(|factorial| is(factorial, Role::Operator, &["!"])) {
			highlights[index].superscript = level;
			index += 1;
		}
		if highlights.get(index).is_some_and(|next| is(next, Role::Operator, &["^"])) {
			highlights[index].superscript = level;
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Each highlight as its text, role, exponent depth, and subscript offset.
	fn read(source: &str) -> Vec<(&str, Role, u8, Option<usize>)> {
		highlight(source, false)
			.into_iter()
			.map(|highlight| (&source[highlight.start..highlight.end], highlight.role, highlight.superscript, highlight.subscript))
			.collect()
	}

	#[test]
	fn a_number_literal_takes_in_its_sign() {
		// Each literal's text from any sign, and whether it multiplies an operand before it
		let literals = |source: &str| {
			let highlights = highlight(source, false).into_iter();
			let literals = highlights.filter_map(|highlight| highlight.literal.map(|literal| (literal, highlight.end)));
			literals.map(|(literal, end)| (source[literal.start..end].to_string(), literal.after_operand)).collect::<Vec<_>>()
		};
		let literal = |text: &str, after_operand: bool| (text.to_string(), after_operand);

		assert_eq!(literals("-2 - 3"), [literal("-2", false), literal("3", false)]);
		assert_eq!(literals("2^-1.5 * +4"), [literal("2", false), literal("-1.5", false), literal("+4", false)]);
		assert_eq!(literals("[1; i] 8 sqrt(x)"), [literal("1", false), literal("8", true)]);
		assert!(literals("2 inf").len() == 1, "infinity isn't adjusted as a number");
	}

	#[test]
	fn roles_follow_spelling_and_neighbors() {
		use Role::*;
		assert_eq!(
			read("2x^2 + sin(x) where x = 1.5e-3"),
			[
				("2", Number, 0, None),
				("x", Variable, 0, None),
				("^", Operator, 0, None),
				("2", Number, 1, None),
				("+", Operator, 0, None),
				("sin", Function, 0, None),
				("(", Bracket, 0, None),
				("x", Variable, 0, None),
				(")", Bracket, 0, None),
				("where", Keyword, 0, None),
				("x", Variable, 0, None),
				("=", Operator, 0, None),
				("1.5e-3", Number, 0, None),
			]
		);

		// A matrix's name applied to an argument is its product, and a matrix literal's separators are told from a call's
		assert_eq!(
			read("[a, b; c] max(a, b) M(v) 1..5 \\pi θ Δx m@"),
			[
				("[", MatrixLiteral, 0, None),
				("a", Variable, 0, None),
				(",", MatrixLiteral, 0, None),
				("b", Variable, 0, None),
				(";", MatrixLiteral, 0, None),
				("c", Variable, 0, None),
				("]", MatrixLiteral, 0, None),
				("max", Function, 0, None),
				("(", Bracket, 0, None),
				("a", Variable, 0, None),
				(",", Operator, 0, None),
				("b", Variable, 0, None),
				(")", Bracket, 0, None),
				("M", Matrix, 0, None),
				("(", Bracket, 0, None),
				("v", Variable, 0, None),
				(")", Bracket, 0, None),
				("1", Number, 0, None),
				("..", MatrixLiteral, 0, None),
				("5", Number, 0, None),
				("\\pi", Variable, 0, None),
				("θ", Variable, 0, None),
				("Δx", Matrix, 0, None),
				("m", Variable, 0, None),
				("@", Error, 0, None),
			]
		);
		assert_eq!(
			read("3i + 2j k"),
			[
				("3", Number, 0, None),
				("i", Basis, 0, None),
				("+", Operator, 0, None),
				("2", Number, 0, None),
				("j", Basis, 0, None),
				("k", Basis, 0, None)
			]
		);
	}

	#[test]
	fn exponents_nest_as_the_grammar_groups_them() {
		fn depths(source: &str) -> Vec<(&str, u8)> {
			read(source).into_iter().map(|(text, _, depth, _)| (text, depth)).collect()
		}

		assert_eq!(depths("a^b^c"), [("a", 0), ("^", 0), ("b", 1), ("^", 1), ("c", 2)]);
		assert_eq!(depths("x^(a^b) + 2"), [("x", 0), ("^", 0), ("(", 1), ("a", 1), ("^", 1), ("b", 2), (")", 1), ("+", 0), ("2", 0)]);
		assert_eq!(
			depths("x^f(y^2)^3 z"),
			[("x", 0), ("^", 0), ("f", 1), ("(", 1), ("y", 1), ("^", 1), ("2", 2), (")", 1), ("^", 1), ("3", 2), ("z", 0)]
		);
		assert_eq!(
			depths("A^T^-1 + 2^ 3 ^ 4"),
			[("A", 0), ("^", 0), ("T", 1), ("^", 1), ("-", 2), ("1", 2), ("+", 0), ("2", 0), ("^", 0), ("3", 1), ("^", 1), ("4", 2)]
		);
		assert_eq!(depths("e^\\pi y"), [("e", 0), ("^", 0), ("\\pi", 1), ("y", 0)]);
		assert_eq!(depths("x^"), [("x", 0), ("^", 0)]);

		// Signs and nots before the operand and factorials after it are part of the exponent
		assert_eq!(depths("2^3! y"), [("2", 0), ("^", 0), ("3", 1), ("!", 1), ("y", 0)]);
		assert_eq!(depths("2^--3"), [("2", 0), ("^", 0), ("-", 1), ("-", 1), ("3", 1)]);
		assert_eq!(depths("2^!0 + 1"), [("2", 0), ("^", 0), ("!", 1), ("0", 1), ("+", 0), ("1", 0)]);
		assert_eq!(depths("2^3!^2"), [("2", 0), ("^", 0), ("3", 1), ("!", 1), ("^", 1), ("2", 2)]);

		// Any group is raised whole, and one whose closer isn't typed yet runs to the end
		assert_eq!(depths("x^|v| y"), [("x", 0), ("^", 0), ("|", 1), ("v", 1), ("|", 1), ("y", 0)]);
		assert_eq!(depths("x^|v + 1"), [("x", 0), ("^", 0), ("|", 1), ("v", 1), ("+", 1), ("1", 1)]);
		assert_eq!(
			depths("2^{a if c, b otherwise} y"),
			[("2", 0), ("^", 0), ("{", 1), ("a", 1), ("if", 1), ("c", 1), (",", 1), ("b", 1), ("otherwise", 1), ("}", 1), ("y", 0)]
		);
		assert_eq!(depths("x^(a + b"), [("x", 0), ("^", 0), ("(", 1), ("a", 1), ("+", 1), ("b", 1)]);

		// Exponents nested deeper than a depth can count stay at the deepest
		let deep = format!("@ {}x", "x^".repeat(300));
		assert_eq!(highlight(&deep, false).iter().map(|highlight| highlight.superscript).max(), Some(u8::MAX));
	}

	#[test]
	fn names_take_the_role_the_sort_pass_reads_them_with() {
		use Role::*;
		fn names(source: &str) -> Vec<(&str, Role)> {
			read(source)
				.into_iter()
				.filter(|(_, role, _, _)| matches!(role, Variable | Matrix | Basis | Function))
				.map(|(text, role, _, _)| (text, role))
				.collect()
		}

		// A name against parentheses, spaced or not, is called only where a function has the name, and otherwise multiplies them
		assert_eq!(
			names("x (5i + 4j) + sin (x) + M(x)"),
			[("x", Variable), ("i", Basis), ("j", Basis), ("sin", Function), ("x", Variable), ("M", Matrix), ("x", Variable)]
		);

		// A function a clause defines is one where it's defined and where it's called, while its parameters and the clause's values are local
		assert_eq!(
			names("f10(2) + k(3) where f10(t) = t, k = 2"),
			[("f10", Function), ("k", Variable), ("f10", Function), ("t", Variable), ("t", Variable), ("k", Variable)]
		);

		// A value or parameter named like a basis vector or a builtin shadows it where it's read, while a call still reaches the builtin
		assert_eq!(
			names("i + g(j) + sin(sin) where i = 2, g(j) = j, sin = 3"),
			[
				("i", Variable),
				("g", Function),
				("j", Basis),
				("sin", Function),
				("sin", Variable),
				("i", Variable),
				("g", Function),
				("j", Variable),
				("j", Variable),
				("sin", Variable),
			]
		);

		// While the expression doesn't parse, a call is one only where a builtin has the name
		assert_eq!(
			names("x (5i + sin (x) + f (x) +"),
			[("x", Variable), ("i", Basis), ("sin", Function), ("x", Variable), ("f", Variable), ("x", Variable)]
		);

		// Names are read on past an error elsewhere in the expression, here a value given to `det`
		assert_eq!(
			names("det(m) + i + f(1) where m = 1, i = 2, f(t) = t"),
			[
				("det", Function),
				("m", Variable),
				("i", Variable),
				("f", Function),
				("m", Variable),
				("i", Variable),
				("f", Function),
				("t", Variable),
				("t", Variable)
			]
		);
	}

	#[test]
	fn delimiters_pair_with_the_group_they_open_or_close() {
		fn partners(source: &str) -> Vec<(&str, Option<&str>)> {
			let highlights = highlight(source, false);
			let text = |index: usize| &source[highlights[index].start..highlights[index].end];
			(0..highlights.len())
				.filter(|&index| matches!(highlights[index].role, Role::Bracket | Role::MatrixLiteral) && text(index) != ",")
				.map(|index| (text(index), highlights[index].partner.map(text)))
				.collect()
		}
		let indices = |source: &str| highlight(source, false).into_iter().map(|highlight| highlight.partner).collect::<Vec<_>>();

		// Each kind pairs within its own nesting, magnitude bars included, as the lexer reads which bar opens and which closes
		assert_eq!(
			indices("(a [b] {c} |d|)"),
			[Some(11), None, Some(4), None, Some(2), Some(7), None, Some(5), Some(10), None, Some(8), Some(0)]
		);
		assert_eq!(partners("||a| - b|"), [("|", Some("|")), ("|", Some("|")), ("|", Some("|")), ("|", Some("|"))]);
		assert_eq!(indices("||a| - b|"), [Some(6), Some(3), None, Some(1), None, None, Some(0)]);

		// A closer of the wrong kind, or with nothing open, is left unpaired, as is an opener never closed
		assert_eq!(partners("(a] b)"), [("(", Some(")")), ("]", None), (")", Some("("))]);
		assert_eq!(partners("f(x"), [("(", None)]);
		assert_eq!(partners("x)"), [(")", None)]);
	}

	#[test]
	fn separators_belong_to_the_innermost_group() {
		let separators = |source: &str| {
			let highlights = highlight(source, false);
			let separator = |highlight: &Highlight| matches!(&source[highlight.start..highlight.end], "," | ";");
			highlights
				.into_iter()
				.enumerate()
				.filter(|(_, highlight)| separator(highlight))
				.map(|(index, highlight)| (index, highlight.separates))
				.collect::<Vec<_>>()
		};

		// A call's arguments, a matrix literal's entries and rows, and a piecewise's cases each separate their own group
		assert_eq!(separators("f(a, [b, c; d], e)"), [(3, Some(1)), (6, Some(4)), (8, Some(4)), (11, Some(1))]);
		assert_eq!(separators("{a if b, c otherwise}"), [(4, Some(0))]);

		// A `where` clause's definitions separate no group's parts, wherever the clause stands, though a group within them has its own
		assert_eq!(separators("a where a = 1, b = 2"), [(5, None)]);
		assert_eq!(separators("2 (a where a = 1, b = 2)"), [(7, None)]);
		assert_eq!(separators("max(a, b where a = 1, b = 2)"), [(3, Some(1)), (9, None)]);
		assert_eq!(separators("(f(1) where f(t, u) = t)"), [(9, Some(7))]);
	}

	#[test]
	fn builtins_constants_operators_and_literals_have_tooltips() {
		fn labels(source: &str) -> Vec<(&str, String)> {
			let highlights = highlight(source, false)
				.into_iter()
				.filter_map(|highlight| Some((&source[highlight.start..highlight.end], highlight.tooltip?.label)));
			highlights.collect()
		}

		// An operator is told by whether an operand ends before it, and a name only where it reaches the builtin or constant
		assert_eq!(
			labels("-sin(x) + pi - 2! * !\\pi where x = 1, e = 2"),
			[
				("-", "Negate"),
				("sin", "Sine"),
				("+", "Add"),
				("pi", "Pi"),
				("-", "Subtract"),
				("!", "Factorial"),
				("*", "Multiply"),
				("!", "Not"),
				("\\pi", "Pi"),
				("where", "Where Clause"),
				("=", "Definition"),
				("=", "Definition"),
			]
			.map(|(text, label)| (text, label.to_string()))
		);
		assert_eq!(
			labels("|A^T I v| ∞ log2(8)"),
			[
				("|", "Magnitude"),
				("^", "Transpose"),
				("T", "Transpose"),
				("I", "Identity Matrix"),
				("|", "Magnitude"),
				("∞", "Infinity"),
				("log2", "Logarithm")
			]
			.map(|(text, label)| (text, label.to_string()))
		);

		// A matrix literal's brackets and separators draw its matrix, and a range's `..` its map from the corners on either side
		let tooltips = |source: &str| highlight(source, false).into_iter().filter_map(|highlight| highlight.tooltip).collect::<Vec<_>>();
		assert_eq!(tooltips("[1, 2x]"), vec![matrix_literal_tooltip(&["1", " 2x"], false); 3]);
		assert_eq!(tooltips("within(0, 1 + 1..2 (3))")[1..], [Operator::Add.tooltip(), range_tooltip("2", "6")]);
		assert_eq!(
			tooltips("x < -1..1 && y"),
			[Operator::Less.tooltip(), Operator::Negate.tooltip(), range_tooltip("-1", "1"), Operator::And.tooltip()]
		);
	}

	#[test]
	fn a_lone_reducer_is_documented_where_one_is_accepted() {
		let lone = |source: &str, accepts_reducers: bool| {
			highlight(source, accepts_reducers)
				.into_iter()
				.map(|highlight| (highlight.role, highlight.tooltip.map(|tooltip| tooltip.label)))
				.collect::<Vec<_>>()
		};

		assert_eq!(lone(" + ", true), [(Role::Operator, Some("Add".to_string()))]);
		assert_eq!(lone("max", true), [(Role::Function, Some("Maximum".to_string()))]);
		assert_eq!(lone("!=", true), [(Role::Operator, Some("Not Equal".to_string()))]);
		assert_eq!(reducer_tooltip("^").unwrap().form.as_deref(), Some("a^b^c^…"));
		let mean = reducer_tooltip("mean").unwrap();
		assert_eq!(mean.form.as_deref(), Some("mean(a, b, c, …)"));
		assert!(mean.description.starts_with("The sum of its arguments"));

		// Elsewhere, or beside anything else, it's read as any other expression
		assert_eq!(lone("+", false), [(Role::Operator, Some("Unary Plus".to_string()))]);
		assert_eq!(lone("max", false), [(Role::Variable, None)]);
		assert_eq!(lone("sin", true), [(Role::Variable, None)]);
		assert_eq!(lone("+ 1", true)[0], (Role::Operator, Some("Unary Plus".to_string())));
	}

	#[test]
	fn subscripts_are_underscores_trailing_digits_and_builtin_suffixes() {
		fn subscripts(source: &str) -> Vec<(&str, Option<usize>)> {
			read(source)
				.into_iter()
				.filter(|(_, role, _, _)| *role != Role::Bracket)
				.map(|(text, _, _, subscript)| (text, subscript))
				.collect()
		}

		assert_eq!(
			subscripts("x_1 x1 M1 x_max f_1(x)"),
			[("x_1", Some(1)), ("x1", Some(1)), ("M1", Some(1)), ("x_max", Some(1)), ("f_1", Some(1)), ("x", None)]
		);
		assert_eq!(
			subscripts("f10(x) log10(x) log_10(x) log2.5(x) root3(x) \\log10(x)"),
			[
				("f10", Some(1)),
				("x", None),
				("log10", Some(3)),
				("x", None),
				("log_10", Some(3)),
				("x", None),
				("log2.5", Some(3)),
				("x", None),
				("root3", Some(4)),
				("x", None),
				("\\log10", Some(4)),
				("x", None)
			]
		);
	}
}
