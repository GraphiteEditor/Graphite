use crate::constants::{MatrixToMatrix, MatrixToValue, ValueOfRegions, ValuesToMatrix};
use crate::value::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
	/// A whole-number literal, kept exact so integer arithmetic on it stays exact beyond the reals' 2^53 limit.
	Integer(i64),
	Float(f64),
}

impl From<f64> for Literal {
	fn from(value: f64) -> Self {
		Self::Float(value)
	}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOp {
	Add,
	Sub,
	Mul,
	/// Logical AND over operands that must each be exactly 0 or 1, returning 0 or 1.
	And,
	Div,
	/// Logical OR over operands that must each be exactly 0 or 1, returning 0 or 1.
	Or,
	Pow,
	Leq,
	Lt,
	Geq,
	Gt,
	Neq,
	Eq,
}

impl BinaryOp {
	/// The operand that leaves the other unchanged, like 0 for `+`, which is also what a fold of no items yields.
	pub fn identity_element(self) -> Option<Value> {
		use BinaryOp as Op;
		match self {
			Op::Add => Some(Value::from_i64(0)),
			Op::Mul => Some(Value::from_i64(1)),
			Op::And => Some(Value::from_bool(true)),
			Op::Or => Some(Value::from_bool(false)),
			Op::Sub | Op::Div | Op::Pow | Op::Leq | Op::Lt | Op::Geq | Op::Gt | Op::Neq | Op::Eq => None,
		}
	}

	/// Whether a chain of comparisons reads in one direction: `<`/`<=`/`==` ascending, `>`/`>=`/`==` descending, or `!=` alone.
	pub fn chain_in_one_direction(ops: &[BinaryOp]) -> bool {
		let ascending = ops.iter().all(|op| matches!(op, BinaryOp::Lt | BinaryOp::Leq | BinaryOp::Eq));
		let descending = ops.iter().all(|op| matches!(op, BinaryOp::Gt | BinaryOp::Geq | BinaryOp::Eq));
		let distinct = ops.iter().all(|op| matches!(op, BinaryOp::Neq));
		ascending || descending || distinct
	}
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum UnaryOp {
	Pos,
	Neg,
	Fac,
	Not,
	/// The magnitude bars `|x|`: absolute value on the reals, extending to the Euclidean magnitude.
	Magnitude,
	/// The postfix `A^T`, swapping a linear matrix's rows and columns.
	Transpose,
}

/// The tree as written, before each subexpression's sort is read from its spelling.
#[derive(Debug, PartialEq)]
pub enum Syntax {
	Lit(Literal),
	Var(String),
	FnCall {
		name: String,
		expr: Vec<Syntax>,
	},
	BinOp {
		lhs: Box<Syntax>,
		op: BinaryOp,
		rhs: Box<Syntax>,
	},
	UnaryOp {
		expr: Box<Syntax>,
		op: UnaryOp,
	},
	/// A chain of two or more comparisons like `a < b < c`, each operator paired with the operand after it: one predicate over each adjacent pair, or over every pair for `!=`.
	Comparison {
		first: Box<Syntax>,
		rest: Vec<(BinaryOp, Syntax)>,
	},
	/// The cases of math's `cases` notation, `{a if cond, b otherwise}`: disjoint conditions in no meaningful order, with `otherwise` holding when none of them do.
	Piecewise {
		cases: Vec<Case>,
		otherwise: Option<Box<Syntax>>,
	},
	/// A matrix literal of up to four whole values: rows like `[a;b]`, or columns like `[a,b]`, the images of the basis directions.
	Matrix {
		entries: Vec<Syntax>,
		by_rows: bool,
	},
	/// The range `a..b`, the map sending parameter `0` to `a` and `1` to `b`, which vector corners make a box.
	Range {
		from: Box<Syntax>,
		to: Box<Syntax>,
	},
}

/// One case of a piecewise, the value it takes where its condition holds.
#[derive(Debug, PartialEq)]
pub struct Case {
	pub value: Syntax,
	pub condition: Syntax,
}

/// A parsed expression, whose every subexpression has the sort its spelling fixes: a value or a matrix.
#[derive(Debug)]
pub enum Node {
	Value(ValueNode),
	Matrix(MatrixNode),
}

/// A subexpression evaluating to a value.
#[derive(Debug)]
pub enum ValueNode {
	Lit(Literal),
	Var(String),
	FnCall {
		name: String,
		expr: Vec<ValueNode>,
	},
	BinOp {
		lhs: Box<ValueNode>,
		op: BinaryOp,
		rhs: Box<ValueNode>,
	},
	UnaryOp {
		expr: Box<ValueNode>,
		op: UnaryOp,
	},
	Comparison {
		first: Box<ValueNode>,
		rest: Vec<(BinaryOp, ValueNode)>,
	},
	Piecewise {
		cases: Vec<SortedCase<ValueNode>>,
		otherwise: Option<Box<ValueNode>>,
	},
	/// A matrix applied to a value, `M v`.
	Apply {
		matrix: Box<MatrixNode>,
		value: Box<ValueNode>,
	},
	/// A function of a matrix with a value result, like `det(A)`.
	OfMatrix {
		function: MatrixToValue,
		matrix: Box<MatrixNode>,
	},
	/// A function of a value and regions with a value result, like `inside(p, R)`.
	OfMatrices {
		function: ValueOfRegions,
		value: Box<ValueNode>,
		matrices: Vec<MatrixNode>,
	},
	/// A chain of `==`, or of `!=`, over matrices, pointwise.
	MatrixComparison {
		matrices: Vec<MatrixNode>,
		distinct: bool,
	},
}

/// A subexpression evaluating to a matrix.
#[derive(Debug)]
pub enum MatrixNode {
	Var(String),
	Literal {
		entries: Vec<ValueNode>,
		by_rows: bool,
	},
	/// A function building a matrix from values, like `rotation(angle)`.
	FromValues {
		function: ValuesToMatrix,
		arguments: Vec<ValueNode>,
	},
	/// The range `a..b`.
	Range {
		from: Box<ValueNode>,
		to: Box<ValueNode>,
	},
	/// A function of a matrix with a matrix result, like `linear(A)`.
	OfMatrix {
		function: MatrixToMatrix,
		matrix: Box<MatrixNode>,
	},
	/// An operation with a matrix result: composition, a value acting on a matrix, sums, translation attached, division, or a power.
	BinOp {
		lhs: Box<Node>,
		op: BinaryOp,
		rhs: Box<Node>,
	},
	/// Negation, the identity `+`, or the transpose.
	UnaryOp {
		expr: Box<MatrixNode>,
		op: UnaryOp,
	},
	Piecewise {
		cases: Vec<SortedCase<MatrixNode>>,
		otherwise: Option<Box<MatrixNode>>,
	},
}

/// One case of a sorted piecewise, whose condition is a value whatever the sort of its cases.
#[derive(Debug)]
pub struct SortedCase<T> {
	pub value: T,
	pub condition: ValueNode,
}
