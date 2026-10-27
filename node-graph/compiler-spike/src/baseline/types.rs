//! A port of `core_types::Type` with the same layout and clone behavior, so type movement costs what it does in Graphite.

use crate::workload;
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::hash::{Hash, Hasher};

pub const UNIT_NAME: &str = "()";
pub const CONTEXT_NAME: &str = "core::option::Option<alloc::sync::Arc<core_types::context::OwnedContextImpl>>";
const UNIT_ID: u128 = 0;
const CONTEXT_ID: u128 = 1;

#[derive(Clone, Debug)]
pub struct TypeDescriptor {
	pub id: Option<u128>,
	pub name: Cow<'static, str>,
	pub size: usize,
	pub align: usize,
}

impl Hash for TypeDescriptor {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.name.hash(state);
	}
}

impl PartialEq for TypeDescriptor {
	fn eq(&self, other: &Self) -> bool {
		match (self.id, other.id) {
			(Some(id), Some(other_id)) => id == other_id,
			_ => self.name == other.name,
		}
	}
}

impl Eq for TypeDescriptor {}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
	Generic(Cow<'static, str>),
	Concrete(TypeDescriptor),
	Fn(Box<Type>, Box<Type>),
	Item(Box<Type>),
	List(Box<Type>),
}

impl Default for Type {
	fn default() -> Self {
		Type::unit()
	}
}

impl Type {
	fn concrete(id: u128, name: &'static str) -> Self {
		Type::Concrete(TypeDescriptor {
			id: Some(id),
			name: Cow::Borrowed(name),
			size: 0,
			align: 0,
		})
	}

	pub fn unit() -> Self {
		Type::concrete(UNIT_ID, UNIT_NAME)
	}

	pub fn context() -> Self {
		Type::concrete(CONTEXT_ID, CONTEXT_NAME)
	}

	pub fn function(input: &Type, output: &Type) -> Type {
		Type::Fn(Box::new(input.clone()), Box::new(output.clone()))
	}

	pub fn nested_type(&self) -> &Type {
		match self {
			Self::Fn(_, output) => output.nested_type(),
			_ => self,
		}
	}

	pub fn replace_nested(&mut self, f: impl Fn(&Type) -> Option<Type>) -> Option<Type> {
		if let Some(replacement) = f(self) {
			return Some(std::mem::replace(self, replacement));
		}
		match self {
			Self::Fn(_, output) => output.replace_nested(f),
			_ => None,
		}
	}

	pub fn fn_input(&self) -> Option<&Type> {
		match self {
			Type::Fn(input, _) => Some(input),
			_ => None,
		}
	}

	pub fn fn_output(&self) -> Option<&Type> {
		match self {
			Type::Fn(_, output) => Some(output),
			_ => None,
		}
	}

	pub fn bundle_element_name(&self) -> Option<&str> {
		let Type::Item(element) = self else { return None };
		let Type::Concrete(descriptor) = element.as_ref() else { return None };
		descriptor.name.strip_prefix("core_types::list::Bundle<")?.strip_suffix('>')
	}

	pub fn identifier_name(&self) -> String {
		match self {
			Type::Generic(name) => name.to_string(),
			Type::Concrete(descriptor) => simplify_identifier_name(&descriptor.name),
			Type::Fn(call_argument, return_value) => format!("{} called with {}", return_value.identifier_name(), call_argument.identifier_name()),
			Type::Item(element) => element.identifier_name(),
			Type::List(element) => format!("{}[]", element.identifier_name()),
		}
	}
}

fn simplify_identifier_name(name: &str) -> String {
	name.split('<')
		.map(|path| path.split(',').map(|path| path.split("::").last().unwrap_or(path)).collect::<Vec<_>>().join(","))
		.collect::<Vec<_>>()
		.join("<")
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct NodeIOTypes {
	pub call_argument: Type,
	pub return_value: Type,
	pub inputs: Vec<Type>,
}

impl NodeIOTypes {
	pub fn new(call_argument: Type, return_value: Type, inputs: Vec<Type>) -> Self {
		Self { call_argument, return_value, inputs }
	}

	pub fn ty(&self) -> Type {
		Type::Fn(Box::new(self.call_argument.clone()), Box::new(self.return_value.clone()))
	}
}

/// Interns concrete type names into `'static` strings with stable IDs, standing in for Rust's `TypeId`.
pub struct TypeNames {
	names: FxHashMap<String, (&'static str, u128)>,
}

impl Default for TypeNames {
	fn default() -> Self {
		let names = [(UNIT_NAME, UNIT_ID), (CONTEXT_NAME, CONTEXT_ID)]
			.into_iter()
			.map(|(name, id)| (name.to_string(), (name, id)))
			.collect();
		Self { names }
	}
}

impl TypeNames {
	/// Converts a type the way Graphite builds it in code: a `TypeId` and a borrowed name.
	pub fn convert(&mut self, ty: &workload::Type) -> Type {
		match ty {
			workload::Type::Generic(name) => Type::Generic(Cow::Borrowed(self.leak(name))),
			workload::Type::Concrete(name) => {
				let (name, id) = self.intern(name);
				Type::concrete(id, name)
			}
			workload::Type::Fn(input, output) => Type::Fn(Box::new(self.convert(input)), Box::new(self.convert(output))),
			workload::Type::Item(element) => Type::Item(Box::new(self.convert(element))),
			workload::Type::List(element) => Type::List(Box::new(self.convert(element))),
		}
	}

	/// Converts a type the way Graphite deserializes it from a document: no `TypeId` and an owned name.
	pub fn convert_deserialized(ty: &workload::Type) -> Type {
		match ty {
			workload::Type::Generic(name) => Type::Generic(Cow::Owned(name.clone())),
			workload::Type::Concrete(name) => Type::Concrete(TypeDescriptor {
				id: None,
				name: Cow::Owned(name.clone()),
				size: 0,
				align: 0,
			}),
			workload::Type::Fn(input, output) => Type::Fn(Box::new(Self::convert_deserialized(input)), Box::new(Self::convert_deserialized(output))),
			workload::Type::Item(element) => Type::Item(Box::new(Self::convert_deserialized(element))),
			workload::Type::List(element) => Type::List(Box::new(Self::convert_deserialized(element))),
		}
	}

	fn intern(&mut self, name: &str) -> (&'static str, u128) {
		if let Some(&entry) = self.names.get(name) {
			return entry;
		}
		let entry = (self.leak(name), self.names.len() as u128);
		self.names.insert(name.to_string(), entry);
		entry
	}

	/// Leaks the string so types hold `Cow::Borrowed` names like Graphite's `TypeId`-derived descriptors.
	pub fn leak(&mut self, name: &str) -> &'static str {
		Box::leak(name.to_string().into_boxed_str())
	}
}
