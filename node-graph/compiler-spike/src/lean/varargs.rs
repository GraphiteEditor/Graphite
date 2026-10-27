//! Named context varargs: an injector binds a name to its value's type for its body, and readers inside the body take their
//! output type from the innermost binding of their name. A shared subgraph is lowered once per binding environment it reads.

use super::document::{IdentifierId, Strings};
use super::registry::Registry;
use super::types::{TypeData, TypeId, TypeInterner};
use crate::workload::{self, CompileMetadata};
use rustc_hash::FxHashMap;

pub const INJECT_VARARG: &str = "spike::InjectVarArg";
pub const READ_VARARG: &str = "spike::ReadVarArg";

const VARARGS: u32 = 1 << 6;

pub const INJECTION_METADATA: CompileMetadata = CompileMetadata {
	extract: 0,
	inject: VARARGS,
	footprint_invariant: false,
	forwards_input: false,
	constant_inputs: 0,
};

pub const READ_METADATA: CompileMetadata = CompileMetadata {
	extract: VARARGS,
	inject: 0,
	footprint_invariant: false,
	forwards_input: false,
	constant_inputs: 0,
};

/// The environment without bindings.
pub const ROOT: u32 = 0;

struct Binding {
	parent: u32,
	name: u64,
	ty: TypeId,
	depth: u32,
}

/// Binding chains interned so equal environments share an ID, which slots and readers key on.
pub struct Environments {
	bindings: Vec<Binding>,
	index: FxHashMap<(u32, u64, TypeId), u32>,
}

impl Default for Environments {
	fn default() -> Self {
		let root = Binding {
			parent: ROOT,
			name: 0,
			ty: TypeId::default(),
			depth: 0,
		};
		Self {
			bindings: vec![root],
			index: FxHashMap::default(),
		}
	}
}

impl Environments {
	pub fn bind(&mut self, parent: u32, name: u64, ty: TypeId) -> u32 {
		if let Some(&environment) = self.index.get(&(parent, name, ty)) {
			return environment;
		}
		let depth = self.depth(parent) + 1;
		self.bindings.push(Binding { parent, name, ty, depth });
		let environment = (self.bindings.len() - 1) as u32;
		self.index.insert((parent, name, ty), environment);
		environment
	}

	pub fn depth(&self, environment: u32) -> u32 {
		self.bindings[environment as usize].depth
	}

	/// The depth and type of the innermost binding of `name`.
	pub fn resolve(&self, mut environment: u32, name: u64) -> Option<(u32, TypeId)> {
		while environment != ROOT {
			let binding = &self.bindings[environment as usize];
			if binding.name == name {
				return Some((binding.depth, binding.ty));
			}
			environment = binding.parent;
		}
		None
	}
}

/// Registers the injector, generic over its name, value, and body types.
pub fn load(types: &mut TypeInterner, registry: &mut Registry, inject: IdentifierId, context: TypeId) {
	let [name, value, body] = ["N", "V", "U"].map(|generic| types.convert(&workload::Type::Generic(generic.to_string())));
	let inputs = [name, value, body].map(|ty| types.function(context, ty)).to_vec();
	registry.add(types, inject, context, inputs, body);
}

/// The reader monomorphized for one bound type, registered on first use like the adapters' `ItemToListNode<T>`.
pub fn monomorphized_reader(types: &mut TypeInterner, strings: &mut Strings, registry: &mut Registry, context: TypeId, name_type: TypeId, ty: TypeId) -> IdentifierId {
	let identifier = IdentifierId(strings.intern(&format!("{READ_VARARG}<{}>", display(types, ty))));
	if !registry.implementations.contains_key(&identifier) {
		let input = types.function(context, name_type);
		registry.add(types, identifier, context, vec![input], ty);
	}
	identifier
}

fn display(types: &TypeInterner, ty: TypeId) -> String {
	match types.get(ty) {
		TypeData::Concrete(_) | TypeData::Generic(_) => types.concrete_name(ty).unwrap_or("?").to_string(),
		TypeData::Item(element) => format!("Item<{}>", display(types, element)),
		TypeData::List(element) => format!("List<{}>", display(types, element)),
		TypeData::Fn(input, output) => format!("Fn({}) -> {}", display(types, input), display(types, output)),
	}
}
