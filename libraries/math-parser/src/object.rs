use crate::matrix::Matrix;
use crate::value::Value;
use std::fmt;

/// What an expression evaluates to, one of the language's two sorts: a value or a matrix. The matrix is boxed so that a
/// value, the common case, stays small.
#[derive(Debug, Clone, PartialEq)]
pub enum Object {
	Value(Value),
	Matrix(Box<Matrix>),
}

/// Generates readers that see a value's parts and nothing of a matrix.
macro_rules! value_readers {
	($($fn_name:ident: $type:ty),* $(,)?) => {
		$(
			#[doc = concat!("Reads the object as [`Value::", stringify!($fn_name), "`] does, or `None` for a matrix.")]
			pub fn $fn_name(&self) -> Option<$type> {
				self.as_value()?.$fn_name()
			}
		)*
	};
}

impl Object {
	#[inline]
	pub fn as_value(&self) -> Option<&Value> {
		match self {
			Self::Value(value) => Some(value),
			Self::Matrix(_) => None,
		}
	}

	#[inline]
	pub fn as_matrix(&self) -> Option<&Matrix> {
		match self {
			Self::Value(_) => None,
			Self::Matrix(matrix) => Some(matrix),
		}
	}

	#[inline]
	pub fn into_value(self) -> Option<Value> {
		match self {
			Self::Value(value) => Some(value),
			Self::Matrix(_) => None,
		}
	}

	#[inline]
	pub fn into_matrix(self) -> Option<Matrix> {
		match self {
			Self::Value(_) => None,
			Self::Matrix(matrix) => Some(*matrix),
		}
	}

	value_readers! {
		as_real: f64,
		as_f32: f32,
		as_bool: bool,
		as_u8: u8,
		as_u16: u16,
		as_u32: u32,
		as_u64: u64,
		as_u128: u128,
		as_i8: i8,
		as_i16: i16,
		as_i32: i32,
		as_i64: i64,
		as_i128: i128,
	}
}

impl<T: Into<Value>> From<T> for Object {
	fn from(value: T) -> Self {
		Self::Value(value.into())
	}
}

impl From<Matrix> for Object {
	fn from(matrix: Matrix) -> Self {
		Self::Matrix(Box::new(matrix))
	}
}

impl fmt::Display for Object {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Value(value) => value.fmt(f),
			Self::Matrix(matrix) => matrix.fmt(f),
		}
	}
}
