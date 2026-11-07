use crate::matrix::Matrix;
use crate::quantity::{MatrixQuantity, Quantity};
use crate::units::DEFAULT_PIXELS_PER_INCH;
use crate::value::Value;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

//TODO: editor integration, implement these traits for whatever is needed, maybe merge them if needed
pub trait ValueProvider {
	fn get_value(&self, name: &str) -> Option<Value>;

	/// The matrix bound to an uppercase-initial name, which the case rule reserves for matrices.
	fn get_matrix(&self, _name: &str) -> Option<Matrix> {
		None
	}

	/// The value bound to a name with the unit it is measured in, which is plain unless a host measures it.
	fn get_quantity(&self, name: &str) -> Option<Quantity> {
		self.get_value(name).map(Quantity::plain)
	}

	/// The matrix bound to a name with the units of its linear part and translation, which are plain unless a host measures them.
	fn get_matrix_quantity(&self, name: &str) -> Option<MatrixQuantity> {
		self.get_matrix(name).map(MatrixQuantity::plain)
	}
}

pub trait FunctionProvider {
	fn run_function(&self, name: &str, args: &[Value]) -> Option<Value>;

	/// Whether the host supplies a function of this name, true for every name `run_function` answers. It is read when parsing, since
	/// the host's function shadows any builtin of the same name and gives a value where the builtin might give a matrix.
	fn provides(&self, name: &str) -> bool;
}

#[derive(Default)]
pub struct ValueMap(pub HashMap<String, Value>);

pub struct NothingMap;

impl<V: ValueProvider> ValueProvider for &V {
	fn get_value(&self, name: &str) -> Option<Value> {
		(**self).get_value(name)
	}

	fn get_matrix(&self, name: &str) -> Option<Matrix> {
		(**self).get_matrix(name)
	}

	fn get_quantity(&self, name: &str) -> Option<Quantity> {
		(**self).get_quantity(name)
	}

	fn get_matrix_quantity(&self, name: &str) -> Option<MatrixQuantity> {
		(**self).get_matrix_quantity(name)
	}
}

impl ValueProvider for NothingMap {
	fn get_value(&self, _: &str) -> Option<Value> {
		None
	}
}

impl ValueProvider for ValueMap {
	fn get_value(&self, name: &str) -> Option<Value> {
		self.0.get(name).cloned()
	}
}

impl Deref for ValueMap {
	type Target = HashMap<String, Value>;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
impl DerefMut for ValueMap {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl FunctionProvider for NothingMap {
	fn run_function(&self, _: &str, _: &[Value]) -> Option<Value> {
		None
	}

	fn provides(&self, _: &str) -> bool {
		false
	}
}

pub struct EvalContext<V: ValueProvider, F: FunctionProvider> {
	values: V,
	functions: F,
	/// The size of the `px` unit, as the count of pixels in an inch.
	pixels_per_inch: f64,
}

impl Default for EvalContext<NothingMap, NothingMap> {
	fn default() -> Self {
		Self::new(NothingMap, NothingMap)
	}
}

impl<V: ValueProvider, F: FunctionProvider> EvalContext<V, F> {
	pub fn new(values: V, functions: F) -> Self {
		Self {
			values,
			functions,
			pixels_per_inch: DEFAULT_PIXELS_PER_INCH,
		}
	}

	/// Sizes the `px` unit as an inch over this count, where a count that isn't positive and finite keeps the default.
	pub fn with_pixels_per_inch(mut self, pixels_per_inch: f64) -> Self {
		if pixels_per_inch.is_finite() && pixels_per_inch > 0. {
			self.pixels_per_inch = pixels_per_inch;
		}
		self
	}

	pub fn pixels_per_inch(&self) -> f64 {
		self.pixels_per_inch
	}

	pub fn get_quantity(&self, name: &str) -> Option<Quantity> {
		self.values.get_quantity(name)
	}

	pub fn get_matrix_quantity(&self, name: &str) -> Option<MatrixQuantity> {
		self.values.get_matrix_quantity(name)
	}

	pub fn provides_function(&self, name: &str) -> bool {
		self.functions.provides(name)
	}

	pub fn get_value(&self, name: &str) -> Option<Value> {
		self.values.get_value(name)
	}

	pub fn get_matrix(&self, name: &str) -> Option<Matrix> {
		self.values.get_matrix(name)
	}

	pub fn run_function(&self, name: &str, args: &[Value]) -> Option<Value> {
		self.functions.run_function(name, args)
	}
}
