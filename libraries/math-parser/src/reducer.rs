use crate::ast::BinaryOp;
use crate::constants::{BuiltinFunction, builtin_function};
use crate::context::ValueProvider;
use crate::lexer::{Lexer, Token};
use crate::value::{Number, Value};
use std::collections::HashSet;

/// How a lone reducer token combines the items it is applied across.
#[derive(Clone, Copy)]
pub enum Reducer {
	/// Left-associative pairwise accumulation, like `((a + b) + c)`.
	FoldLeft(BinaryOp),
	/// Right-associative pairwise accumulation, like `a ^ (b ^ c)`.
	FoldRight(BinaryOp),
	/// A single n-ary predicate asserting the relation between every adjacent pair, like `a < b < c`.
	ChainAdjacent(BinaryOp),
	/// A single n-ary predicate asserting that every pair of items is distinct, like `a != b != c`.
	ChainDistinct,
	/// A single call of a variadic function over all items, like `min(a, b, c)`.
	Function(BuiltinFunction),
}

/// Classifies an input string as a lone reducer token, or `None` when it should instead be parsed as a full expression.
/// A lone token expands to the expression written out over a whole item list: operators interleave and functions wrap.
pub fn classify_reducer(source: &str, bindings: impl ValueProvider) -> Option<Reducer> {
	let mut lexer = Lexer::new(source);
	let token = lexer.next_token()?;
	if lexer.next_token().is_some() {
		return None;
	}

	Some(match token {
		Token::Plus => Reducer::FoldLeft(BinaryOp::Add),
		Token::Minus => Reducer::FoldLeft(BinaryOp::Sub),
		Token::Star => Reducer::FoldLeft(BinaryOp::Mul),
		Token::Slash => Reducer::FoldLeft(BinaryOp::Div),
		Token::AndAnd => Reducer::FoldLeft(BinaryOp::And),
		Token::OrOr => Reducer::FoldLeft(BinaryOp::Or),
		Token::Caret => Reducer::FoldRight(BinaryOp::Pow),
		Token::Lt => Reducer::ChainAdjacent(BinaryOp::Lt),
		Token::Le => Reducer::ChainAdjacent(BinaryOp::Leq),
		Token::Gt => Reducer::ChainAdjacent(BinaryOp::Gt),
		Token::Ge => Reducer::ChainAdjacent(BinaryOp::Geq),
		Token::EqEq => Reducer::ChainAdjacent(BinaryOp::Eq),
		Token::Neq => Reducer::ChainDistinct,
		Token::Ident(name) => {
			// A binding shadows the function of exactly its spelling, which the `\` prefix still reaches
			let bare_name = match name.strip_prefix('\\') {
				Some(bare_name) => bare_name,
				None if bindings.get_value(name).is_some() => return None,
				None => name,
			};
			Reducer::Function(builtin_function(bare_name).filter(|builtin| builtin.variadic)?.function)
		}
		_ => return None,
	})
}

impl Reducer {
	/// Evaluates this reducer across the given items, or `None` for an ill-formed application, like a NaN item, a fold of an
	/// empty list under an operator with no identity element, or an indeterminate result, since no operation produces NaN.
	pub fn evaluate(&self, items: &[f64]) -> Option<f64> {
		if items.iter().any(|item| item.is_nan()) {
			return None;
		}
		self.evaluate_unsettled(items).filter(|result| !result.is_nan())
	}

	fn evaluate_unsettled(&self, items: &[f64]) -> Option<f64> {
		match self {
			Reducer::FoldLeft(op) => {
				let mut iter = items.iter();
				let Some(&first) = iter.next() else { return op.identity_element() };
				let folded = iter.try_fold(Number::Real(first), |accumulated, &item| accumulated.binary_op(*op, Number::Real(item)));
				folded?.as_real()
			}

			Reducer::FoldRight(op) => {
				let mut iter = items.iter().rev();
				let &first = iter.next()?;
				let folded = iter.try_fold(Number::Real(first), |accumulated, &item| Number::Real(item).binary_op(*op, accumulated));
				folded?.as_real()
			}

			// A chain over zero or one items is true, since no pair exists to fail the relation
			Reducer::ChainAdjacent(op) => {
				let satisfied = items.windows(2).all(|pair| Number::Real(pair[0]).binary_op(*op, Number::Real(pair[1])) == Some(Number::Real(1.)));
				Some(if satisfied { 1. } else { 0. })
			}

			// Unifying -0 with 0 (NaN is already rejected) gives equal items equal bits, so hashing finds a repeat in O(n)
			Reducer::ChainDistinct => {
				let mut seen = HashSet::new();
				let distinct = items.iter().all(|&item| seen.insert(if item == 0. { 0 } else { item.to_bits() }));
				Some(if distinct { 1. } else { 0. })
			}

			Reducer::Function(function) => {
				let values: Vec<Value> = items.iter().map(|&item| Value::from_f64(item)).collect();
				function(&values)?.as_real()
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ast;
	use crate::context::{EvalContext, NothingMap, ValueMap};
	use std::collections::HashMap;

	fn run(source: &str, items: &[f64]) -> Option<f64> {
		classify_reducer(source, NothingMap).and_then(|reducer| reducer.evaluate(items))
	}

	#[test]
	fn lone_tokens_classify_and_expressions_do_not() {
		for source in ["+", " * ", "^", "<", "!=", "min", "\\min", "xor"] {
			assert!(classify_reducer(source, NothingMap).is_some(), "expected `{source}` to classify as a reducer");
		}

		// Anything beyond one token, and any fixed-arity function name, is an expression instead
		for source in ["a + b", "*5", "min(a, b)", "log", "atan2", "sqrt", "\\sqrt", "mod", "%", "", "  ", "x"] {
			assert!(classify_reducer(source, NothingMap).is_none(), "expected `{source}` to classify as an expression");
		}
	}

	#[test]
	fn bindings_shadow_reducer_function_names() {
		// A lone token reduces across the items if it classifies, and otherwise evaluates as an expression over the bindings
		let evaluate = |source: &str, items: &[f64], bindings: &ValueMap| match classify_reducer(source, bindings) {
			Some(reducer) => reducer.evaluate(items),
			None => ast::Node::try_parse_from_str(source).ok()?.eval(&EvalContext::new(bindings, NothingMap)).ok()?.as_real(),
		};

		let items = [5., 2., 8.];
		let unbound = ValueMap::default();
		let min_bound = ValueMap(HashMap::from([("min".to_string(), Value::from_f64(42.))]));

		assert_eq!(evaluate("min", &items, &unbound), Some(2.));
		assert_eq!(evaluate("\\min", &items, &unbound), Some(2.));
		assert_eq!(evaluate("min", &items, &min_bound), Some(42.));
		assert_eq!(evaluate("\\min", &items, &min_bound), Some(2.));
	}

	#[test]
	fn folds_accumulate_pairwise() {
		assert_eq!(run("+", &[1., 2., 3.]), Some(6.));
		assert_eq!(run("-", &[10., 3., 2.]), Some(5.));
		assert_eq!(run("*", &[2., 3., 4.]), Some(24.));
		assert_eq!(run("^", &[2., 2., 3.]), Some(256.));
		assert_eq!(run("-", &[7.]), Some(7.));
	}

	#[test]
	fn empty_lists_use_identity_elements() {
		assert_eq!(run("+", &[]), Some(0.));
		assert_eq!(run("*", &[]), Some(1.));
		assert_eq!(run("&&", &[]), Some(1.));
		assert_eq!(run("||", &[]), Some(0.));
		assert_eq!(run("-", &[]), None);
		assert_eq!(run("^", &[]), None);
	}

	#[test]
	fn chains_are_single_predicates() {
		assert_eq!(run("<", &[1., 2., 3.]), Some(1.));
		assert_eq!(run("<", &[3., 5., 2.]), Some(0.));
		assert_eq!(run("<=", &[1., 1., 2.]), Some(1.));
		assert_eq!(run("==", &[2., 2., 2.]), Some(1.));
		assert_eq!(run("!=", &[1., 2., 1.]), Some(0.));
		assert_eq!(run("!=", &[1., 2., 3.]), Some(1.));
		assert_eq!(run("!=", &[0., -0.]), Some(0.));
		assert_eq!(run("<", &[5.]), Some(1.));
		assert_eq!(run("!=", &[]), Some(1.));
	}

	#[test]
	fn distinctness_scales_to_large_lists() {
		let mut items: Vec<f64> = (0..1_000_000).map(f64::from).collect();
		assert_eq!(run("!=", &items), Some(1.));

		items.push(0.);
		assert_eq!(run("!=", &items), Some(0.));
	}

	#[test]
	fn function_tokens_apply_variadically() {
		assert_eq!(run("min", &[5., 2., 8.]), Some(2.));
		assert_eq!(run("\\min", &[5., 2., 8.]), Some(2.));
		assert_eq!(run("mean", &[1., 2., 3., 6.]), Some(3.));
		assert_eq!(run("count", &[1., 2., 3.]), Some(3.));
		assert_eq!(run("rms", &[3., 4.]), Some(12.5_f64.sqrt()));
		assert_eq!(run("mode", &[1., 2., 2.]), Some(2.));
		assert_eq!(run("xor", &[1., 1., 1.]), Some(1.));
		assert_eq!(run("min", &[]), None);
		assert_eq!(run("count", &[]), Some(0.));
	}

	#[test]
	fn nan_items_are_rejected() {
		// `min` and `count` would otherwise drop or ignore the NaN and return a number
		for source in ["min", "count", "+", "<"] {
			assert_eq!(run(source, &[f64::NAN, 1.]), None, "`{source}`");
		}
	}
}
