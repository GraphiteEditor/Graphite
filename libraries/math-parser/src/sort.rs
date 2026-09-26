use crate::ast::{BinaryOp, Case, Literal, MatrixNode, Node, SortedCase, Syntax, UnaryOp, ValueNode};
use crate::constants::{Builtin, builtin_function};
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

/// Reads the sort of every subexpression from its spelling, so each evaluator takes only trees of its own sort.
pub fn sorted(syntax: Syntax) -> Result<Node, SortError> {
	Ok(match syntax {
		Syntax::Lit(literal) => Node::Value(ValueNode::Lit(literal)),
		Syntax::Var(name) if names_matrix(&name) => Node::Matrix(MatrixNode::Var(name)),
		Syntax::Var(name) => Node::Value(ValueNode::Var(name)),
		Syntax::FnCall { name, expr } => call(name, expr)?,
		Syntax::BinOp { lhs, op, rhs } => binary(sorted(*lhs)?, op, sorted(*rhs)?)?,
		Syntax::UnaryOp { expr, op } => match (sorted(*expr)?, op) {
			(Node::Value(_), UnaryOp::Transpose) => return Err(VALUE_AS_MATRIX),
			(Node::Value(expr), op) => Node::Value(ValueNode::UnaryOp { expr: Box::new(expr), op }),
			(Node::Matrix(expr), UnaryOp::Pos | UnaryOp::Neg | UnaryOp::Transpose) => Node::Matrix(MatrixNode::UnaryOp { expr: Box::new(expr), op }),
			(Node::Matrix(_), _) => return Err(NO_MATRIX_OPERATOR),
		},
		Syntax::Comparison { first, rest } => {
			let first = sorted(*first)?;
			let rest = rest.into_iter().map(|(op, operand)| Ok((op, sorted(operand)?))).collect::<Result<Vec<(BinaryOp, Node)>, SortError>>()?;
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
		Syntax::Piecewise { cases, otherwise } => piecewise(cases, otherwise)?,
		Syntax::Matrix { entries, by_rows } => Node::Matrix(MatrixNode::Literal {
			entries: entries.into_iter().map(|entry| value(sorted(entry)?)).collect::<Result<Vec<ValueNode>, SortError>>()?,
			by_rows,
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
fn call(name: String, arguments: Vec<Syntax>) -> Result<Node, SortError> {
	let arguments = arguments.into_iter().map(sorted).collect::<Result<Vec<Node>, SortError>>()?;
	let bare_name = name.strip_prefix('\\').unwrap_or(&name);

	// A matrix applied to one argument is implicit multiplication, so `M(v)` matches `M v`
	if names_matrix(bare_name) {
		return binary(Node::Matrix(MatrixNode::Var(name)), BinaryOp::Mul, one(arguments)?);
	}

	Ok(match builtin_function(bare_name) {
		Some(Builtin::OfMatrix(function)) => Node::Value(ValueNode::OfMatrix {
			function,
			matrix: Box::new(matrix(one(arguments)?)?),
		}),
		Some(Builtin::MatrixOfMatrix(function)) => Node::Matrix(MatrixNode::OfMatrix {
			function,
			matrix: Box::new(matrix(one(arguments)?)?),
		}),
		Some(Builtin::MatrixOfValues(function)) => Node::Matrix(MatrixNode::FromValues {
			function,
			arguments: values(arguments)?,
		}),
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
			value: Box::new(ValueNode::BinOp {
				lhs: Box::new(ValueNode::Lit(Literal::Integer(1))),
				op: Op::Div,
				rhs: Box::new(value),
			}),
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

/// A piecewise takes the sort of its cases, which must agree, under conditions that are values.
fn piecewise(cases: Vec<Case>, otherwise: Option<Box<Syntax>>) -> Result<Node, SortError> {
	let cases = cases
		.into_iter()
		.map(|Case { value: case, condition }| Ok((sorted(case)?, value(sorted(condition)?)?)))
		.collect::<Result<Vec<(Node, ValueNode)>, SortError>>()?;
	let otherwise = otherwise.map(|otherwise| sorted(*otherwise)).transpose()?;

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
