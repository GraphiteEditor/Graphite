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
#[derive(Debug, Clone, PartialEq)]
pub enum Syntax<'src> {
	Lit(Literal),
	Var(&'src str),
	FnCall {
		name: &'src str,
		expr: Vec<Syntax<'src>>,
	},
	BinOp {
		lhs: Box<Syntax<'src>>,
		op: BinaryOp,
		rhs: Box<Syntax<'src>>,
	},
	UnaryOp {
		expr: Box<Syntax<'src>>,
		op: UnaryOp,
	},
	/// A chain of two or more comparisons like `a < b < c`, each operator paired with the operand after it: one predicate over each adjacent pair, or over every pair for `!=`.
	Comparison {
		first: Box<Syntax<'src>>,
		rest: Vec<(BinaryOp, Syntax<'src>)>,
	},
	/// A chain of three or more factors joined by `*`, `/`, or juxtaposition, each operator paired with the factor after it, left flat for
	/// the sort pass to group, since a matrix applies to every factor after it.
	Product {
		first: Box<Syntax<'src>>,
		rest: Vec<(BinaryOp, Syntax<'src>)>,
	},
	/// The cases of math's `cases` notation, `{a if cond, b otherwise}`: disjoint conditions in no meaningful order, with `otherwise` holding when none of them do.
	Piecewise {
		cases: Vec<Case<'src>>,
		otherwise: Option<Box<Syntax<'src>>>,
	},
	/// A matrix literal of up to four whole values: rows like `[a;b]`, or columns like `[a,b]`, the images of the basis directions.
	Matrix {
		entries: Vec<Syntax<'src>>,
		by_rows: bool,
	},
	/// The range `a..b`, the map sending parameter `0` to `a` and `1` to `b`, which vector corners make a box.
	Range {
		from: Box<Syntax<'src>>,
		to: Box<Syntax<'src>>,
	},
	/// An expression with the names its `where` clause defines, like `a + f(2) where a = 1, f(t) = t^2`.
	Where {
		body: Box<Syntax<'src>>,
		bindings: Vec<Binding<'src>>,
	},
	/// A call whose parentheses end with a `where` clause, boxed so a tree's every node stays small.
	CallWhere(Box<CallWhere<'src>>),
}

/// A call whose parentheses end with a `where` clause, like `max(a, b where a = 1)`, whose names every argument may read but
/// the function's name, outside the parentheses, can't.
#[derive(Debug, Clone, PartialEq)]
pub struct CallWhere<'src> {
	pub name: &'src str,
	pub arguments: Vec<Syntax<'src>>,
	pub bindings: Vec<Binding<'src>>,
}

/// One case of a piecewise, the value it takes where its condition holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Case<'src> {
	pub value: Syntax<'src>,
	pub condition: Syntax<'src>,
}

/// One definition in a `where` clause: a value like `a = 1`, or a function like `f(t) = t^2`.
#[derive(Debug, Clone, PartialEq)]
pub struct Binding<'src> {
	pub name: &'src str,
	/// The function's parameters, of which a value has none.
	pub parameters: Vec<&'src str>,
	pub value: Syntax<'src>,
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
	/// A function of a value and regions with a value result, like `within(p, R)`.
	OfValueAndRegions {
		function: ValueOfRegions,
		value: Box<ValueNode>,
		regions: Vec<MatrixNode>,
		/// The value after the regions, where the builtin takes one, like `smoothstep`'s continuity.
		trailing: Option<Box<ValueNode>>,
	},
	/// A chain of `==`, or of `!=`, over matrices, pointwise.
	MatrixComparison {
		matrices: Vec<MatrixNode>,
		distinct: bool,
	},
	/// A value a `where` clause defines, or a parameter of the function being called.
	Local(Local),
	/// A call of a function a `where` clause defines.
	Call {
		function: Local,
		arguments: Vec<Node>,
	},
	/// An expression within the names its `where` clause defines.
	Where {
		clause: Box<Clause>,
		body: Box<ValueNode>,
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
	/// A matrix a `where` clause defines, or a parameter of the function being called.
	Local(Local),
	/// A call of a function a `where` clause defines.
	Call {
		function: Local,
		arguments: Vec<Node>,
	},
	/// An expression within the names its `where` clause defines.
	Where {
		clause: Box<Clause>,
		body: Box<MatrixNode>,
	},
}

/// One case of a sorted piecewise, whose condition is a value whatever the sort of its cases.
#[derive(Debug)]
pub struct SortedCase<T> {
	pub value: T,
	pub condition: ValueNode,
}

/// Where a name a `where` clause or a function's parameters define lives: its scope, counted outward from the innermost, and
/// its position among that scope's values or functions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Local {
	pub depth: usize,
	pub index: usize,
}

/// A sorted `where` clause: each value's definition, and each function's body over its parameters.
#[derive(Debug)]
pub struct Clause {
	pub values: Vec<Node>,
	pub functions: Vec<Node>,
}
