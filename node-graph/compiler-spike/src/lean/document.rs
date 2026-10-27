//! Documents as tables of shared definitions with index-based references, built once at load time.

use super::types::{TypeId, TypeInterner};
use crate::workload::{self, CompileMetadata, Reflection};
use rustc_hash::FxHashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IdentifierId(pub u32);

/// A definition owned by the registry (substitution templates) or by one document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefinitionId {
	Template(u32),
	Document(u32),
}

impl DefinitionId {
	/// The index within its owner's definition list, the registry's templates or the document's definitions.
	pub fn index(self) -> usize {
		match self {
			DefinitionId::Template(index) | DefinitionId::Document(index) => index as usize,
		}
	}
}

#[derive(Clone, Copy, Debug)]
pub struct Value {
	pub ty: TypeId,
	pub hash: u64,
}

#[derive(Clone, Copy, Debug)]
pub enum Input {
	Node { local: u32, output: u32 },
	Value(Value),
	Import(u32),
	Scope(u32),
	Reflection(Reflection),
	Inline,
}

#[derive(Clone, Copy, Debug)]
pub enum Implementation {
	Proto(IdentifierId),
	Network(DefinitionId),
	Extract,
}

#[derive(Clone, Debug)]
pub struct Node {
	pub id: u64,
	pub implementation: Implementation,
	pub inputs: Range<u32>,
	pub call_argument: TypeId,
	pub visible: bool,
	pub skip_deduplication: bool,
	pub compile_metadata: Option<CompileMetadata>,
}

#[derive(Clone, Debug, Default)]
pub struct Definition {
	pub nodes: Vec<Node>,
	pub inputs: Vec<Input>,
	pub exports: Vec<Input>,
	/// Scope keys this network injects, with the local node providing each.
	pub scopes: Vec<(u32, u32)>,
	pub generated: bool,
	/// Whether this network is part of the document tree, where the preprocessor substitutes nodes and fills in their compile metadata.
	pub in_document: bool,
	/// A Merkle hash of everything elaboration reads, including nested definitions, which keys cached instances.
	pub deep_hash: u64,
	/// The document definition and node implemented by this definition, so edits can rehash up to the root.
	pub owner: Option<(u32, u32)>,
	/// For each node, how many exports its instance has, or `SINGLE_OUTPUT` when it lowers to one IR node.
	pub export_counts: Vec<u32>,
}

pub const SINGLE_OUTPUT: u32 = u32::MAX;

impl Definition {
	pub fn node_inputs(&self, node: &Node) -> &[Input] {
		&self.inputs[node.inputs.start as usize..node.inputs.end as usize]
	}

	/// Hashes the definition's content, with nested definitions contributing their own deep hashes.
	pub fn compute_deep_hash(&self, nested_hash: impl Fn(DefinitionId) -> u64) -> u64 {
		let mut hasher = rustc_hash::FxHasher::default();
		(self.generated, self.in_document).hash(&mut hasher);
		for node in &self.nodes {
			node.id.hash(&mut hasher);
			match node.implementation {
				Implementation::Proto(identifier) => (0_u8, identifier.0 as u64).hash(&mut hasher),
				Implementation::Network(definition) => (1_u8, nested_hash(definition)).hash(&mut hasher),
				Implementation::Extract => (2_u8, 0_u64).hash(&mut hasher),
			}
			(node.call_argument, node.visible, node.skip_deduplication, node.compile_metadata).hash(&mut hasher);
			hash_inputs(self.node_inputs(node), &mut hasher);
		}
		hash_inputs(&self.exports, &mut hasher);
		self.scopes.hash(&mut hasher);
		hasher.finish()
	}
}

fn hash_inputs(inputs: &[Input], hasher: &mut impl Hasher) {
	for input in inputs {
		match *input {
			Input::Node { local, output } => (0_u8, local, output).hash(hasher),
			Input::Value(value) => (1_u8, value.ty, value.hash).hash(hasher),
			Input::Import(index) => (2_u8, index).hash(hasher),
			Input::Scope(key) => (3_u8, key).hash(hasher),
			Input::Reflection(reflection) => (4_u8, reflection as u32).hash(hasher),
			Input::Inline => 5_u8.hash(hasher),
		}
	}
}

pub struct Substitution {
	pub definition: DefinitionId,
	pub wrapped_local: u32,
	/// The template's inputs, which fill in inputs a document node lacks.
	pub defaults: Vec<Input>,
}

/// Interns identifiers and scope keys, shared by the registry and every document.
#[derive(Default)]
pub struct Strings {
	ids: FxHashMap<String, u32>,
	names: Vec<String>,
}

impl Strings {
	pub fn intern(&mut self, name: &str) -> u32 {
		if let Some(&id) = self.ids.get(name) {
			return id;
		}
		let id = self.names.len() as u32;
		self.names.push(name.to_string());
		self.ids.insert(name.to_string(), id);
		id
	}

	pub fn get(&self, name: &str) -> Option<u32> {
		self.ids.get(name).copied()
	}

	pub fn name(&self, id: u32) -> &str {
		&self.names[id as usize]
	}
}

/// Converts workload networks into definitions appended to `definitions`.
pub struct Converter<'a> {
	pub types: &'a mut TypeInterner,
	pub strings: &'a mut Strings,
	pub definitions: &'a mut Vec<Definition>,
	pub template: bool,
	pub in_document: bool,
}

impl Converter<'_> {
	fn id(&self, index: usize) -> DefinitionId {
		match self.template {
			true => DefinitionId::Template(index as u32),
			false => DefinitionId::Document(index as u32),
		}
	}

	pub fn input(&mut self, input: &workload::Input, locals: &FxHashMap<u64, u32>) -> Input {
		match input {
			workload::Input::Node(reference) => Input::Node {
				local: locals[&reference.node],
				output: reference.output as u32,
			},
			workload::Input::Value(value) => Input::Value(Value {
				ty: self.types.convert(&value.ty),
				hash: value.hash,
			}),
			workload::Input::Import(import) => Input::Import(import.index as u32),
			workload::Input::Scope(key) => Input::Scope(self.strings.intern(key)),
			workload::Input::Reflection(reflection) => Input::Reflection(*reflection),
			workload::Input::Inline => Input::Inline,
		}
	}

	pub fn node(&mut self, id: u64, node: &workload::Node, definition: &mut Definition, locals: &FxHashMap<u64, u32>) -> Node {
		let implementation = match &node.implementation {
			workload::Implementation::Proto(identifier) => Implementation::Proto(IdentifierId(self.strings.intern(identifier))),
			workload::Implementation::Network(network) => Implementation::Network(self.network(network)),
			workload::Implementation::Extract => Implementation::Extract,
		};
		let start = definition.inputs.len() as u32;
		for input in &node.inputs {
			let input = self.input(input, locals);
			definition.inputs.push(input);
		}
		Node {
			id,
			implementation,
			inputs: start..definition.inputs.len() as u32,
			call_argument: self.types.convert(&node.call_argument),
			visible: node.visible,
			skip_deduplication: node.skip_deduplication,
			compile_metadata: node.compile_metadata,
		}
	}

	/// Converts a network with additional root nodes, such as the preprocessor's inject scope templates.
	pub fn network_with(&mut self, network: &workload::Network, extra_nodes: &[(u64, workload::Node)], extra_scopes: &[(String, u64)]) -> DefinitionId {
		let all_nodes: Vec<&(u64, workload::Node)> = network.nodes.iter().chain(extra_nodes).collect();
		let locals: FxHashMap<u64, u32> = all_nodes.iter().enumerate().map(|(index, (id, _))| (*id, index as u32)).collect();

		let mut definition = Definition {
			generated: network.generated,
			in_document: self.in_document,
			..Default::default()
		};
		for (id, node) in all_nodes {
			let node = self.node(*id, node, &mut definition, &locals);
			definition.nodes.push(node);
		}
		definition.exports = network.exports.iter().map(|input| self.input(input, &locals)).collect();
		definition.scopes = network
			.scope_injections
			.iter()
			.map(|(key, id, _)| (key.as_str(), *id))
			.chain(extra_scopes.iter().map(|(key, id)| (key.as_str(), *id)))
			.map(|(key, id)| (self.strings.intern(key), locals[&id]))
			.collect();

		let index = self.definitions.len() as u32;
		definition.deep_hash = definition.compute_deep_hash(|nested| self.definitions[nested.index()].deep_hash);
		for (local, node) in definition.nodes.iter().enumerate() {
			if let Implementation::Network(nested) = node.implementation {
				self.definitions[nested.index()].owner = Some((index, local as u32));
			}
		}
		self.definitions.push(definition);
		self.id(index as usize)
	}

	pub fn network(&mut self, network: &workload::Network) -> DefinitionId {
		self.network_with(network, &[], &[])
	}
}
