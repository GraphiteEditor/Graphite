use crate::ast::BinaryOp;
use crate::constants::{BuiltinFunction, builtin_function};
use crate::context::ValueProvider;
use crate::lexer::{Lexer, Token};
use crate::value::{Number, Value};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

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
	pub fn evaluate(&self, items: &[Value]) -> Option<Value> {
		if items.iter().any(|Value::Number(number)| number.is_nan()) {
			return None;
		}
		let Value::Number(result) = self.evaluate_unsettled(items)?;
		(!result.is_nan()).then(|| Value::Number(result.canonical()))
	}

	fn evaluate_unsettled(&self, items: &[Value]) -> Option<Value> {
		// Items take canonical form first, so a fold reads a whole real as the integer its written-out expression would
		let numbers = items.iter().map(|Value::Number(number)| number.canonical());
		match self {
			Reducer::FoldLeft(op) => {
				let mut iter = numbers;
				let Some(first) = iter.next() else { return op.identity_element() };
				iter.try_fold(first, |accumulated, item| accumulated.binary_op(*op, item)).map(Value::Number)
			}

			Reducer::FoldRight(op) => {
				let mut iter = numbers.rev();
				let first = iter.next()?;
				iter.try_fold(first, |accumulated, item| item.binary_op(*op, accumulated)).map(Value::Number)
			}

			// A chain over zero or one items is true, since no pair exists to fail the relation
			Reducer::ChainAdjacent(op) => {
				let satisfied = numbers.clone().zip(numbers.skip(1)).all(|(lhs, rhs)| lhs.binary_op(*op, rhs).and_then(Number::as_bool) == Some(true));
				Some(Value::from_bool(satisfied))
			}

			// Canonical form stores each value one way (and NaN is already rejected), so equal items share a key and hashing finds a repeat in O(n)
			Reducer::ChainDistinct => {
				let mut seen = HashSet::new();
				let distinct = numbers.map(CanonicalBits::of).all(|key| seen.insert(key));
				Some(Value::from_bool(distinct))
			}

			// Builtins read any complex or quaternion storage as having a vector part, which a host's item may lack
			Reducer::Function(function) if items.iter().any(|Value::Number(number)| matches!(number, Number::Complex(_) | Number::Quaternion(_))) => {
				function(&numbers.map(Value::Number).collect::<Vec<_>>())
			}
			Reducer::Function(function) => function(items),
		}
	}
}

/// A canonical number's storage as hashable bits, which equal numbers share.
#[derive(PartialEq, Eq)]
enum CanonicalBits {
	Integer(i64),
	Real(u64),
	Complex(u64, u64),
	Quaternion([u64; 4]),
}

// Hashes the bits alone, leaving equality to tell the variants apart, which spares hashing the variant for every item
impl Hash for CanonicalBits {
	fn hash<H: Hasher>(&self, state: &mut H) {
		match *self {
			CanonicalBits::Integer(integer) => state.write_i64(integer),
			CanonicalBits::Real(bits) => state.write_u64(bits),
			CanonicalBits::Complex(real_bits, imaginary_bits) => {
				state.write_u64(real_bits);
				state.write_u64(imaginary_bits);
			}
			CanonicalBits::Quaternion(parts) => {
				for part in parts {
					state.write_u64(part);
				}
			}
		}
	}
}

impl CanonicalBits {
	fn of(canonical: Number) -> Self {
		match canonical {
			Number::Integer(integer) => Self::Integer(integer),
			Number::Real(real) => Self::Real(real.to_bits()),
			Number::Complex(complex) => Self::Complex(complex.re.to_bits(), complex.im.to_bits()),
			Number::Quaternion(quaternion) => Self::Quaternion(quaternion.parts().map(f64::to_bits)),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ast;
	use crate::context::{EvalContext, NothingMap, ValueMap};
	use crate::quaternion::Quaternion;
	use crate::value::Complex;
	use std::collections::HashMap;

	fn run(source: &str, items: &[f64]) -> Option<f64> {
		let items: Vec<Value> = items.iter().map(|&item| Value::from_f64(item)).collect();
		classify_reducer(source, NothingMap).and_then(|reducer| reducer.evaluate(&items)).and_then(|value| value.as_real())
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
		let evaluate = |source: &str, items: &[Value], bindings: &ValueMap| match classify_reducer(source, bindings) {
			Some(reducer) => reducer.evaluate(items)?.as_real(),
			None => ast::Node::try_parse_from_str(source).ok()?.eval(&EvalContext::new(bindings, NothingMap)).ok()?.as_real(),
		};

		let items = [5., 2., 8.].map(Value::from_f64);
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
	fn integer_items_fold_exactly() {
		// Integer items accumulate in integer storage, so a sum beyond the reals' 2^53 limit stays exact
		let items = [Value::from_i64(1 << 53), Value::from_i64(1)];
		let sum = classify_reducer("+", NothingMap).unwrap().evaluate(&items).unwrap();
		assert_eq!(sum.as_i64(), Some((1 << 53) + 1));

		// Whole real items do the same, since the written-out expression would read them as integers
		let items = [Value::from_f64((1_i64 << 53) as f64), Value::from_f64(1.)];
		let sum = classify_reducer("+", NothingMap).unwrap().evaluate(&items).unwrap();
		assert_eq!(sum.as_i64(), Some((1 << 53) + 1));
	}

	#[test]
	fn distinctness_compares_values_rather_than_storage() {
		let distinct = |items: &[Value]| classify_reducer("!=", NothingMap).unwrap().evaluate(items).and_then(|value| value.as_bool());

		assert_eq!(distinct(&[Value::from_i64(2), Value::from_f64(2.)]), Some(false));
		assert_eq!(distinct(&[Value::from_f64(3.), Value::from(Complex::new(3., 0.))]), Some(false));
		assert_eq!(distinct(&[Value::from(Complex::new(1., 2.)), Value::from(Quaternion::new(1., 2., 0., -0.))]), Some(false));
		assert_eq!(distinct(&[Value::from(Quaternion::J), Value::from(Quaternion::K)]), Some(true));
		assert_eq!(distinct(&[Value::from_i64(1 << 53), Value::from_i64((1 << 53) + 1)]), Some(true));
	}

	#[test]
	fn items_stored_with_zero_vector_parts_read_as_reals() {
		let items = [Value::from(Quaternion::new(3., 0., 0., 0.)), Value::from(Complex::new(-2., 0.))];
		let reduce = |source: &str| classify_reducer(source, NothingMap).unwrap().evaluate(&items);

		assert_eq!(reduce("min"), Some(Value::from_i64(-2)));
		assert_eq!(reduce(">"), Some(Value::from_bool(true)));
	}

	#[test]
	fn nan_items_are_rejected() {
		// `min` and `count` would otherwise drop or ignore the NaN and return a number
		for source in ["min", "count", "+", "<"] {
			assert_eq!(run(source, &[f64::NAN, 1.]), None, "`{source}`");
		}
	}
}
