use crate::quaternion::Quaternion;
use crate::value::{Complex, Number};
use chumsky::input::{Input, ValueInput};
use chumsky::span::SimpleSpan;
use std::fmt;
use std::ops::Range;

pub type Span = SimpleSpan;

#[derive(Clone, Debug, PartialEq)]
pub enum Token<'src> {
	/// A whole-number literal that fits exact integer storage.
	Integer(i64),
	Float(f64),
	Ident(&'src str),

	AndAnd,
	OrOr,
	Bang,
	Not,
	BarOpen,
	BarClose,

	LParen,
	RParen,
	LBrace,
	RBrace,
	Comma,
	Plus,
	Minus,
	/// Reserved for percentages, so the parser never matches it and its error points a C-style remainder to `mod(a, b)`.
	Percent,
	Star,
	Slash,
	Caret,

	Lt,
	Le,
	Gt,
	Ge,
	Neq,
	EqEq,

	If,
	Otherwise,
	/// Reserved for `where` bindings, so the parser never matches it yet and no host binding can claim the name first.
	Where,

	/// An unrecognized character; the parser never matches this, forcing a parse error rather than silently truncating the input.
	Error,
}

impl<'src> fmt::Display for Token<'src> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Token::Integer(x) => write!(f, "{x}"),
			Token::Float(x) => write!(f, "{x}"),
			Token::Ident(name) => write!(f, "{name}"),

			Token::AndAnd => f.write_str("&&"),
			Token::OrOr => f.write_str("||"),
			Token::Bang => f.write_str("!"),
			Token::Not => f.write_str("¬"),
			Token::BarOpen | Token::BarClose => f.write_str("|"),

			Token::LParen => f.write_str("("),
			Token::RParen => f.write_str(")"),
			Token::LBrace => f.write_str("{"),
			Token::RBrace => f.write_str("}"),
			Token::Comma => f.write_str(","),
			Token::Plus => f.write_str("+"),
			Token::Minus => f.write_str("-"),
			Token::Percent => f.write_str("%"),
			Token::Star => f.write_str("*"),
			Token::Slash => f.write_str("/"),
			Token::Caret => f.write_str("^"),

			Token::Lt => f.write_str("<"),
			Token::Le => f.write_str("<="),
			Token::Gt => f.write_str(">"),
			Token::Ge => f.write_str(">="),
			Token::Neq => f.write_str("!="),
			Token::EqEq => f.write_str("=="),

			Token::If => f.write_str("if"),
			Token::Otherwise => f.write_str("otherwise"),
			Token::Where => f.write_str("where"),

			Token::Error => f.write_str("<error>"),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Constant {
	Pi,
	Tau,
	E,
	Phi,
	Inf,
	I,
	J,
	K,
	True,
	False,
}

impl Constant {
	pub fn value(self) -> Number {
		use Constant::*;
		use std::f64::consts;
		match self {
			Pi => Number::Real(consts::PI),
			Tau => Number::Real(consts::TAU),
			E => Number::Real(consts::E),
			// TODO: Replace with f64::GOLDEN_RATIO when we bump MSRV to 1.94
			Phi => Number::Real(1.618033988749895),
			Inf => Number::Real(f64::INFINITY),
			I => Number::Complex(Complex::new(0., 1.)),
			J => Number::Quaternion(Quaternion::J),
			K => Number::Quaternion(Quaternion::K),
			True => Number::from_bool(true),
			False => Number::from_bool(false),
		}
	}

	/// The word and typeset spellings, matched exactly: constants are lowercase-only, since uppercase-initial names are reserved for matrices.
	pub fn from_name(name: &str) -> Option<Constant> {
		use Constant::*;
		let spellings = [
			("e", E),
			("i", I),
			("j", J),
			("k", K),
			("pi", Pi),
			("π", Pi),
			("tau", Tau),
			("τ", Tau),
			("phi", Phi),
			("φ", Phi),
			("inf", Inf),
			("infinity", Inf),
			("true", True),
			("false", False),
		];
		spellings.into_iter().find_map(|(spelling, constant)| (name == spelling).then_some(constant))
	}
}

impl fmt::Display for Constant {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		use Constant::*;
		f.write_str(match self {
			Pi => "pi",
			Tau => "tau",
			E => "e",
			Phi => "phi",
			Inf => "inf",
			I => "i",
			J => "j",
			K => "k",
			True => "true",
			False => "false",
		})
	}
}

/// How a `|` reads at its position: opening or closing a magnitude, or, as the first of a `||` pair, the Or operator.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Bar {
	Open,
	Close,
	Or,
}

/// The token of a reserved word, which is never a name, whoever would bind it.
fn keyword(word: &str) -> Option<Token<'static>> {
	match word {
		"if" => Some(Token::If),
		"otherwise" => Some(Token::Otherwise),
		"where" => Some(Token::Where),
		_ => None,
	}
}

/// Whether a character ends an operand: a name, a number, a closing parenthesis or brace, or the `∞` literal.
fn ends_operand(c: char) -> bool {
	c.is_alphanumeric() || unicode_ident::is_xid_continue(c) || matches!(c, '.' | ')' | '}' | '∞')
}

/// Reads every `|` in the source up front, since each depends on what precedes it: a bar opens a magnitude where an operand
/// is expected and closes one after an operand. A `||` likewise opens or closes two, but is Or after an operand unless two are
/// open, or with nothing after it. So `|a||b|` is the magnitude of `a` or `b`, `||a|-b|` and `|a*|b||` nest, and `|a|b||` is rejected.
fn classify_bars(input: &str) -> Vec<(usize, Bar)> {
	let mut bars = Vec::new();
	let mut depth = 0_usize;
	let mut after_operand = false;
	let mut chars = input.char_indices().peekable();

	while let Some((position, c)) = chars.next() {
		match c {
			'|' if chars.next_if(|(_, next)| *next == '|').is_some() => {
				let at_end = chars.clone().all(|(_, c)| c.is_whitespace());
				if !after_operand && !at_end {
					bars.push((position, Bar::Open));
					bars.push((position + 1, Bar::Open));
					depth += 2;
				} else if depth >= 2 {
					bars.push((position, Bar::Close));
					bars.push((position + 1, Bar::Close));
					depth -= 2;
				} else {
					bars.push((position, Bar::Or));
					after_operand = false;
				}
			}
			'|' if after_operand && depth > 0 => {
				bars.push((position, Bar::Close));
				depth -= 1;
				after_operand = true;
			}
			'|' => {
				bars.push((position, Bar::Open));
				depth += 1;
				after_operand = false;
			}
			// Whitespace changes nothing, and neither does `!`, a postfix factorial after an operand or a prefix not before one
			c if c.is_whitespace() || c == '!' => {}
			// A name ends an operand, but a keyword like `if` comes before one
			c if c == '\\' || unicode_ident::is_xid_start(c) => {
				let mut end = position + c.len_utf8();
				while let Some(&(next_position, next)) = chars.peek()
					&& unicode_ident::is_xid_continue(next)
				{
					end = next_position + next.len_utf8();
					chars.next();
				}
				after_operand = keyword(&input[position..end]).is_none();
			}
			c => after_operand = ends_operand(c),
		}
	}

	bars
}

pub struct Lexer<'a> {
	input: &'a str,
	pos: usize,
	bars: Vec<(usize, Bar)>,
}

impl<'a> Lexer<'a> {
	pub fn new(input: &'a str) -> Self {
		Self {
			input,
			pos: 0,
			bars: classify_bars(input),
		}
	}

	/// The reading of the `|` at the given byte position.
	fn bar_at(&self, position: usize) -> Option<Bar> {
		self.bars.binary_search_by_key(&position, |(bar_position, _)| *bar_position).ok().map(|index| self.bars[index].1)
	}

	fn peek(&self) -> Option<char> {
		self.input[self.pos..].chars().next()
	}

	fn bump(&mut self) -> Option<char> {
		let c = self.peek()?;
		self.pos += c.len_utf8();
		Some(c)
	}

	fn consume_while<F>(&mut self, cond: F) -> &'a str
	where
		F: Fn(char) -> bool,
	{
		let start = self.pos;
		while self.peek().is_some_and(&cond) {
			self.bump();
		}
		&self.input[start..self.pos]
	}

	fn consume_digits(&mut self) -> (usize, f64) {
		let mut value = 0_f64;
		let mut digits = 0;
		while let Some(d) = self.peek().and_then(|c| c.to_digit(10)) {
			value = value * 10. + d as f64;
			digits += 1;
			self.bump();
		}
		(digits, value)
	}

	// Two number literals never juxtapose, so digit grouping like `10 000` can't silently multiply
	fn follows_number_literal(&self, literal_start: usize) -> bool {
		let preceding = self.input[..literal_start].trim_end();

		// The preceding token begins within its run of name and number characters (`2pi`, `1e5`), whose start is a token boundary to lex from
		let run_start = preceding
			.char_indices()
			.rev()
			.take_while(|&(_, c)| unicode_ident::is_xid_continue(c) || c == '.')
			.last()
			.map_or(preceding.len(), |(index, _)| index);
		Lexer::new(&preceding[run_start..]).last().is_some_and(|token| matches!(token, Token::Integer(_) | Token::Float(_)))
	}

	// A `.`-led literal can't follow an operand (`sqrt(4).5`), which must write its leading zero instead
	fn follows_operand(&self, literal_start: usize) -> bool {
		let mut preceding = self.input[..literal_start].trim_end();

		// A `!` run is postfix factorial only when an operand precedes it, otherwise it's a prefix logical not
		while let Some(rest) = preceding.strip_suffix('!') {
			preceding = rest.trim_end();
		}

		preceding
			.chars()
			.next_back()
			.is_some_and(|c| ends_operand(c) || (c == '|' && self.bar_at(preceding.len() - 1) == Some(Bar::Close)))
	}

	fn lex_number(&mut self) -> Option<Token<'a>> {
		let start_pos = self.pos;
		let (int_digits, int_value) = self.consume_digits();
		let mut got_digit = int_digits > 0;
		let mut plain_integer = true;

		if self.peek() == Some('.') {
			self.bump();
			plain_integer = false;
			got_digit |= self.consume_digits().0 > 0;
		}

		if got_digit && matches!(self.peek(), Some('e' | 'E')) {
			self.bump();
			plain_integer = false;
			if matches!(self.peek(), Some('+' | '-')) {
				self.bump();
			}
			if self.consume_digits().0 == 0 {
				self.pos = start_pos;
				return None;
			}
		}

		// A numeric literal cannot be glued directly to another by a stray decimal point or digit (e.g. `1..5`, `1.5.5`), so reject rather than letting it parse as implicit multiplication
		let leading_dot = self.input[start_pos..].starts_with('.');
		if !got_digit || self.peek().is_some_and(|c| c == '.' || c.is_ascii_digit()) || self.follows_number_literal(start_pos) || (leading_dot && self.follows_operand(start_pos)) {
			self.pos = start_pos;
			return None;
		}

		// A whole number is kept exact while it fits integer storage (18 digits always do), and the accumulation is exact up
		// to 15 digits; longer whole literals and fractional ones get std's correctly-rounded parsing
		let literal = &self.input[start_pos..self.pos];
		if plain_integer && int_digits <= 15 {
			return Some(Token::Integer(int_value as i64));
		}
		if plain_integer && let Ok(integer) = literal.parse::<i64>() {
			return Some(Token::Integer(integer));
		}

		// A literal spelled as a real, like `2.0` or `1e3`, still names a whole number, so it takes integer storage while it fits
		let float = literal.parse::<f64>().ok()?;
		Some(match Number::real_or_integer(float) {
			Number::Integer(integer) => Token::Integer(integer),
			_ => Token::Float(float),
		})
	}

	/// Consumes identifier continuation characters: Unicode's `XID_Continue`, which covers letters, digits,
	/// underscores, and the combining marks that complete a cluster like a decomposed `é`, plus a decimal point
	/// sandwiched between digits so that base-suffixed function names like `log3.25` lex as a single identifier.
	fn consume_identifier_body(&mut self, first: char) -> &'a str {
		let start = self.pos;
		let mut previous = first;
		while let Some(c) = self.peek() {
			let dot_between_digits = c == '.' && previous.is_ascii_digit() && self.input[self.pos + 1..].chars().next().is_some_and(|next| next.is_ascii_digit());
			// The middle dot and its Greek twin are identifier characters in Unicode, but they would pass for the `⋅` operator mid-name
			let middle_dot = matches!(c as u32, 0xB7 | 0x387);
			if !((unicode_ident::is_xid_continue(c) && !middle_dot) || dot_between_digits) {
				break;
			}
			previous = c;
			self.bump();
		}
		&self.input[start..self.pos]
	}

	fn skip_ws(&mut self) {
		self.consume_while(char::is_whitespace);
	}

	pub fn next_token(&mut self) -> Option<Token<'a>> {
		self.skip_ws();
		let start = self.pos;
		let ch = self.bump()?;

		use Token::*;
		let tok = match ch {
			'&' => {
				if self.peek() == Some('&') {
					self.bump();
					AndAnd
				} else {
					Error
				}
			}
			'|' => match self.bar_at(start) {
				Some(Bar::Or) => {
					self.bump();
					OrOr
				}
				Some(Bar::Open) => BarOpen,
				Some(Bar::Close) => BarClose,
				None => Error,
			},

			'(' => LParen,
			')' => RParen,
			'{' => LBrace,
			'}' => RBrace,
			',' => Comma,
			'+' => Plus,
			'-' => Minus,
			'*' => Star,
			'%' => Percent,
			'/' => Slash,
			'^' => Caret,
			'≠' => Neq,

			// A symbol can't be a name, so unlike `inf`, no binding can shadow `∞`
			'∞' => Float(f64::INFINITY),

			// Typeset math symbol aliases
			'−' => Minus,
			'×' | '⋅' => Star,
			'÷' => Slash,
			'∧' => AndAnd,
			'∨' => OrOr,
			// Its own token rather than a `Bang` alias, since `!` is also the postfix factorial and `5¬` is not one
			'¬' => Not,

			'!' => {
				if self.peek() == Some('=') {
					self.bump();
					Neq
				} else {
					Bang
				}
			}

			'≤' => Le,
			'<' => {
				if self.peek() == Some('=') {
					self.bump();
					Le
				} else {
					Lt
				}
			}

			'≥' => Ge,
			'>' => {
				if self.peek() == Some('=') {
					self.bump();
					Ge
				} else {
					Gt
				}
			}
			'=' => {
				if self.peek() == Some('=') {
					self.bump();
					EqEq
				} else {
					Error
				}
			}

			c if c.is_ascii_digit() || (c == '.' && self.peek().is_some_and(|c| c.is_ascii_digit())) => {
				self.pos = start;
				match self.lex_number() {
					Some(number) => number,
					// Consume the whole malformed numeric run so the error span covers it and lexing makes forward progress
					None => {
						self.pos = start;
						let mut prev = '\0';
						while let Some(c) = self.peek() {
							let part_of_number = c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' || ((c == '+' || c == '-') && matches!(prev, 'e' | 'E'));
							if !part_of_number {
								break;
							}
							prev = c;
							self.bump();
						}
						Error
					}
				}
			}

			_ => {
				let body = self.consume_identifier_body(ch);
				let ident = &self.input[start..self.pos];

				if let Some(keyword) = keyword(ident) {
					keyword
				} else if unicode_ident::is_xid_start(ch) {
					// A name is a Unicode identifier, as in Rust, so any script's letters may spell one
					Ident(ident)
				} else if ch == '\\' && body.chars().next().is_some_and(unicode_ident::is_xid_start) {
					// The `\` prefix names the language's own builtin, like `\pi`, and is never an identifier by itself
					Ident(ident)
				} else {
					// Digits, combining marks, invisible formatting characters, and symbols never begin a name, which also
					// leaves `#`, `$`, `~`, and `@` free to become namespace prefixes once a host scope needs them
					Error
				}
			}
		};

		Some(tok)
	}
}

impl<'a> Iterator for Lexer<'a> {
	type Item = Token<'a>;

	fn next(&mut self) -> Option<Self::Item> {
		self.next_token()
	}
}

/// Replaces each whole identifier for which `rename` returns a new spelling, so `b` never matches inside `logb`, and leaves all other source text untouched.
/// Returns `None` if the source fails to lex.
pub fn rename_identifiers(source: &str, mut rename: impl FnMut(&str) -> Option<String>) -> Option<String> {
	let mut lexer = Lexer::new(source);
	let mut result = String::with_capacity(source.len());
	let mut copied_up_to = 0;

	while let Some(token) = lexer.next_token() {
		match token {
			Token::Error => return None,
			Token::Ident(name) => {
				if let Some(new_name) = rename(name) {
					// An `Ident` always borrows directly from the source, so its span is recoverable by pointer offset
					let start = name.as_ptr() as usize - source.as_ptr() as usize;
					result.push_str(&source[copied_up_to..start]);
					result.push_str(&new_name);
					copied_up_to = start + name.len();
				}
			}
			_ => {}
		}
	}

	result.push_str(&source[copied_up_to..]);
	Some(result)
}

impl<'src> Input<'src> for Lexer<'src> {
	type Token = Token<'src>;
	type Span = Span;
	type Cursor = usize; // byte offset inside `input`
	type MaybeToken = Token<'src>;
	type Cache = Self;

	#[inline]
	fn begin(self) -> (Self::Cursor, Self::Cache) {
		(0, self)
	}

	#[inline]
	fn cursor_location(cursor: &Self::Cursor) -> usize {
		*cursor
	}

	#[inline]
	unsafe fn next_maybe(this: &mut Self::Cache, cursor: &mut Self::Cursor) -> Option<Self::MaybeToken> {
		this.pos = *cursor;
		if let Some(tok) = this.next_token() {
			*cursor = this.pos;
			Some(tok)
		} else {
			None
		}
	}

	#[inline]
	unsafe fn span(this: &mut Self::Cache, range: Range<&Self::Cursor>) -> Self::Span {
		// The cursor rests after the previous token, so the whitespace before the first token is left out
		let start = *range.end - this.input[*range.start..*range.end].trim_start().len();
		(start..*range.end).into()
	}
}

impl<'src> ValueInput<'src> for Lexer<'src> {
	#[inline]
	unsafe fn next(this: &mut Self::Cache, cursor: &mut Self::Cursor) -> Option<Self::Token> {
		this.pos = *cursor;
		if let Some(tok) = this.next_token() {
			*cursor = this.pos;
			Some(tok)
		} else {
			None
		}
	}
}
