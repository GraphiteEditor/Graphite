use crate::ast::{BinaryOp, Case, Literal, MatrixNode, Node, SortedCase, Syntax, UnaryOp, ValueNode};
use crate::constants::{Builtin, builtin_function};
use crate::context::FunctionProvider;
use crate::lexer::names_matrix;
use std::fmt;

/// A subexpression standing where its sort cannot, which fails the parse, since every sort is fixed by spelling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SortError(&'static str);

impl fmt::Display for SortError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.0)
	}
}

const MATRIX_AS_VALUE: SortError = SortError("A matrix stands where a value is needed");
const VALUE_AS_MATRIX: SortError = SortError("A value stands where a matrix is needed");
const NO_MATRIX_OPERATOR: SortError = SortError("The operator has no meaning for a matrix");
const MIXED_CASES: SortError = SortError("A piecewise's cases must all be values or all be matrices");
const INVALID_ARGUMENTS: SortError = SortError("Invalid arguments for function call");

/// Reads the sort of every subexpression from its spelling, so each evaluator takes only trees of its own sort. A call of a
/// function the host provides is a value, whatever builtin shares its name.
pub fn sorted(syntax: Syntax, functions: &dyn FunctionProvider) -> Result<Node, SortError> {
	Ok(match syntax {
		Syntax::Lit(literal) => Node::Value(ValueNode::Lit(literal)),
		Syntax::Var(name) if names_matrix(&name) => Node::Matrix(MatrixNode::Var(name)),
		Syntax::Var(name) => Node::Value(ValueNode::Var(name)),
		Syntax::FnCall { name, expr } => call(name, expr, functions)?,
		Syntax::BinOp { lhs, op, rhs } => binary(sorted(*lhs, functions)?, op, sorted(*rhs, functions)?)?,
		Syntax::Product { first, rest } => product(sorted(*first, functions)?, &mut rest.into_iter(), functions)?,
		Syntax::UnaryOp { expr, op } => match (sorted(*expr, functions)?, op) {
			(Node::Value(_), UnaryOp::Transpose) => return Err(VALUE_AS_MATRIX),
			(Node::Value(expr), op) => Node::Value(ValueNode::UnaryOp { expr: Box::new(expr), op }),
			(Node::Matrix(expr), UnaryOp::Pos | UnaryOp::Neg | UnaryOp::Transpose) => Node::Matrix(MatrixNode::UnaryOp { expr: Box::new(expr), op }),
			(Node::Matrix(_), _) => return Err(NO_MATRIX_OPERATOR),
		},
		Syntax::Comparison { first, rest } => {
			let first = sorted(*first, functions)?;
			let rest = rest
				.into_iter()
				.map(|(op, operand)| Ok((op, sorted(operand, functions)?)))
				.collect::<Result<Vec<(BinaryOp, Node)>, SortError>>()?;
			match first {
				Node::Value(first) => {
					let rest = rest.into_iter().map(|(op, operand)| Ok((op, value(operand)?))).collect::<Result<Vec<_>, SortError>>()?;
					Node::Value(ValueNode::Comparison { first: Box::new(first), rest })
				}
				// Matrices have no order, so a chain over them is `==` or `!=` throughout
				Node::Matrix(first) => {
					let distinct = rest.iter().all(|(op, _)| *op == BinaryOp::Neq);
					if !distinct && !rest.iter().all(|(op, _)| *op == BinaryOp::Eq) {
						return Err(NO_MATRIX_OPERATOR);
					}
					let rest = rest.into_iter().map(|(_, operand)| matrix(operand).map_err(|_| NO_MATRIX_OPERATOR));
					let matrices = std::iter::once(Ok(first)).chain(rest).collect::<Result<Vec<MatrixNode>, SortError>>()?;
					Node::Value(ValueNode::MatrixComparison { matrices, distinct })
				}
			}
		}
		Syntax::Piecewise { cases, otherwise } => piecewise(cases, otherwise, functions)?,
		Syntax::Matrix { entries, by_rows } => Node::Matrix(MatrixNode::Literal {
			entries: entries.into_iter().map(|entry| value(sorted(entry, functions)?)).collect::<Result<Vec<ValueNode>, SortError>>()?,
			by_rows,
		}),
		Syntax::Range { from, to } => Node::Matrix(MatrixNode::Range {
			from: Box::new(value(sorted(*from, functions)?)?),
			to: Box::new(value(sorted(*to, functions)?)?),
		}),
	})
}

fn value(node: Node) -> Result<ValueNode, SortError> {
	match node {
		Node::Value(value) => Ok(value),
		Node::Matrix(_) => Err(MATRIX_AS_VALUE),
	}
}

fn matrix(node: Node) -> Result<MatrixNode, SortError> {
	match node {
		Node::Matrix(matrix) => Ok(matrix),
		Node::Value(_) => Err(VALUE_AS_MATRIX),
	}
}

fn values(nodes: Vec<Node>) -> Result<Vec<ValueNode>, SortError> {
	nodes.into_iter().map(value).collect()
}

fn one(arguments: Vec<Node>) -> Result<Node, SortError> {
	<[Node; 1]>::try_from(arguments).map(|[argument]| argument).map_err(|_| INVALID_ARGUMENTS)
}

/// A call's sort follows the builtin's, a matrix's name applying it to its one argument, and any other name taking values alone.
/// A range function like `inside(p, R)` takes a value and then matrices.
fn call(name: String, arguments: Vec<Syntax>, functions: &dyn FunctionProvider) -> Result<Node, SortError> {
	let arguments = arguments.into_iter().map(|argument| sorted(argument, functions)).collect::<Result<Vec<Node>, SortError>>()?;
	let (prefixed, bare_name) = match name.strip_prefix('\\') {
		Some(bare_name) => (true, bare_name),
		None => (false, name.as_str()),
	};

	// A matrix applied to one argument is implicit multiplication, so `M(v)` matches `M v`
	if names_matrix(bare_name) {
		return binary(Node::Matrix(MatrixNode::Var(name)), BinaryOp::Mul, one(arguments)?);
	}

	// A host function shadows the builtin of its spelling unless the `\` prefix asks for the language's own
	let builtin = if !prefixed && functions.provides(bare_name) { None } else { builtin_function(bare_name) };

	Ok(match builtin {
		Some(Builtin::OfMatrix(function)) => Node::Value(ValueNode::OfMatrix {
			function,
			matrix: Box::new(matrix(one(arguments)?)?),
		}),
		Some(Builtin::MatrixOfMatrix(function)) => Node::Matrix(MatrixNode::OfMatrix {
			function,
			matrix: Box::new(matrix(one(arguments)?)?),
		}),
		Some(Builtin::MatrixOfValues { function, arity }) => {
			if !arity.contains(&arguments.len()) {
				return Err(INVALID_ARGUMENTS);
			}
			Node::Matrix(MatrixNode::FromValues {
				function,
				arguments: values(arguments)?,
			})
		}
		Some(Builtin::OfValueAndRegions { function, regions }) => {
			if arguments.len() != regions + 1 {
				return Err(INVALID_ARGUMENTS);
			}
			let mut arguments = arguments.into_iter();
			Node::Value(ValueNode::OfValueAndRegions {
				function,
				value: Box::new(value(arguments.next().ok_or(INVALID_ARGUMENTS)?)?),
				regions: arguments.map(matrix).collect::<Result<Vec<MatrixNode>, SortError>>()?,
			})
		}
		_ => Node::Value(ValueNode::FnCall { name, expr: values(arguments)? }),
	})
}

/// The sort of an operation: value·value and matrix·value are values, matrix·matrix and value·matrix are matrices, and
/// `+` joins like sorts or attaches a value to a matrix as translation.
fn binary(lhs: Node, op: BinaryOp, rhs: Node) -> Result<Node, SortError> {
	use BinaryOp as Op;
	Ok(match (lhs, op, rhs) {
		(Node::Value(lhs), op, Node::Value(rhs)) => Node::Value(ValueNode::BinOp {
			lhs: Box::new(lhs),
			op,
			rhs: Box::new(rhs),
		}),
		(Node::Matrix(matrix), Op::Mul, Node::Value(value)) => Node::Value(ValueNode::Apply {
			matrix: Box::new(matrix),
			value: Box::new(value),
		}),
		// Division is times-inverse, so a matrix over a value applies to the value's reciprocal
		(Node::Matrix(matrix), Op::Div, Node::Value(value)) => Node::Value(ValueNode::Apply {
			matrix: Box::new(matrix),
			value: Box::new(reciprocal(value)),
		}),
		(Node::Matrix(lhs), Op::Eq | Op::Neq, Node::Matrix(rhs)) => Node::Value(ValueNode::MatrixComparison {
			matrices: vec![lhs, rhs],
			distinct: op == Op::Neq,
		}),
		(lhs @ Node::Value(_), Op::Mul | Op::Div | Op::Add | Op::Sub, rhs @ Node::Matrix(_))
		| (lhs @ Node::Matrix(_), Op::Mul | Op::Div | Op::Add | Op::Sub, rhs @ Node::Matrix(_))
		| (lhs @ Node::Matrix(_), Op::Add | Op::Sub | Op::Pow, rhs @ Node::Value(_)) => Node::Matrix(MatrixNode::BinOp {
			lhs: Box::new(lhs),
			op,
			rhs: Box::new(rhs),
		}),
		_ => return Err(NO_MATRIX_OPERATOR),
	})
}

fn reciprocal(value: ValueNode) -> ValueNode {
	ValueNode::BinOp {
		lhs: Box::new(ValueNode::Lit(Literal::Integer(1))),
		op: BinaryOp::Div,
		rhs: Box::new(value),
	}
}

/// A product folds left, except that a matrix meeting a value applies to the whole rest of the product, so `M 2 v` is `M (2 v)`
/// as in linear algebra rather than `(M 2) v`.
fn product(first: Node, rest: &mut std::vec::IntoIter<(BinaryOp, Syntax)>, functions: &dyn FunctionProvider) -> Result<Node, SortError> {
	let mut accumulated = first;

	while let Some((op, factor)) = rest.next() {
		accumulated = match (accumulated, sorted(factor, functions)?) {
			(Node::Matrix(matrix), Node::Value(factor)) => {
				// Dividing by a value applies the matrix to its reciprocal times the rest, so `M / v w` is `M (v⁻¹ w)`
				let factor = if op == BinaryOp::Div { reciprocal(factor) } else { factor };

				let argument = product(Node::Value(factor), rest, functions)?;
				return binary(Node::Matrix(matrix), BinaryOp::Mul, argument);
			}
			(accumulated, factor) => binary(accumulated, op, factor)?,
		};
	}

	Ok(accumulated)
}

/// A piecewise takes the sort of its cases, which must agree, under conditions that are values.
fn piecewise(cases: Vec<Case>, otherwise: Option<Box<Syntax>>, functions: &dyn FunctionProvider) -> Result<Node, SortError> {
	let cases = cases
		.into_iter()
		.map(|Case { value: case, condition }| Ok((sorted(case, functions)?, value(sorted(condition, functions)?)?)))
		.collect::<Result<Vec<(Node, ValueNode)>, SortError>>()?;
	let otherwise = otherwise.map(|otherwise| sorted(*otherwise, functions)).transpose()?;

	let first = cases.first().map(|(case, _)| case).or(otherwise.as_ref());
	if first.is_some_and(|first| matches!(first, Node::Matrix(_))) {
		let cases = cases
			.into_iter()
			.map(|(case, condition)| {
				Ok(SortedCase {
					value: matrix(case).map_err(|_| MIXED_CASES)?,
					condition,
				})
			})
			.collect::<Result<Vec<_>, SortError>>()?;
		let otherwise = otherwise.map(|otherwise| matrix(otherwise).map_err(|_| MIXED_CASES)).transpose()?.map(Box::new);
		return Ok(Node::Matrix(MatrixNode::Piecewise { cases, otherwise }));
	}

	let cases = cases
		.into_iter()
		.map(|(case, condition)| {
			Ok(SortedCase {
				value: value(case).map_err(|_| MIXED_CASES)?,
				condition,
			})
		})
		.collect::<Result<Vec<_>, SortError>>()?;
	let otherwise = otherwise.map(|otherwise| value(otherwise).map_err(|_| MIXED_CASES)).transpose()?.map(Box::new);
	Ok(Node::Value(ValueNode::Piecewise { cases, otherwise }))
}
