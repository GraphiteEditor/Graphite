use crate::matrix::Matrix;
use crate::quantity::{MatrixQuantity, Quantity};
use crate::value::Value;
use std::fmt;

/// What an expression evaluates to, one of the language's two sorts: a value or a matrix, each with the units it is measured in.
/// The matrix is boxed so that a value, the common case, stays small.
#[derive(Debug, Clone, PartialEq)]
pub enum Object {
	Value(Quantity),
	Matrix(Box<MatrixQuantity>),
}

/// Generates readers that see a value's parts read as a plain number and nothing of a matrix.
macro_rules! value_readers {
	($($fn_name:ident: $type:ty),* $(,)?) => {
		$(
			#[doc = concat!("Reads the object as [`Value::", stringify!($fn_name), "`] does once read as a plain number, or `None` for a matrix or a value with a dimension other than an angle.")]
			pub fn $fn_name(&self) -> Option<$type> {
				self.as_value()?.$fn_name()
			}
		)*
	};
}

impl Object {
	/// The value with its unit, or `None` for a matrix.
	#[inline]
	pub fn as_quantity(&self) -> Option<&Quantity> {
		match self {
			Self::Value(quantity) => Some(quantity),
			Self::Matrix(_) => None,
		}
	}

	/// The value read as a plain number, as [`Quantity::as_plain`] reads it, or `None` for a matrix.
	#[inline]
	pub fn as_value(&self) -> Option<Value> {
		self.as_quantity()?.as_plain()
	}

	/// The matrix with its units, or `None` for a value.
	#[inline]
	pub fn as_matrix_quantity(&self) -> Option<&MatrixQuantity> {
		match self {
			Self::Value(_) => None,
			Self::Matrix(matrix) => Some(matrix),
		}
	}

	/// The matrix, or `None` for a value or a matrix with units.
	#[inline]
	pub fn as_matrix(&self) -> Option<Matrix> {
		self.as_matrix_quantity().filter(|matrix| matrix.is_plain()).map(|matrix| matrix.matrix)
	}

	#[inline]
	pub fn into_value(self) -> Option<Value> {
		self.as_value()
	}

	#[inline]
	pub fn into_matrix(self) -> Option<Matrix> {
		self.as_matrix()
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
		Self::Value(Quantity::plain(value.into()))
	}
}

impl From<Quantity> for Object {
	fn from(quantity: Quantity) -> Self {
		Self::Value(quantity)
	}
}

impl From<Matrix> for Object {
	fn from(matrix: Matrix) -> Self {
		Self::Matrix(Box::new(MatrixQuantity::plain(matrix)))
	}
}

impl From<MatrixQuantity> for Object {
	fn from(matrix: MatrixQuantity) -> Self {
		Self::Matrix(Box::new(matrix))
	}
}

impl fmt::Display for Object {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Value(quantity) => quantity.fmt(f),
			Self::Matrix(matrix) => matrix.fmt(f),
		}
	}
}
