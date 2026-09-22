use crate::value::Complex;

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
	Float(f64),
	Complex(Complex),
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
	pub fn identity_element(self) -> Option<f64> {
		use BinaryOp as Op;
		match self {
			Op::Add | Op::Or => Some(0.),
			Op::Mul | Op::And => Some(1.),
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
}

#[derive(Debug, PartialEq)]
pub enum Node {
	Lit(Literal),
	Var(String),
	FnCall {
		name: String,
		expr: Vec<Node>,
	},
	BinOp {
		lhs: Box<Node>,
		op: BinaryOp,
		rhs: Box<Node>,
	},
	UnaryOp {
		expr: Box<Node>,
		op: UnaryOp,
	},
	/// A chain of two or more comparisons like `a < b < c`, each operator paired with the operand after it: one predicate over each adjacent pair, or over every pair for `!=`.
	Comparison {
		first: Box<Node>,
		rest: Vec<(BinaryOp, Node)>,
	},
	Conditional {
		condition: Box<Node>,
		if_block: Box<Node>,
		else_block: Box<Node>,
	},
}
