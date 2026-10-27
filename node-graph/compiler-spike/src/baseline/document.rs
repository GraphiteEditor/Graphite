//! A port of `graph_craft::document`: document nodes and the flattening passes of `Compiler::compile`.

use super::proto::{ConstructionArgs, ProtoNetwork, ProtoNode};
use super::types::Type;
use super::{Builtins, CompileMetadata, ContextModification, NodeId, PASSTHROUGH, VALUE_NODE};
use crate::workload::Reflection;
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

fn merge_ids(a: NodeId, b: NodeId) -> NodeId {
	let mut hasher = DefaultHasher::new();
	a.hash(&mut hasher);
	b.hash(&mut hasher);
	NodeId(hasher.finish())
}

/// Stands in for `generate_uuid`, which draws from a random source.
fn generate_node_id() -> NodeId {
	thread_local! {
		static STATE: std::cell::Cell<u64> = const { std::cell::Cell::new(0x9E37_79B9_7F4A_7C15) };
	}
	STATE.with(|state| {
		let mut x = state.get();
		x ^= x << 13;
		x ^= x >> 7;
		x ^= x << 17;
		state.set(x);
		NodeId(x)
	})
}

#[derive(Debug, PartialEq)]
pub enum ValueKind {
	/// A document value, represented by its exported cache hash and serialized size.
	Document {
		size: usize,
	},
	None,
	NodeIdPath(Vec<NodeId>),
	SourceId(u64),
	ContextModification(ContextModification),
}

#[derive(Debug, PartialEq)]
pub struct ValueData {
	pub ty: Type,
	pub kind: ValueKind,
}

/// A port of `MemoHash<TaggedValue>`: the value behind an `Arc` with its cache hash computed once.
#[derive(Clone, Debug)]
pub struct MemoValue {
	pub hash: u64,
	pub value: Arc<ValueData>,
}

impl PartialEq for MemoValue {
	fn eq(&self, other: &Self) -> bool {
		self.value == other.value
	}
}

impl MemoValue {
	pub fn new(data: ValueData) -> Self {
		let mut hasher = DefaultHasher::new();
		std::mem::discriminant(&data.kind).hash(&mut hasher);
		match &data.kind {
			ValueKind::Document { size } => size.hash(&mut hasher),
			ValueKind::None => {}
			ValueKind::NodeIdPath(path) => path.hash(&mut hasher),
			ValueKind::SourceId(source) => source.hash(&mut hasher),
			ValueKind::ContextModification(modification) => modification.hash(&mut hasher),
		}
		Self {
			hash: hasher.finish(),
			value: Arc::new(data),
		}
	}

	pub fn with_hash(data: ValueData, hash: u64) -> Self {
		Self { hash, value: Arc::new(data) }
	}

	pub fn ty(&self) -> Type {
		self.value.ty.clone()
	}

	pub fn source_id(&self) -> Option<u64> {
		match self.value.kind {
			ValueKind::SourceId(source) => Some(source),
			_ => None,
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeInput {
	Node { node_id: NodeId, output_index: usize },
	Value { value: MemoValue, exposed: bool },
	Import { import_type: Type, import_index: usize },
	Scope(Cow<'static, str>),
	Reflection(Reflection),
	Inline,
}

impl NodeInput {
	pub fn node(node_id: NodeId, output_index: usize) -> Self {
		Self::Node { node_id, output_index }
	}

	fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId) {
		if let NodeInput::Node { node_id, .. } = self {
			*node_id = f(*node_id)
		}
	}
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OriginalLocation {
	pub path: Option<Vec<NodeId>>,
	pub dependants: Vec<Vec<NodeId>>,
	pub auto_convert_index: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DocumentNodeImplementation {
	Network(NodeNetwork),
	ProtoNode(Cow<'static, str>),
	Extract,
}

impl DocumentNodeImplementation {
	fn output_count(&self) -> usize {
		match self {
			DocumentNodeImplementation::Network(network) => network.exports.len(),
			_ => 1,
		}
	}

	fn is_proto_node(&self, identifier: &str) -> bool {
		matches!(self, DocumentNodeImplementation::ProtoNode(proto) if proto == identifier)
	}
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentNode {
	pub inputs: Vec<NodeInput>,
	pub call_argument: Type,
	pub implementation: DocumentNodeImplementation,
	pub visible: bool,
	pub skip_deduplication: bool,
	pub compile_metadata: Option<CompileMetadata>,
	pub original_location: OriginalLocation,
}

impl Default for DocumentNode {
	fn default() -> Self {
		Self {
			inputs: Vec::new(),
			call_argument: Type::context(),
			implementation: DocumentNodeImplementation::ProtoNode(Cow::Borrowed(PASSTHROUGH)),
			visible: true,
			skip_deduplication: false,
			compile_metadata: None,
			original_location: OriginalLocation::default(),
		}
	}
}

impl DocumentNode {
	fn populate_first_network_input(&mut self, node_id: NodeId, output_index: usize, offset: usize) {
		let (index, _) = self.inputs.iter().enumerate().nth(offset).expect("network input should exist at the offset");
		self.inputs[index] = NodeInput::Node { node_id, output_index };
	}

	fn resolve_proto_node(self) -> ProtoNode {
		let DocumentNodeImplementation::ProtoNode(identifier) = self.implementation else {
			unreachable!("tried to resolve a node that was not flattened");
		};
		assert!(!self.inputs.iter().any(|input| matches!(input, NodeInput::Import { .. })), "received non-resolved input");

		let mut construction_args = ConstructionArgs::Nodes(vec![]);
		if let &[NodeInput::Inline] = self.inputs.as_slice() {
			construction_args = ConstructionArgs::Inline;
		}
		if let &[NodeInput::Value { ref value, .. }] = self.inputs.as_slice() {
			construction_args = ConstructionArgs::Value(value.clone());
		}
		if let ConstructionArgs::Nodes(nodes) = &mut construction_args {
			nodes.extend(self.inputs.iter().map(|input| match input {
				NodeInput::Node { node_id, .. } => *node_id,
				_ => unreachable!(),
			}));
		}

		ProtoNode {
			identifier,
			call_argument: self.call_argument,
			construction_args,
			original_location: self.original_location,
			skip_deduplication: self.skip_deduplication,
			compile_metadata: self.compile_metadata.unwrap_or_default(),
			upstream_context_features: 0,
		}
	}
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeNetwork {
	pub exports: Vec<NodeInput>,
	pub nodes: FxHashMap<NodeId, DocumentNode>,
	pub scope_injections: FxHashMap<String, (NodeId, Type)>,
	pub generated: bool,
}

struct ScopeChain<'a> {
	scopes: &'a FxHashMap<String, (NodeId, Type)>,
	parent: Option<&'a ScopeChain<'a>>,
}

enum ScopeChainLookup<'a> {
	Current(&'a NodeId, &'a Type),
	Parent(&'a NodeId, &'a Type),
	None,
}

impl ScopeChain<'_> {
	fn lookup(&self, key: &str) -> ScopeChainLookup<'_> {
		self.scopes
			.get(key)
			.map(|(id, ty)| ScopeChainLookup::Current(id, ty))
			.or_else(|| {
				self.parent.and_then(|parent| match parent.lookup(key) {
					ScopeChainLookup::Current(id, ty) | ScopeChainLookup::Parent(id, ty) => Some(ScopeChainLookup::Parent(id, ty)),
					ScopeChainLookup::None => None,
				})
			})
			.unwrap_or(ScopeChainLookup::None)
	}
}

impl NodeNetwork {
	pub fn resolve_scope_inputs(&mut self) {
		let mut leftover = Vec::new();
		self.resolve_scope_inputs_impl(None, &mut leftover);
		assert!(leftover.is_empty(), "Unresolved scope keys at top level: {leftover:?}");
	}

	fn resolve_scope_inputs_impl(&mut self, parent: Option<&ScopeChain<'_>>, network_inputs: &mut Vec<NodeInput>) {
		let scope_chain = ScopeChain {
			scopes: &self.scope_injections,
			parent,
		};

		for node in self.nodes.values_mut() {
			let DocumentNodeImplementation::Network(network) = &mut node.implementation else { continue };
			network.resolve_scope_inputs_impl(Some(&scope_chain), &mut node.inputs);
		}

		let mut key_to_index: FxHashMap<Cow<'static, str>, usize> = FxHashMap::default();
		for node in self.nodes.values_mut() {
			for input in node.inputs.iter_mut() {
				let NodeInput::Scope(key) = input else { continue };
				*input = match scope_chain.lookup(key) {
					ScopeChainLookup::Current(node_id, _) => NodeInput::node(*node_id, 0),
					ScopeChainLookup::Parent(_, ty) => {
						let import_index = *key_to_index.entry(key.clone()).or_insert_with(|| {
							let index = network_inputs.len();
							network_inputs.push(NodeInput::Scope(key.clone()));
							index
						});
						NodeInput::Import {
							import_type: ty.clone(),
							import_index,
						}
					}
					ScopeChainLookup::None => panic!("Scope key `{key}` not found in any ancestor scope_injections"),
				};
			}
		}
	}

	fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId + Copy) {
		self.exports.iter_mut().for_each(|output| output.map_ids(f));
		self.scope_injections.values_mut().for_each(|(id, _)| *id = f(*id));
		let nodes = std::mem::take(&mut self.nodes);
		self.nodes = nodes
			.into_iter()
			.map(|(id, mut node)| {
				node.inputs.iter_mut().for_each(|input| input.map_ids(f));
				node.original_location.dependants.iter_mut().for_each(|dependants| dependants.iter_mut().for_each(|id| *id = f(*id)));
				(f(id), node)
			})
			.collect();
	}

	pub fn generate_node_paths(&mut self, prefix: &[NodeId]) {
		for (node_id, node) in &mut self.nodes {
			let mut new_path = prefix.to_vec();
			if !self.generated {
				new_path.push(*node_id);
			}
			if let DocumentNodeImplementation::Network(network) = &mut node.implementation {
				network.generate_node_paths(new_path.as_slice());
			}
			if node.original_location.path.is_none() {
				node.original_location.path = Some(new_path);
				node.original_location.dependants = (0..node.implementation.output_count()).map(|_| Vec::new()).collect();
			}
		}
	}

	pub fn populate_dependants(&mut self) {
		let mut dependency_changes = Vec::new();
		for (node_id, node) in &mut self.nodes {
			let len = node.original_location.dependants.len();
			node.original_location.dependants.extend(vec![vec![]; node.implementation.output_count().max(len) - len]);
			for input in &node.inputs {
				if let NodeInput::Node { node_id: dependency_id, output_index } = input {
					dependency_changes.push((*dependency_id, *output_index, *node_id));
				}
			}
		}
		for (dependency_id, output_index, node_id) in dependency_changes {
			let node = self.nodes.get_mut(&dependency_id).expect("Encountered invalid node id");
			let len = node.original_location.dependants.len();
			node.original_location.dependants.extend(vec![vec![]; (output_index + 1).max(len) - len]);
			node.original_location.dependants[output_index].push(node_id);
		}
	}

	fn replace_node_inputs(&mut self, node_id: NodeId, old_input: (NodeId, usize), new_input: (NodeId, usize)) {
		let Some(node) = self.nodes.get_mut(&node_id) else { return };
		for input in node.inputs.iter_mut() {
			if let NodeInput::Node { node_id: input_id, output_index } = input
				&& (*input_id, *output_index) == old_input
			{
				(*input_id, *output_index) = new_input;
			}
		}
	}

	fn replace_network_outputs(&mut self, old_output: &NodeInput, new_output: &NodeInput) {
		for output in self.exports.iter_mut() {
			if *output == *old_output {
				*output = new_output.clone();
			}
		}
	}

	pub fn flatten(&mut self, node_id: NodeId, builtins: &Builtins) {
		let Some((id, mut node)) = self.nodes.remove_entry(&node_id) else { return };

		if !node.visible && !node.implementation.is_proto_node(PASSTHROUGH) {
			node.implementation = DocumentNodeImplementation::ProtoNode(Cow::Borrowed(PASSTHROUGH));

			if node.inputs.is_empty() {
				node.inputs.push(NodeInput::Value {
					value: MemoValue::new(ValueData {
						ty: Type::unit(),
						kind: ValueKind::None,
					}),
					exposed: false,
				});
			}

			let removed_inputs = node.inputs.split_off(1);
			for removed_input in removed_inputs {
				if let NodeInput::Node { node_id, output_index } = removed_input
					&& let Some(former_dependency) = self.nodes.get_mut(&node_id)
					&& !node.inputs.iter().any(|input| matches!(input, NodeInput::Node { node_id: other, .. } if *other == node_id))
				{
					former_dependency.original_location.dependants[output_index].retain(|&dependant| dependant != id);
				}
			}

			let path = node.original_location.path.clone().unwrap_or_default();
			Self::replace_value_inputs_with_nodes(&mut node.inputs, &mut self.nodes, &path, id, builtins);

			node.call_argument = Type::unit();
			self.nodes.insert(id, node);
			return;
		}

		let path = node.original_location.path.clone().unwrap_or_default();

		if !node.implementation.is_proto_node(VALUE_NODE) {
			Self::replace_value_inputs_with_nodes(&mut node.inputs, &mut self.nodes, &path, id, builtins);
		}

		let DocumentNodeImplementation::Network(mut inner_network) = node.implementation else {
			assert!(!self.nodes.contains_key(&id), "Trying to insert a node into the network caused an id conflict");
			self.nodes.insert(id, node);
			return;
		};

		Self::replace_value_inputs_with_nodes(
			&mut inner_network.exports,
			&mut inner_network.nodes,
			node.original_location.path.as_ref().unwrap_or(&vec![]),
			id,
			builtins,
		);

		inner_network.map_ids(|inner_id| merge_ids(id, inner_id));
		inner_network.populate_dependants();
		let new_nodes = inner_network.nodes.keys().cloned().collect::<Vec<_>>();

		for (key, value) in inner_network.scope_injections.into_iter() {
			self.scope_injections.entry(key).or_insert(value);
		}

		for (nested_node_id, mut nested_node) in inner_network.nodes.into_iter() {
			for (nested_input_index, nested_input) in nested_node.clone().inputs.iter().enumerate() {
				let NodeInput::Import { import_index, .. } = nested_input else { continue };
				let parent_input = node.inputs.get(*import_index).expect("Import index of network node implementation should always exist");
				match *parent_input {
					NodeInput::Node { node_id, output_index } => {
						nested_node.populate_first_network_input(node_id, output_index, nested_input_index);
						let input_node = self.nodes.get_mut(&node_id).expect("Unable to find input node");
						input_node.original_location.dependants[output_index].push(nested_node_id);
					}
					NodeInput::Import { import_index, .. } => {
						let parent_input_index = import_index;
						let Some(NodeInput::Import { import_index, .. }) = nested_node.inputs.get_mut(nested_input_index) else {
							continue;
						};
						*import_index = parent_input_index;
					}
					NodeInput::Inline => (),
					NodeInput::Value { .. } | NodeInput::Scope(_) | NodeInput::Reflection(_) => unreachable!("Should have been resolved before flattening"),
				}
			}
			self.nodes.insert(nested_node_id, nested_node);
		}

		for parent_input in &node.inputs {
			if let NodeInput::Node { node_id, output_index } = *parent_input
				&& let Some(input_node) = self.nodes.get_mut(&node_id)
			{
				input_node.original_location.dependants[output_index].retain(|&dependant| dependant != id);
			}
		}

		self.replace_node_with_its_exports(id, &node.original_location, &inner_network.exports);

		for node_id in new_nodes {
			self.flatten(node_id, builtins);
		}
	}

	fn replace_node_with_its_exports(&mut self, id: NodeId, original_location: &OriginalLocation, exports: &[NodeInput]) {
		for (i, export) in exports.iter().enumerate() {
			if let NodeInput::Node { node_id, output_index } = &export {
				for dependants in &original_location.dependants {
					for dependant in dependants {
						self.replace_node_inputs(*dependant, (id, i), (*node_id, *output_index));
					}
				}

				if let Some(new_output_node) = self.nodes.get_mut(node_id) {
					for dependant in &original_location.dependants[i] {
						new_output_node.original_location.dependants[*output_index].push(*dependant);
					}
					new_output_node.original_location.dependants[*output_index].retain(|&dependant| dependant != id);
				}
			}

			self.replace_network_outputs(&NodeInput::node(id, i), export);
		}
	}

	#[inline(never)]
	fn replace_value_inputs_with_nodes(inputs: &mut [NodeInput], collection: &mut FxHashMap<NodeId, DocumentNode>, path: &[NodeId], id: NodeId, builtins: &Builtins) {
		for export in inputs {
			let previous_export = std::mem::replace(
				export,
				NodeInput::Import {
					import_type: Type::unit(),
					import_index: 0,
				},
			);

			let (value, exposed) = match previous_export {
				NodeInput::Value { value, exposed } => (value, exposed),
				NodeInput::Reflection(Reflection::DocumentNodePath) => (
					MemoValue::new(ValueData {
						ty: builtins.node_id_path.clone(),
						kind: ValueKind::NodeIdPath(path.to_vec()),
					}),
					false,
				),
				NodeInput::Reflection(Reflection::SourceId) => {
					let mut hasher = rustc_hash::FxHasher::default();
					path.hash(&mut hasher);
					let data = ValueData {
						ty: builtins.source_id.clone(),
						kind: ValueKind::SourceId(hasher.finish()),
					};
					(MemoValue::new(data), false)
				}
				previous_export => {
					*export = previous_export;
					continue;
				}
			};

			let value_node_id = generate_node_id();
			let merged_node_id = merge_ids(id, value_node_id);
			let mut original_location = OriginalLocation {
				path: Some(path.to_vec()),
				dependants: vec![vec![id]],
				..Default::default()
			};
			if let Some(path) = &mut original_location.path {
				path.push(value_node_id);
			}

			collection.insert(
				merged_node_id,
				DocumentNode {
					inputs: vec![NodeInput::Value { value, exposed }],
					implementation: DocumentNodeImplementation::ProtoNode(Cow::Borrowed(VALUE_NODE)),
					original_location,
					..Default::default()
				},
			);
			*export = NodeInput::node(merged_node_id, 0);
		}
	}

	fn remove_passthrough_node(&mut self, passthrough_id: NodeId) {
		let node = self.nodes.remove(&passthrough_id).expect("Passthrough node should exist");
		assert_eq!(node.inputs.len(), 1, "Passthrough node has more than one input");

		if let NodeInput::Node {
			node_id: upstream_node_id,
			output_index: upstream_output_index,
		} = node.inputs[0]
			&& let Some(upstream_node) = self.nodes.get_mut(&upstream_node_id)
		{
			let dependants_of_upstream = &mut upstream_node.original_location.dependants[upstream_output_index];
			dependants_of_upstream.extend(node.original_location.dependants[0].iter().copied());
			dependants_of_upstream.retain(|&dependant| dependant != passthrough_id);
		}

		for node_input in self.exports.iter_mut() {
			if matches!(node_input, NodeInput::Node { node_id, .. } if *node_id == passthrough_id) {
				*node_input = node.inputs[0].clone();
			}
		}

		for &dependant_id in &node.original_location.dependants[0] {
			let Some(dependant_node) = self.nodes.get_mut(&dependant_id) else { continue };
			for input in dependant_node.inputs.iter_mut() {
				if matches!(input, NodeInput::Node { node_id, .. } if *node_id == passthrough_id) {
					*input = node.inputs[0].clone();
				}
			}
		}
	}

	pub fn remove_all_passthrough_nodes(&mut self) {
		let passthrough_nodes = self
			.nodes
			.iter()
			.filter(|(_, node)| node.implementation.is_proto_node(PASSTHROUGH))
			.map(|(id, _)| *id)
			.collect::<Vec<_>>();
		for id in passthrough_nodes {
			self.remove_passthrough_node(id);
		}
	}

	pub fn into_proto_network(self) -> ProtoNetwork {
		let nodes: Vec<_> = self.nodes.into_iter().map(|(id, node)| (id, node.resolve_proto_node())).collect();
		let [NodeInput::Node { node_id, .. }] = self.exports.as_slice() else {
			panic!("The spike only handles networks with a single node export");
		};
		ProtoNetwork {
			inputs: Vec::new(),
			output: *node_id,
			nodes,
		}
	}
}
