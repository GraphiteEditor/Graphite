use crate::value::Value;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

//TODO: editor integration, implement these traits for whatever is needed, maybe merge them if needed
pub trait ValueProvider {
	fn get_value(&self, name: &str) -> Option<Value>;
}

pub trait FunctionProvider {
	fn run_function(&self, name: &str, args: &[Value]) -> Option<Value>;
}

pub struct ValueMap(HashMap<String, Value>);

pub struct NothingMap;

impl ValueProvider for &ValueMap {
	fn get_value(&self, name: &str) -> Option<Value> {
		self.0.get(name).cloned()
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

	pub fn run_function(&self, name: &str, args: &[Value]) -> Option<Value> {
		self.functions.run_function(name, args)
	}
}
