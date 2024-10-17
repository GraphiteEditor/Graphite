use std::collections::HashMap;

use crate::value::Value;

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

impl ValueMap {
	pub fn new() -> Self {
		ValueMap(HashMap::new())
	}

	pub fn insert(&mut self, name: String, value: Value) {
		self.0.insert(name, value);
	}

	pub fn contains(&self, name: &str) -> bool {
		self.0.contains_key(name)
	}

	pub fn remove(&mut self, name: &str) -> Option<Value> {
		self.0.remove(name)
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
