#[derive(Debug, PartialEq, Eq)]
pub struct Unit {
	// Exponent of length unit (meters)
	pub length: i32,
	// Exponent of mass unit (kilograms)
	pub mass: i32,
	// Exponent of time unit (seconds)
	pub time: i32,
}

impl Unit {
	pub const BASE_UNIT: Unit = Unit { length: 0, mass: 0, time: 0 };

	pub fn base_unit() -> Self {
		Self::BASE_UNIT
	}

	pub fn is_base(&self) -> bool {
		*self == Self::BASE_UNIT
	}
}

#[derive(Debug, PartialEq)]
pub enum Literal {
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
	Div,
	Pow,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum UnaryOp {
	Neg,
	Sqrt,
	Sin,
	Cos,
	Tan,
	Csc,
	Sec,
	Cot,
	InvSin,
	InvCos,
	InvTan,
	InvCsc,
	InvSec,
	InvCot,
	Fac,
}

#[derive(Debug, PartialEq)]
pub enum Node {
	Lit(Literal),
	Var(String),
	FnCall { name: String, expr: Box<Node> },
	BinOp { lhs: Box<Node>, op: BinaryOp, rhs: Box<Node> },
	UnaryOp { expr: Box<Node>, op: UnaryOp },
}
