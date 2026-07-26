use crate::ast::Literal;
use chumsky::input::{Input, ValueInput};
use chumsky::span::SimpleSpan;
use num_complex::Complex64;
use std::fmt;
use std::ops::Range;

pub type Span = SimpleSpan;

#[derive(Clone, Debug, PartialEq)]
pub enum Token<'src> {
	Float(f64),
	Const(Constant),
	Ident(&'src str),

	AndAnd,
	OrOr,
	Bang,

	LParen,
	RParen,
	Comma,
	Plus,
	Minus,
	Modulo,
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

	/// An unrecognized character; the parser never matches this, forcing a parse error rather than silently truncating the input.
	Error,
}

impl<'src> fmt::Display for Token<'src> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Token::Float(x) => write!(f, "{x}"),
			Token::Const(c) => write!(f, "{c}"),
			Token::Ident(name) => write!(f, "{name}"),

			Token::AndAnd => f.write_str("&&"),
			Token::OrOr => f.write_str("||"),
			Token::Bang => f.write_str("!"),

			Token::LParen => f.write_str("("),
			Token::RParen => f.write_str(")"),
			Token::Comma => f.write_str(","),
			Token::Plus => f.write_str("+"),
			Token::Minus => f.write_str("-"),
			Token::Modulo => f.write_str("%"),
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
	G,
}

impl Constant {
	pub fn value(self) -> Literal {
		use Constant::*;
		use std::f64::consts;
		match self {
			Pi => Literal::Float(consts::PI),
			Tau => Literal::Float(consts::TAU),
			E => Literal::Float(consts::E),
			Phi => Literal::Float(1.618_033_988_75),
			Inf => Literal::Float(f64::INFINITY),
			I => Literal::Complex(Complex64::new(0., 1.)),
			G => Literal::Float(9.80665),
		}
	}

	pub fn from_name(name: &str) -> Option<Constant> {
		use Constant::*;
		Some(match name {
			"pi" | "π" => Pi,
			"tau" | "τ" => Tau,
			"e" => E,
			"phi" | "φ" => Phi,
			"inf" | "∞" => Inf,
			"i" => I,
			"G" => G,
			_ => return None,
		})
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
			G => "G",
		})
	}
}

pub struct Lexer<'a> {
	input: &'a str,
	pos: usize,
}

impl<'a> Lexer<'a> {
	pub fn new(input: &'a str) -> Self {
		Self { input, pos: 0 }
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

	// A numeric literal cannot follow another operand across whitespace (`10 000`, `sqrt(4).5`), only constants/calls/parens may juxtapose
	fn juxtaposes_with_preceding_operand(&self, literal_start: usize) -> bool {
		let mut preceding = self.input[..literal_start].trim_end();

		// A `!` run is postfix factorial only when an operand precedes it, otherwise it's a prefix logical not
		while let Some(rest) = preceding.strip_suffix('!') {
			preceding = rest.trim_end();
		}

		preceding.chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '.' || c == ')' || c == '∞')
	}

	fn lex_number(&mut self) -> Option<f64> {
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
		if !got_digit || self.peek().is_some_and(|c| c == '.' || c.is_ascii_digit()) || self.juxtaposes_with_preceding_operand(start_pos) {
			self.pos = start_pos;
			return None;
		}

		// Accumulation is exact up to 15 digits; longer or fractional literals get std's correctly-rounded parsing
		if plain_integer && int_digits <= 15 {
			return Some(int_value);
		}
		self.input[start_pos..self.pos].parse::<f64>().ok()
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
			'|' => {
				if self.peek() == Some('|') {
					self.bump();
					OrOr
				} else {
					Error
				}
			}

			'(' => LParen,
			')' => RParen,
			',' => Comma,
			'+' => Plus,
			'-' => Minus,
			'*' => Star,
			'%' => Modulo,
			'/' => Slash,
			'^' => Caret,
			'≠' => Neq,

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
					Some(number) => Float(number),
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
				self.consume_while(|c| c.is_alphanumeric() || c == '_');
				let ident = &self.input[start..self.pos];

				if ident == "if" {
					If
				} else if let Some(lit) = Constant::from_name(ident) {
					Const(lit)
				} else if ch.is_alphanumeric() {
					Ident(ident)
				} else {
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
	unsafe fn span(_this: &mut Self::Cache, range: Range<&Self::Cursor>) -> Self::Span {
		(*range.start..*range.end).into()
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
