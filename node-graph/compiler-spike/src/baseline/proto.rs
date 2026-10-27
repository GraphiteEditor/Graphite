//! A port of `graph_craft::proto`: proto networks, context nullification, and stable node IDs.

use super::document::{MemoValue, OriginalLocation, ValueData, ValueKind};
use super::types::Type;
use super::{Builtins, CompileMetadata, ContextModification, FOOTPRINT, MEMOIZE, NodeId, PASSTHROUGH, VALUE_NODE};
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, PartialEq)]
pub enum ConstructionArgs {
	Value(MemoValue),
	Nodes(Vec<NodeId>),
	Inline,
}

impl ConstructionArgs {
	fn cache_hash<H: Hasher>(&self, state: &mut H) {
		std::mem::discriminant(self).hash(state);
		match self {
			Self::Nodes(nodes) => {
				for node in nodes {
					node.hash(state);
				}
			}
			Self::Value(value) => value.hash.hash(state),
			Self::Inline => {}
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProtoNode {
	pub construction_args: ConstructionArgs,
	pub call_argument: Type,
	pub identifier: Cow<'static, str>,
	pub original_location: OriginalLocation,
	pub skip_deduplication: bool,
	pub compile_metadata: CompileMetadata,
	pub upstream_context_features: u32,
}

impl Default for ProtoNode {
	fn default() -> Self {
		Self {
			identifier: Cow::Borrowed(PASSTHROUGH),
			construction_args: ConstructionArgs::Nodes(Vec::new()),
			call_argument: Type::unit(),
			original_location: OriginalLocation::default(),
			skip_deduplication: false,
			compile_metadata: CompileMetadata::default(),
			upstream_context_features: 0,
		}
	}
}

impl ProtoNode {
	fn stable_node_id(&self) -> NodeId {
		let mut hasher = rustc_hash::FxHasher::default();

		self.identifier.as_ref().hash(&mut hasher);
		self.construction_args.cache_hash(&mut hasher);
		if self.skip_deduplication {
			self.original_location.path.hash(&mut hasher);
		}

		std::mem::discriminant(&self.call_argument).hash(&mut hasher);
		self.call_argument.hash(&mut hasher);

		NodeId(hasher.finish())
	}

	fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId) {
		if let ConstructionArgs::Nodes(ids) = &mut self.construction_args {
			ids.iter_mut().for_each(|id| *id = f(*id));
		}
	}
}

#[derive(Clone, Copy, PartialEq)]
enum NodeState {
	Unvisited,
	Visiting,
	Visited,
}

#[derive(Clone, Debug, Default)]
pub struct ProtoNetwork {
	pub inputs: Vec<NodeId>,
	pub output: NodeId,
	pub nodes: Vec<(NodeId, ProtoNode)>,
}

impl ProtoNetwork {
	fn collect_outwards_edges(&self) -> HashMap<NodeId, Vec<NodeId>> {
		let mut edges: HashMap<NodeId, Vec<NodeId>> = self.nodes.iter().map(|(id, _)| (*id, Vec::new())).collect();
		for (id, node) in &self.nodes {
			if let ConstructionArgs::Nodes(ref_nodes) = &node.construction_args {
				for ref_id in ref_nodes {
					edges.entry(*ref_id).or_default().push(*id)
				}
			}
		}
		edges
	}

	pub fn generate_stable_node_ids(&mut self) {
		let outwards_edges = self.collect_outwards_edges();

		for index in 0..self.nodes.len() {
			let stable_id = self.nodes[index].1.stable_node_id();
			self.replace_node_id(&outwards_edges, NodeId(index as u64), stable_id);
			self.nodes[index].0 = stable_id;
		}
	}

	fn collect_inwards_edges_with_mapping(&self) -> (Vec<Vec<usize>>, FxHashMap<NodeId, usize>) {
		let id_map: FxHashMap<_, _> = self.nodes.iter().enumerate().map(|(index, (id, _))| (*id, index)).collect();

		let mut inwards_edges = vec![Vec::new(); self.nodes.len()];
		for (node_id, node) in &self.nodes {
			let node_index = id_map[node_id];
			if let ConstructionArgs::Nodes(ref_nodes) = &node.construction_args {
				for ref_id in ref_nodes {
					inwards_edges[node_index].push(id_map[ref_id]);
				}
			}
		}

		(inwards_edges, id_map)
	}

	/// With `memoize`, each node's context analysis runs once instead of once per path reaching it, which Graphite currently does.
	pub fn insert_context_nullification_nodes(&mut self, builtins: &Builtins, memoize: bool) -> Result<(), String> {
		self.reorder_ids()?;

		let mut analyzed = memoize.then(FxHashMap::default);
		let (output_dependencies, _) = self.find_context_dependencies(self.output, builtins, &mut analyzed);
		self.nodes[self.output.0 as usize].1.upstream_context_features = output_dependencies.features;

		self.reorder_ids()?;
		Ok(())
	}

	pub fn non_constant_inputs(&self) -> usize {
		self.nodes
			.iter()
			.filter(|(_, node)| node.compile_metadata.constant_inputs != 0)
			.map(|(_, node)| {
				let inputs = match &node.construction_args {
					ConstructionArgs::Nodes(inputs) => inputs.as_slice(),
					_ => &[],
				};
				inputs
					.iter()
					.enumerate()
					.filter(|&(index, &input)| index < 64 && node.compile_metadata.constant_inputs & (1 << index) != 0 && !self.resolves_to_value(input))
					.count()
			})
			.sum()
	}

	fn resolves_to_value(&self, mut id: NodeId) -> bool {
		loop {
			let Some((_, node)) = self.nodes.get(id.0 as usize) else { return false };
			match &node.construction_args {
				ConstructionArgs::Value(_) => return true,
				ConstructionArgs::Nodes(inputs) if node.compile_metadata.forwards_input => match inputs.first() {
					Some(&input) => id = input,
					None => return false,
				},
				_ => return false,
			}
		}
	}

	pub fn footprint_invariance_violations(&self) -> usize {
		self.nodes
			.iter()
			.filter(|(_, node)| node.compile_metadata.footprint_invariant && node.upstream_context_features & FOOTPRINT != 0)
			.count()
	}

	fn insert_context_nullification_node(&mut self, node_id: NodeId, context_dependencies: ContextModification, builtins: &Builtins) -> NodeId {
		let (_, node) = &self.nodes[node_id.0 as usize];
		let mut path = node.original_location.path.clone();
		if let Some(path) = path.as_mut() {
			path.push(NodeId(10))
		}

		let features = context_dependencies.features;
		let forwarding = CompileMetadata {
			forwards_input: true,
			..Default::default()
		};

		let memoize_node_id = NodeId(self.nodes.len() as u64);
		self.nodes.push((
			memoize_node_id,
			ProtoNode {
				construction_args: ConstructionArgs::Nodes(vec![node_id]),
				call_argument: Type::context(),
				identifier: Cow::Borrowed(MEMOIZE),
				original_location: OriginalLocation {
					path: path.clone(),
					..Default::default()
				},
				compile_metadata: forwarding,
				upstream_context_features: features,
				..Default::default()
			},
		));

		let nullification_value_node_id = NodeId(self.nodes.len() as u64);
		let value = ValueData {
			ty: builtins.context_modification.clone(),
			kind: ValueKind::ContextModification(context_dependencies),
		};
		self.nodes.push((
			nullification_value_node_id,
			ProtoNode {
				construction_args: ConstructionArgs::Value(MemoValue::new(value)),
				call_argument: Type::context(),
				identifier: Cow::Borrowed(VALUE_NODE),
				original_location: OriginalLocation {
					path: path.clone(),
					..Default::default()
				},
				..Default::default()
			},
		));

		let nullification_node_id = NodeId(self.nodes.len() as u64);
		self.nodes.push((
			nullification_node_id,
			ProtoNode {
				construction_args: ConstructionArgs::Nodes(vec![memoize_node_id, nullification_value_node_id]),
				call_argument: Type::context(),
				identifier: Cow::Borrowed(super::CONTEXT_MODIFICATION),
				original_location: OriginalLocation { path, ..Default::default() },
				compile_metadata: forwarding,
				upstream_context_features: features,
				..Default::default()
			},
		));
		nullification_node_id
	}

	fn find_context_dependencies(&mut self, id: NodeId, builtins: &Builtins, analyzed: &mut Option<FxHashMap<NodeId, (ContextModification, Option<NodeId>)>>) -> (ContextModification, Option<NodeId>) {
		if let Some(analyzed) = analyzed
			&& let Some(result) = analyzed.get(&id)
		{
			return result.clone();
		}
		let result = self.analyze_context_dependencies(id, builtins, analyzed);
		if let Some(analyzed) = analyzed {
			analyzed.insert(id, result.clone());
		}
		result
	}

	fn analyze_context_dependencies(
		&mut self,
		id: NodeId,
		builtins: &Builtins,
		analyzed: &mut Option<FxHashMap<NodeId, (ContextModification, Option<NodeId>)>>,
	) -> (ContextModification, Option<NodeId>) {
		let mut branch_dependencies = Vec::new();
		let mut combined = ContextModification::default();
		let node_index = id.0 as usize;

		let dependencies = self.nodes[node_index].1.compile_metadata;

		let mut inputs = match &self.nodes[node_index].1.construction_args {
			ConstructionArgs::Nodes(items) => items.clone(),
			ConstructionArgs::Value(value) if let Some(source) = value.source_id() => {
				return (
					ContextModification {
						features: dependencies.extract,
						sources: BTreeSet::from([source]),
					},
					Some(id),
				);
			}
			ConstructionArgs::Value(_) | ConstructionArgs::Inline => {
				return (
					ContextModification {
						features: dependencies.extract,
						sources: BTreeSet::new(),
					},
					Some(id),
				);
			}
		};

		for &node in &inputs {
			let branch = self.find_context_dependencies(node, builtins, analyzed);
			self.nodes[node.0 as usize].1.upstream_context_features = branch.0.features;

			combined.features |= branch.0.features;
			combined.sources.extend(&branch.0.sources);
			branch_dependencies.push(branch);
		}

		let features = (combined.features & !dependencies.inject) | dependencies.extract;
		let introduces_new_dependencies = combined.features & features != features;

		for (child_node, (child_dependencies, new_id)) in inputs.iter_mut().zip(branch_dependencies) {
			if let Some(new_id) = new_id {
				*child_node = new_id;
			} else if introduces_new_dependencies || child_dependencies != combined {
				*child_node = self.insert_context_nullification_node(*child_node, child_dependencies, builtins);
			}
		}
		self.nodes[node_index].1.construction_args = ConstructionArgs::Nodes(inputs);

		let net_injections = dependencies.inject & !dependencies.extract;
		let remaining_from_children = combined.features & !net_injections;
		let supplies_existing_dependencies = combined.features & !remaining_from_children != 0;

		let new_dependencies = ContextModification { features, sources: combined.sources };

		let mut new_id = None;
		if supplies_existing_dependencies {
			new_id = Some(self.insert_context_nullification_node(id, new_dependencies.clone(), builtins));
		}

		(new_dependencies, new_id)
	}

	fn replace_node_id(&mut self, outwards_edges: &HashMap<NodeId, Vec<NodeId>>, node_id: NodeId, replacement_node_id: NodeId) {
		if let Some(referring_nodes) = outwards_edges.get(&node_id) {
			for &referring_node_id in referring_nodes {
				let (_, referring_node) = &mut self.nodes[referring_node_id.0 as usize];
				referring_node.map_ids(|id| if id == node_id { replacement_node_id } else { id })
			}
		}

		if self.output == node_id {
			self.output = replacement_node_id;
		}
		self.inputs.iter_mut().for_each(|id| {
			if *id == node_id {
				*id = replacement_node_id;
			}
		});
	}

	fn topological_sort(&self) -> Result<Vec<NodeId>, String> {
		let (inwards_edges, id_map) = self.collect_inwards_edges_with_mapping();
		let mut sorted = Vec::with_capacity(self.nodes.len());
		let mut stack = vec![id_map[&self.output]];
		let mut state = vec![NodeState::Unvisited; self.nodes.len()];

		while let Some(&node_index) = stack.last() {
			match state[node_index] {
				NodeState::Unvisited => {
					state[node_index] = NodeState::Visiting;
					for &dependency_index in inwards_edges[node_index].iter().rev() {
						match state[dependency_index] {
							NodeState::Visiting => return Err(format!("Cycle detected involving node {:?}", self.nodes[dependency_index].0)),
							NodeState::Unvisited => stack.push(dependency_index),
							NodeState::Visited => {}
						}
					}
				}
				NodeState::Visiting => {
					stack.pop();
					state[node_index] = NodeState::Visited;
					sorted.push(NodeId(node_index as u64));
				}
				NodeState::Visited => {
					stack.pop();
				}
			}
		}

		Ok(sorted)
	}

	fn reorder_ids(&mut self) -> Result<(), String> {
		let order = self.topological_sort()?;

		let new_positions: FxHashMap<_, _> = order.iter().enumerate().map(|(position, id)| (self.nodes[id.0 as usize].0, position)).collect();

		let mut new_nodes = Vec::with_capacity(order.len());
		for (index, &id) in order.iter().enumerate() {
			let mut node = std::mem::take(&mut self.nodes[id.0 as usize].1);
			node.map_ids(|id| NodeId(*new_positions.get(&id).expect("node not found in lookup table") as u64));
			new_nodes.push((NodeId(index as u64), node));
		}

		self.nodes = new_nodes;
		self.inputs = self.inputs.iter().filter_map(|id| new_positions.get(id).map(|x| NodeId(*x as u64))).collect();
		self.output = NodeId(*new_positions.get(&self.output).unwrap() as u64);

		Ok(())
	}
}
