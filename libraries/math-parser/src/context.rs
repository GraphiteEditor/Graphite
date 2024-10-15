use std::collections::HashMap;

use crate::value::Value;

pub struct ValueMap(HashMap<String, Value>);

pub struct NothingMap;

pub trait ValueProvider {
	fn get_value(&self, name: &str) -> Option<&Value>;
}

impl ValueProvider for &ValueMap {
	fn get_value(&self, name: &str) -> Option<&Value> {
		self.0.get(name)
	}
}

impl ValueProvider for NothingMap {
	fn get_value(&self, name: &str) -> Option<&Value> {
		None
	}
}

impl ValueProvider for ValueMap {
	fn get_value(&self, name: &str) -> Option<&Value> {
		self.0.get(name)
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

pub struct EvalContext<T: ValueProvider> {
	values: T,
}

impl Default for EvalContext<NothingMap> {
	fn default() -> Self {
		Self { values: NothingMap }
	}
}

impl<T: ValueProvider> EvalContext<T> {
	pub fn new(values: T) -> Self {
		Self { values }
	}

	pub fn get_value(&self, name: &str) -> Option<&Value> {
		self.values.get_value(name)
	}
}
