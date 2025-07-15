use crate::ast::Literal;
use chumsky::input::{Input, ValueInput};
use chumsky::prelude::*;
use chumsky::span::SimpleSpan;
use chumsky::text::{ident, int};
use core::f64;
use num_complex::Complex64;
use std::fmt;
use std::iter::Peekable;
use std::ops::Range;
use std::str::Chars;

pub type Span = SimpleSpan;

#[derive(Clone, Debug, PartialEq)]
pub enum Token<'src> {
	Float(f64),
	Const(Constant),
	Ident(&'src str),

	LParen,
	RParen,
	Comma,
	Plus,
	Minus,
	Star,
	Slash,
	Caret,

	Lt,
	Le,
	Gt,
	Ge,
	EqEq,

	If,
}

impl<'src> fmt::Display for Token<'src> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Token::Float(x) => write!(f, "{x}"),
			Token::Const(c) => write!(f, "{c}"),
			Token::Ident(name) => write!(f, "{name}"),

			Token::LParen => f.write_str("("),
			Token::RParen => f.write_str(")"),
			Token::Comma => f.write_str(","),
			Token::Plus => f.write_str("+"),
			Token::Minus => f.write_str("-"),
			Token::Star => f.write_str("*"),
			Token::Slash => f.write_str("/"),
			Token::Caret => f.write_str("^"),

			Token::Lt => f.write_str("<"),
			Token::Le => f.write_str("<="),
			Token::Gt => f.write_str(">"),
			Token::Ge => f.write_str(">="),
			Token::EqEq => f.write_str("=="),

			Token::If => f.write_str("if"),
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
			I => Literal::Complex(Complex64::new(0.0, 1.0)),
			G => Literal::Float(9.80665),
		}
	}

	pub fn from_str(name: &str) -> Option<Constant> {
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

	fn lex_ident(&mut self) -> &'a str {
		self.consume_while(|c| c.is_alphanumeric() || c == '_')
	}

	fn lex_uint(&mut self) -> Option<(u64, usize)> {
		let mut v = 0u64;
		let mut digits = 0;
		while let Some(d) = self.peek().and_then(|c| c.to_digit(10)) {
			v = v * 10 + d as u64;
			digits += 1;
			self.bump();
		}
		(digits > 0).then_some((v, digits))
	}

	fn lex_number(&mut self) -> Option<f64> {
		let start_pos = self.pos;
		let (int_val, int_digits) = self.lex_uint().unwrap_or((0, 0));
		let mut got_digit = int_digits > 0;
		let mut num = int_val as f64;

		if self.peek() == Some('.') {
			self.bump();
			if let Some((frac_val, frac_digits)) = self.lex_uint() {
				num += (frac_val as f64) / 10f64.powi(frac_digits as i32);
				got_digit = true;
			}
		}

		if matches!(self.peek(), Some('e' | 'E')) {
			self.bump();
			let sign = match self.peek() {
				Some('+') => {
					self.bump();
					1
				}
				Some('-') => {
					self.bump();
					-1
				}
				_ => 1,
			};
			if let Some((exp_val, _)) = self.lex_uint() {
				num *= 10f64.powi(sign * exp_val as i32);
			} else {
				self.pos = start_pos;
				return None;
			}
		}

		got_digit.then_some(num)
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
			'(' => LParen,
			')' => RParen,
			',' => Comma,
			'+' => Plus,
			'-' => Minus,
			'*' => Star,
			'/' => Slash,
			'^' => Caret,

			'<' => {
				if self.peek() == Some('=') {
					self.bump();
					Le
				} else {
					Lt
				}
			}
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
					return None;
				}
			}

			c if c.is_ascii_digit() || (c == '.' && self.peek().is_some_and(|c| c.is_ascii_digit())) => {
				self.pos = start;
				Float(self.lex_number()?)
			}

			_ => {
				self.consume_while(|c| c.is_alphanumeric() || c == '_');
				let ident = &self.input[start..self.pos];

				if ident == "if" {
					If
				} else if let Some(lit) = Constant::from_str(ident) {
					Const(lit)
				} else if ch.is_alphanumeric() {
					Ident(ident)
				} else {
					return None;
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
