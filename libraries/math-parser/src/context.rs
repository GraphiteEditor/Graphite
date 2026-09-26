use crate::matrix::Matrix;
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
}

impl Default for EvalContext<NothingMap, NothingMap> {
	fn default() -> Self {
		Self {
			values: NothingMap,
			functions: NothingMap,
		}
	}
}

impl<V: ValueProvider, F: FunctionProvider> EvalContext<V, F> {
	pub fn new(values: V, functions: F) -> Self {
		Self { values, functions }
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
