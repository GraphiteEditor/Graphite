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
	/// Logical AND (nonzero treated as true, returns 1. or 0.)
	And,
	Div,
	/// Logical OR (nonzero treated as true, returns 1. or 0.)
	Or,
	Modulo,
	Pow,
	Leq,
	Lt,
	Geq,
	Gt,
	Neq,
	Eq,
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
	FnCall { name: String, expr: Vec<Node> },
	BinOp { lhs: Box<Node>, op: BinaryOp, rhs: Box<Node> },
	UnaryOp { expr: Box<Node>, op: UnaryOp },
	Conditional { condition: Box<Node>, if_block: Box<Node>, else_block: Box<Node> },
}
