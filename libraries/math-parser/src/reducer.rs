use crate::ast::BinaryOp;
use crate::constants::{Builtin, BuiltinFunction, builtin_function};
use crate::context::{NothingMap, ValueProvider};
use crate::executer::settle_matrix;
use crate::lexer::{Lexer, Token};
use crate::matrix::Matrix;
use crate::object::Object;
use crate::parser::ParseError;
use crate::value::{Number, Value};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

/// A variadic function's action across a matrix list, which `mean` and `count` alone have.
pub type MatrixFunction = fn(&[Matrix]) -> Option<Object>;

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
	Function { function: BuiltinFunction, over_matrices: Option<MatrixFunction> },
}

/// The action of a variadic function across matrices: the mean is pointwise and the count counts, while the others need an
/// order or a product of values.
fn matrix_function(name: &str) -> Option<MatrixFunction> {
	match name {
		"mean" => Some(|matrices| settle_matrix(Matrix::mean(matrices)?).ok().map(Object::from)),
		"count" => Some(|matrices| Some(Object::from(Value::from_i64(matrices.len() as i64)))),
		_ => None,
	}
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
			match builtin_function(bare_name)? {
				Builtin::Values { function, variadic: true } => Reducer::Function {
					function,
					over_matrices: matrix_function(bare_name),
				},
				_ => return None,
			}
		}
		_ => return None,
	})
}

/// Why an input string isn't a lone reducer token, for a host input taking only one, or `None` when it is one or is blank.
pub fn reducer_error(source: &str) -> Option<ParseError> {
	if source.trim().is_empty() || classify_reducer(source, NothingMap).is_some() {
		return None;
	}

	// The whole source is wrong rather than any part of it, so the error spans all of it but the whitespace around it
	let start = source.len() - source.trim_start().len();
	Some(ParseError::lone_reducer_expected(start..source.trim_end().len()))
}

impl Reducer {
	/// Evaluates this reducer across the given items, or `None` for an ill-formed application, like a NaN item, a fold of an
	/// empty list under an operator with no identity element, or an indeterminate result, since no operation produces NaN.
	pub fn evaluate(&self, items: &[Value]) -> Option<Value> {
		if items.iter().any(|Value::Number(number)| number.is_nan()) {
			return None;
		}
		settle(self.evaluate_unsettled(items)?)
	}

	/// Evaluates this reducer across each leading run of the items, the first item alone, then the first two, and so on, giving for
	/// each run exactly what [`Self::evaluate`] gives for it. Once a fold step fails, every longer run fails with it.
	pub fn evaluate_running(&self, items: &[Value]) -> Vec<Option<Value>> {
		// A NaN item makes every run containing it ill-formed
		let valid_count = items.iter().position(|Value::Number(number)| number.is_nan()).unwrap_or(items.len());
		let valid_items = &items[..valid_count];

		// Items take canonical form first, as in `evaluate_unsettled`
		let numbers = valid_items.iter().map(|Value::Number(number)| number.canonical());
		let mut results: Vec<Option<Value>> = match self {
			// O(n) by folding each item into the previous run's result
			Reducer::FoldLeft(op) => {
				let mut accumulated: Option<Number> = None;
				numbers
					.enumerate()
					.map(|(index, item)| {
						accumulated = match index {
							// A lone item meets no operator, so logic checks it is a truth value here
							0 if matches!(op, BinaryOp::And | BinaryOp::Or) => item.as_bool().map(|_| item),
							0 => Some(item),
							_ => accumulated.and_then(|accumulated| accumulated.binary_op(*op, item)),
						};
						accumulated.and_then(|accumulated| settle(Value::Number(accumulated)))
					})
					.collect()
			}

			// O(n) by checking only the newest adjacent pair against the previous run's result
			Reducer::ChainAdjacent(op) => {
				let mut satisfied = Some(true);
				let mut previous = None;
				numbers
					.map(|number| {
						if let Some(previous) = previous {
							satisfied = satisfied.and_then(|satisfied| adjacent_holds(*op, previous, number).map(|holds| satisfied && holds));
						}
						previous = Some(number);
						satisfied.and_then(|satisfied| settle(Value::from_bool(satisfied)))
					})
					.collect()
			}

			// O(n) by keeping the set of items seen so far
			Reducer::ChainDistinct => {
				let mut seen = HashSet::new();
				let mut distinct = true;
				numbers
					.map(|number| {
						distinct &= seen.insert(CanonicalBits::of(number));
						settle(Value::from_bool(distinct))
					})
					.collect()
			}

			// O(n²) since `^` groups from the right and a variadic builtin reads its whole list, so no run builds on a shorter one's result
			Reducer::FoldRight(_) | Reducer::Function { .. } => (1..=valid_count).map(|count| self.evaluate(&valid_items[..count])).collect(),
		};

		results.resize(items.len(), None);
		results
	}

	fn evaluate_unsettled(&self, items: &[Value]) -> Option<Value> {
		// Items take canonical form first, so a fold reads a whole real as the integer its written-out expression would
		let numbers = items.iter().map(|Value::Number(number)| number.canonical());
		match self {
			Reducer::FoldLeft(op) => {
				let mut iter = numbers;
				let Some(first) = iter.next() else { return op.identity_element() };

				// A lone item meets no operator, so logic checks it is a truth value here
				if matches!(op, BinaryOp::And | BinaryOp::Or) {
					first.as_bool()?;
				}
				iter.try_fold(first, |accumulated, item| accumulated.binary_op(*op, item)).map(Value::Number)
			}

			Reducer::FoldRight(op) => {
				let mut iter = numbers.rev();
				let first = iter.next()?;
				iter.try_fold(first, |accumulated, item| item.binary_op(*op, accumulated)).map(Value::Number)
			}

			Reducer::ChainAdjacent(op) => chain_holds(*op, numbers).map(Value::from_bool),

			// Canonical form stores each value one way (and NaN is already rejected), so equal items share a key and hashing finds a repeat in O(n)
			Reducer::ChainDistinct => {
				let mut seen = HashSet::new();
				let distinct = numbers.map(CanonicalBits::of).all(|key| seen.insert(key));
				Some(Value::from_bool(distinct))
			}

			// Builtins read any complex or quaternion storage as having a vector part, which a host's item may lack
			Reducer::Function { function, .. } if items.iter().any(|Value::Number(number)| matches!(number, Number::Complex(_) | Number::Quaternion(_))) => {
				function(&numbers.map(Value::Number).collect::<Vec<_>>())
			}
			Reducer::Function { function, .. } => function(items),
		}
	}

	/// Evaluates this reducer across matrices: `*` and `/` compose in list order, `+`, `-`, and `mean` are pointwise, and `count`
	/// counts. Any other reducer is `None`, like a NaN item or a fold of an empty list under `-` or `/`.
	pub fn evaluate_matrices(&self, items: &[Matrix]) -> Option<Object> {
		if items.iter().any(|matrix| matrix.is_nan()) {
			return None;
		}

		match self {
			Reducer::FoldLeft(op) => {
				let combine = matrix_fold_step(*op)?;

				let mut iter = items.iter().copied();
				let Some(first) = iter.next() else {
					return match op {
						BinaryOp::Add => Some(Object::from(Matrix::ZERO)),
						BinaryOp::Mul => Some(Object::from(Matrix::IDENTITY)),
						_ => None,
					};
				};
				let combined = iter.try_fold(first, |accumulated, matrix| settle_matrix(combine(accumulated, matrix)?).ok())?;
				Some(Object::from(combined))
			}
			Reducer::Function { over_matrices, .. } => over_matrices.and_then(|function| function(items)),
			Reducer::FoldRight(_) | Reducer::ChainAdjacent(_) | Reducer::ChainDistinct => None,
		}
	}

	/// Evaluates this reducer across each leading run of the matrices, the first alone, then the first two, and so on, giving for
	/// each run exactly what [`Self::evaluate_matrices`] gives for it. Under `*`, each result is where that many steps end up.
	pub fn evaluate_running_matrices(&self, items: &[Matrix]) -> Vec<Option<Object>> {
		// A NaN item makes every run containing it ill-formed
		let valid_count = items.iter().position(|matrix| matrix.is_nan()).unwrap_or(items.len());
		let valid_items = &items[..valid_count];

		let mut results: Vec<Option<Object>> = match self {
			// O(n) by folding each matrix into the previous run's result
			Reducer::FoldLeft(op) => match matrix_fold_step(*op) {
				Some(combine) => {
					let mut accumulated: Option<Matrix> = None;
					valid_items
						.iter()
						.enumerate()
						.map(|(index, &matrix)| {
							accumulated = match index {
								0 => Some(matrix),
								_ => accumulated.and_then(|accumulated| settle_matrix(combine(accumulated, matrix)?).ok()),
							};
							accumulated.map(Object::from)
						})
						.collect()
				}
				None => Vec::new(),
			},

			// O(n²) since a matrix function is an opaque function of its whole list
			Reducer::Function { over_matrices, .. } => (1..=valid_count).map(|count| over_matrices.and_then(|function| function(&valid_items[..count]))).collect(),

			Reducer::FoldRight(_) | Reducer::ChainAdjacent(_) | Reducer::ChainDistinct => Vec::new(),
		};

		results.resize(items.len(), None);
		results
	}
}

/// A reducer's result in canonical form, or `None` for an indeterminate result, since no operation produces NaN.
fn settle(Value::Number(result): Value) -> Option<Value> {
	(!result.is_nan()).then(|| Value::Number(result.canonical()))
}

/// How a fold combines its running matrix with the next one: `*` and `/` compose in list order, while `+` and `-` are pointwise.
/// Any other operator has no action across matrices.
fn matrix_fold_step(op: BinaryOp) -> Option<fn(Matrix, Matrix) -> Option<Matrix>> {
	Some(match op {
		BinaryOp::Add => |a, b| Some(a + b),
		BinaryOp::Sub => |a, b| Some(a - b),
		BinaryOp::Mul => |a, b| Some(a.compose(b)),
		BinaryOp::Div => |a, b| Some(a.compose(b.inverse()?)),
		_ => return None,
	})
}

/// Whether every adjacent pair of items satisfies the chain's relation: true over zero or one items, since no pair exists to fail
/// it, and `None` when any item has no order, since every pair is checked. Its own function keeps the loop's frame small.
#[inline(never)]
fn chain_holds(op: BinaryOp, numbers: impl Iterator<Item = Number>) -> Option<bool> {
	let mut satisfied = true;
	let mut previous = None;
	for number in numbers {
		if let Some(previous) = previous {
			satisfied &= adjacent_holds(op, previous, number)?;
		}
		previous = Some(number);
	}
	Some(satisfied)
}

/// Whether two adjacent items of a comparison chain satisfy its relation, or `None` when one has a vector part and so no order.
fn adjacent_holds(op: BinaryOp, lhs: Number, rhs: Number) -> Option<bool> {
	if op == BinaryOp::Eq {
		return Some(lhs == rhs);
	}

	let ordering = lhs.real_ordering(rhs)?;
	Some(match op {
		BinaryOp::Lt => ordering == Ordering::Less,
		BinaryOp::Leq => ordering != Ordering::Greater,
		BinaryOp::Gt => ordering == Ordering::Greater,
		BinaryOp::Geq => ordering != Ordering::Less,
		_ => return None,
	})
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
	fn logic_checks_a_lone_item_is_a_truth_value() {
		assert_eq!(run("&&", &[1.]), Some(1.));
		assert_eq!(run("||", &[0.]), Some(0.));
		assert_eq!(run("&&", &[5.]), None);
		assert_eq!(run("||", &[0.5]), None);
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
	fn orderings_reject_vector_items() {
		// A vector has no order, so an ordered chain over one is ill-formed rather than false, while equality compares any items
		let reduce = |source: &str, items: &[Value]| classify_reducer(source, NothingMap).unwrap().evaluate(items);
		let vectors = [Value::from(Quaternion::J), Value::from(Quaternion::K)];
		for source in ["<", "<=", ">", ">="] {
			assert_eq!(reduce(source, &vectors), None, "`{source}`");
			assert_eq!(reduce(source, &[Value::from_f64(1.), Value::from(Quaternion::J), Value::from_f64(3.)]), None, "`{source}`");
		}
		assert_eq!(reduce("==", &vectors), Some(Value::from_bool(false)));
		assert_eq!(reduce("==", &[Value::from(Quaternion::J), Value::from(Quaternion::J)]), Some(Value::from_bool(true)));
	}

	#[test]
	fn nan_items_are_rejected() {
		// `min` and `count` would otherwise drop or ignore the NaN and return a number
		for source in ["min", "count", "+", "<"] {
			assert_eq!(run(source, &[f64::NAN, 1.]), None, "`{source}`");
		}
	}

	#[test]
	fn matrix_lists_compose_sum_average_and_count() {
		let reduce = |source: &str, items: &[Matrix]| classify_reducer(source, NothingMap).unwrap().evaluate_matrices(items);
		let scale = Matrix::scale(Quaternion::new(2., 0., 0., 0.));
		let shift = Matrix::IDENTITY.translated(Quaternion::I);

		// Products compose in list order, so the shift lands before the scale doubles it
		assert_eq!(reduce("*", &[scale, shift]).unwrap().into_matrix().unwrap().translation, Quaternion::new(0., 2., 0., 0.));
		assert_eq!(reduce("*", &[shift, scale]).unwrap().into_matrix().unwrap().translation, Quaternion::I);
		assert_eq!(reduce("/", &[scale, scale]).unwrap(), Object::from(Matrix { axes: scale.axes, ..Matrix::IDENTITY }));

		// Sums and the mean are pointwise, and the count is a value
		assert_eq!(reduce("+", &[scale, scale]).unwrap().into_matrix().unwrap().rows[1], Quaternion::new(0., 4., 0., 0.));
		assert_eq!(reduce("-", &[scale, scale]).unwrap().into_matrix().unwrap().rows[1], Quaternion::ZERO);
		assert_eq!(reduce("mean", &[scale, Matrix::IDENTITY]).unwrap().into_matrix().unwrap().rows[1], Quaternion::new(0., 1.5, 0., 0.));
		let huge = Matrix::scale(Quaternion::new(1e308, 0., 0., 0.));
		assert_eq!(reduce("mean", &[huge, huge]).unwrap(), Object::from(huge));
		assert_eq!(reduce("count", &[scale, shift]), Some(Object::from(Value::from_i64(2))));

		// An empty list yields the identity element where there is one, and a singular divisor has no inverse
		assert_eq!(reduce("*", &[]), Some(Object::from(Matrix::IDENTITY)));
		assert_eq!(reduce("+", &[]), Some(Object::from(Matrix::ZERO)));
		assert_eq!(reduce("count", &[]), Some(Object::from(Value::from_i64(0))));
		for source in ["-", "/", "mean"] {
			assert_eq!(reduce(source, &[]), None, "`{source}`");
		}
		assert_eq!(reduce("/", &[scale, Matrix::ZERO]), None);

		// Matrices have no order, power, extremum, or truth, even alone, and a NaN item is rejected
		for source in ["<", "==", "!=", "^", "&&", "min", "max", "median", "xor"] {
			assert_eq!(reduce(source, &[scale, shift]), None, "`{source}`");
		}
		for source in ["&&", "||"] {
			assert_eq!(reduce(source, &[scale]), None, "`{source}`");
		}
		assert_eq!(reduce("+", &[scale, Matrix::IDENTITY.translated(Quaternion::splat(f64::NAN))]), None);
	}

	#[test]
	fn running_results_match_each_leading_run() {
		// Between them, the lists hold repeats, descents, a zero divisor, a NaN, and items that aren't truth values
		let lists: [&[f64]; 6] = [&[3., 1., 4., 1., 5.], &[2., 0., 3.], &[1., 1., 0., 1.], &[0.5, 2., -1., 2.], &[1., f64::NAN, 2.], &[]];
		for source in ["+", "-", "*", "/", "&&", "||", "^", "<", "<=", "==", "!=", "min", "max", "mean", "count"] {
			let reducer = classify_reducer(source, NothingMap).unwrap();
			for list in lists {
				let items: Vec<Value> = list.iter().map(|&item| Value::from_f64(item)).collect();
				let expected: Vec<Option<Value>> = (1..=items.len()).map(|count| reducer.evaluate(&items[..count])).collect();
				assert_eq!(reducer.evaluate_running(&items), expected, "`{source}` over {list:?}");
			}
		}
	}

	#[test]
	fn running_matrix_results_match_each_leading_run() {
		let scale = Matrix::scale(Quaternion::new(2., 0., 0., 0.));
		let shift = Matrix::IDENTITY.translated(Quaternion::I);
		let nan = Matrix::IDENTITY.translated(Quaternion::splat(f64::NAN));

		// Between them, the lists hold a singular divisor and a NaN
		let lists: [&[Matrix]; 4] = [&[scale, shift, scale, shift], &[shift, Matrix::ZERO, scale], &[scale, nan, shift], &[]];
		for source in ["*", "/", "+", "-", "mean", "count", "<", "min"] {
			let reducer = classify_reducer(source, NothingMap).unwrap();
			for (list_index, list) in lists.into_iter().enumerate() {
				let expected: Vec<Option<Object>> = (1..=list.len()).map(|count| reducer.evaluate_matrices(&list[..count])).collect();
				assert_eq!(reducer.evaluate_running_matrices(list), expected, "`{source}` over list {list_index}");
			}
		}
	}
}
