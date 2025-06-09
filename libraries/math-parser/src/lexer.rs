// ── lexer.rs ───────────────────────────────────────────────────────────
use crate::ast::Literal;
use chumsky::input::{Input, ValueInput};
use chumsky::prelude::*;
use chumsky::span::SimpleSpan;
use chumsky::text::{ident, int};
use num_complex::Complex64;
use std::ops::Range;

pub type Span = SimpleSpan;

#[derive(Clone, Debug, PartialEq)]
pub enum Token<'src> {
	// literals ----------------------------------------------------------------
	Const(Literal), // numeric or complex constants recognised at lex‑time
	Var(&'src str), //  #identifier  (variables)
	Call(&'src str),
	// punctuation -------------------------------------------------------------
	LParen,
	RParen,
	Comma,
	Plus,
	Minus,
	Star,
	Slash,
	Caret,
	// comparison --------------------------------------------------------------
	Lt,
	Le,
	Gt,
	Ge,
	EqEq,
	// keywords ----------------------------------------------------------------
	If,
}

pub fn lexer<'src>() -> impl Parser<'src, &'src str, Vec<(Token<'src>, Span)>, extra::Err<Rich<'src, char>>> {
	// ── numbers ────────────────────────────────────────────────────────────
	let num = int(10)
		.then(just('.').then(int(10)).or_not())
		.then(just('e').or(just('E')).then(one_of("+-").or_not()).then(int(10)).or_not())
		.map(|((int_part, frac), exp): ((&str, _), _)| {
			let mut s = int_part.to_string();
			if let Some((_, frac)) = frac {
				s.push('.');
				s.push_str(frac);
			}
			if let Some(((e, sign), exp)) = exp {
				s.push(e);
				if let Some(sign) = sign {
					s.push(sign);
				}
				s.push_str(exp);
			}
			Token::Const(Literal::Float(s.parse::<f64>().unwrap()))
		});

	// ── single‑char symbols ────────────────────────────────────────────────
	let sym = choice((
		just('(').to(Token::LParen),
		just(')').to(Token::RParen),
		just(',').to(Token::Comma),
		just('+').to(Token::Plus),
		just('-').to(Token::Minus),
		just('*').to(Token::Star),
		just('/').to(Token::Slash),
		just('^').to(Token::Caret),
	));

	// ── comparison operators ───────────────────────────────────────────────
	let cmp = choice((
		just("<=").to(Token::Le),
		just(">=").to(Token::Ge),
		just("==").to(Token::EqEq),
		just('<').to(Token::Lt),
		just('>').to(Token::Gt),
	));

	let kw_token = |w, t| just(w).padded().to(t);

	let kw_lit = |w, lit: Literal| just(w).padded().to(lit);

	let const_token = choice((
		kw_lit("pi", Literal::Float(std::f64::consts::PI)),
		kw_lit("π", Literal::Float(std::f64::consts::PI)),
		kw_lit("tau", Literal::Float(std::f64::consts::TAU)),
		kw_lit("τ", Literal::Float(std::f64::consts::TAU)),
		kw_lit("e", Literal::Float(std::f64::consts::E)),
		kw_lit("phi", Literal::Float(1.618_033_988_75)),
		kw_lit("φ", Literal::Float(1.618_033_988_75)),
		kw_lit("inf", Literal::Float(f64::INFINITY)),
		kw_lit("∞", Literal::Float(f64::INFINITY)),
		kw_lit("i", Literal::Complex(Complex64::new(0.0, 1.0))),
		kw_lit("G", Literal::Float(9.80665)),
	))
	.map(Token::Const);

	let var_token = just('#').ignore_then(ident()).map(Token::Var);
	let call_token = just('@').ignore_then(ident()).map(Token::Call);

	choice((num, kw_token("if", Token::If), const_token, cmp, sym, var_token, call_token))
		.map_with(|t, e| (t, e.span()))
		.padded()
		.repeated()
		.collect()
}

#[derive(Debug)]
pub struct TokenStream<'src> {
	tokens: Vec<(Token<'src>, Span)>,
}

impl<'src> TokenStream<'src> {
	pub fn new(tokens: Vec<(Token<'src>, Span)>) -> Self {
		TokenStream { tokens }
	}
}

impl<'src> Input<'src> for TokenStream<'src> {
	type Token = (Token<'src>, Span);
	type Span = Span;
	type Cursor = usize;
	type MaybeToken = (Token<'src>, Span);
	type Cache = Self;

	fn begin(self) -> (Self::Cursor, Self::Cache) {
		(0, self)
	}

	fn cursor_location(cursor: &Self::Cursor) -> usize {
		*cursor
	}

	#[inline(always)]
	unsafe fn next_maybe(this: &mut Self::Cache, cursor: &mut Self::Cursor) -> Option<Self::MaybeToken> {
		if let Some(tok) = this.tokens.get(*cursor) {
			*cursor += 1;
			Some(tok.clone())
		} else {
			None
		}
	}

	#[inline(always)]
	unsafe fn span(_this: &mut Self::Cache, range: Range<&Self::Cursor>) -> Self::Span {
		(*range.start..*range.end).into()
	}
}

impl<'src> ValueInput<'src> for TokenStream<'src> {
	unsafe fn next(this: &mut Self::Cache, cursor: &mut Self::Cursor) -> Option<Self::Token> {
		if let Some(tok) = this.tokens.get(*cursor) {
			*cursor += 1;
			Some(tok.clone())
		} else {
			None
		}
	}
}
