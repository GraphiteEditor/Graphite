//! Interned types: every structurally distinct type is a `TypeId`, so equality is an integer compare and nothing is cloned.

use crate::workload;
use rustc_hash::FxHashMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TypeData {
	Generic(u32),
	Concrete(u32),
	Fn(TypeId, TypeId),
	Item(TypeId),
	List(TypeId),
}

#[derive(Default)]
pub struct TypeInterner {
	data: Vec<TypeData>,
	ids: FxHashMap<TypeData, TypeId>,
	names: Vec<String>,
	name_ids: FxHashMap<String, u32>,
}

impl TypeInterner {
	pub fn intern(&mut self, data: TypeData) -> TypeId {
		if let Some(&id) = self.ids.get(&data) {
			return id;
		}
		let id = TypeId(self.data.len() as u32);
		self.data.push(data);
		self.ids.insert(data, id);
		id
	}

	pub fn get(&self, id: TypeId) -> TypeData {
		self.data[id.0 as usize]
	}

	pub fn name_id(&mut self, name: &str) -> u32 {
		if let Some(&id) = self.name_ids.get(name) {
			return id;
		}
		let id = self.names.len() as u32;
		self.names.push(name.to_string());
		self.name_ids.insert(name.to_string(), id);
		id
	}

	pub fn name(&self, id: u32) -> &str {
		&self.names[id as usize]
	}

	pub fn concrete(&mut self, name: &str) -> TypeId {
		let name = self.name_id(name);
		self.intern(TypeData::Concrete(name))
	}

	pub fn function(&mut self, input: TypeId, output: TypeId) -> TypeId {
		self.intern(TypeData::Fn(input, output))
	}

	pub fn convert(&mut self, ty: &workload::Type) -> TypeId {
		let data = match ty {
			workload::Type::Generic(name) => TypeData::Generic(self.name_id(name)),
			workload::Type::Concrete(name) => TypeData::Concrete(self.name_id(name)),
			workload::Type::Fn(input, output) => TypeData::Fn(self.convert(input), self.convert(output)),
			workload::Type::Item(element) => TypeData::Item(self.convert(element)),
			workload::Type::List(element) => TypeData::List(self.convert(element)),
		};
		self.intern(data)
	}

	pub fn to_workload(&self, id: TypeId) -> workload::Type {
		match self.get(id) {
			TypeData::Generic(name) => workload::Type::Generic(self.name(name).to_string()),
			TypeData::Concrete(name) => workload::Type::Concrete(self.name(name).to_string()),
			TypeData::Fn(input, output) => workload::Type::Fn(Box::new(self.to_workload(input)), Box::new(self.to_workload(output))),
			TypeData::Item(element) => workload::Type::Item(Box::new(self.to_workload(element))),
			TypeData::List(element) => workload::Type::List(Box::new(self.to_workload(element))),
		}
	}

	/// Whether a value of type `from` may feed a connector of type `to`, as `valid_type` in Graphite.
	pub fn valid(&self, from: TypeId, to: TypeId) -> bool {
		if from == to {
			return true;
		}
		match (self.get(from), self.get(to)) {
			(TypeData::Generic(_), _) | (_, TypeData::Generic(_)) => true,
			(TypeData::Fn(in1, out1), TypeData::Fn(in2, out2)) => self.valid(out2, out1) && self.valid(in1, in2),
			(TypeData::Item(element1), TypeData::Item(element2)) | (TypeData::List(element1), TypeData::List(element2)) => self.valid(element1, element2),
			_ => false,
		}
	}

	pub fn nested(&self, id: TypeId) -> TypeId {
		match self.get(id) {
			TypeData::Fn(_, output) => self.nested(output),
			_ => id,
		}
	}

	pub fn fn_input(&self, id: TypeId) -> Option<TypeId> {
		match self.get(id) {
			TypeData::Fn(input, _) => Some(input),
			_ => None,
		}
	}

	pub fn fn_output(&self, id: TypeId) -> Option<TypeId> {
		match self.get(id) {
			TypeData::Fn(_, output) => Some(output),
			_ => None,
		}
	}

	pub fn generic_name(&self, id: TypeId) -> Option<u32> {
		match self.get(id) {
			TypeData::Generic(name) => Some(name),
			_ => None,
		}
	}

	/// Replaces every generic anywhere in `id`, such as the `T` in `Fn(Context, List<T>)`.
	pub fn substitute(&mut self, id: TypeId, replace: &impl Fn(u32) -> Option<TypeId>) -> TypeId {
		let data = match self.get(id) {
			TypeData::Generic(name) => return replace(name).unwrap_or(id),
			TypeData::Concrete(_) => return id,
			TypeData::Fn(input, output) => TypeData::Fn(self.substitute(input, replace), self.substitute(output, replace)),
			TypeData::Item(element) => TypeData::Item(self.substitute(element, replace)),
			TypeData::List(element) => TypeData::List(self.substitute(element, replace)),
		};
		self.intern(data)
	}

	/// Collects every generic name anywhere in `id`.
	pub fn collect_generics(&self, id: TypeId, generics: &mut Vec<u32>) {
		match self.get(id) {
			TypeData::Generic(name) if !generics.contains(&name) => generics.push(name),
			TypeData::Generic(_) | TypeData::Concrete(_) => {}
			TypeData::Fn(input, output) => {
				self.collect_generics(input, generics);
				self.collect_generics(output, generics);
			}
			TypeData::Item(element) | TypeData::List(element) => self.collect_generics(element, generics),
		}
	}

	pub fn bundle_element_name(&self, id: TypeId) -> Option<&str> {
		let TypeData::Item(element) = self.get(id) else { return None };
		let TypeData::Concrete(name) = self.get(element) else { return None };
		self.name(name).strip_prefix("core_types::list::Bundle<")?.strip_suffix('>')
	}

	pub fn concrete_name(&self, id: TypeId) -> Option<&str> {
		match self.get(id) {
			TypeData::Concrete(name) => Some(self.name(name)),
			_ => None,
		}
	}
}
