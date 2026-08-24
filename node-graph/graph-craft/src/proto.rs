use crate::document::value::TaggedValue;
use crate::document::{InlineRust, value};
use crate::document::{NodeId, OriginalLocation};
pub use core_types::registry::*;
use core_types::*;
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

#[derive(Debug, Default, PartialEq, Clone, Eq, serde::Serialize, serde::Deserialize)]
/// A list of [`ProtoNode`]s, which is an intermediate step between the [`crate::document::NodeNetwork`] and the `BorrowTree` containing a single flattened network.
pub struct ProtoNetwork {
	// TODO: remove this since it seems to be unused?
	// Should a proto Network even allow inputs? Don't think so
	pub inputs: Vec<NodeId>,
	/// The node ID that provides the output. This node is then responsible for calling the rest of the graph.
	pub output: NodeId,
	/// A list of nodes stored in a Vec to allow for sorting.
	pub nodes: Vec<(NodeId, ProtoNode)>,
	/// Peak record-stack bytes for an evaluation, folded from the resolved layouts by [`compute_layouts`](ProtoNetwork::compute_layouts).
	#[serde(default)]
	pub stack_need: usize,
}

impl core::fmt::Display for ProtoNetwork {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.write_str("Proto Network with nodes: ")?;
		fn write_node(f: &mut core::fmt::Formatter<'_>, network: &ProtoNetwork, id: NodeId, indent: usize) -> core::fmt::Result {
			f.write_str(&"\t".repeat(indent))?;
			let Some((_, node)) = network.nodes.iter().find(|(node_id, _)| *node_id == id) else {
				return f.write_str("{{Unknown Node}}");
			};
			f.write_str("Node: ")?;
			f.write_str(node.identifier.as_str())?;

			f.write_str("\n")?;
			f.write_str(&"\t".repeat(indent))?;
			f.write_str("{\n")?;

			f.write_str(&"\t".repeat(indent + 1))?;
			f.write_str("Input: ")?;
			f.write_fmt(format_args!("Call Argument (type = {:?})", node.call_argument))?;
			f.write_str("\n")?;

			match &node.construction_args {
				ConstructionArgs::Value(value) => {
					f.write_str(&"\t".repeat(indent + 1))?;
					f.write_fmt(format_args!("Value construction argument: {value:?}"))?
				}
				ConstructionArgs::Nodes(nodes) => {
					for id in nodes {
						write_node(f, network, *id, indent + 1)?;
					}
				}
				ConstructionArgs::Inline(inline) => {
					f.write_str(&"\t".repeat(indent + 1))?;
					f.write_fmt(format_args!("Inline construction argument: {inline:?}"))?
				}
			}
			f.write_str(&"\t".repeat(indent))?;
			f.write_str("}\n")?;
			Ok(())
		}

		let id = self.output;
		write_node(f, self, id, 0)
	}
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
/// Defines the arguments used to construct the boxed node struct. This is used to call the constructor function in the `node_registry.rs` file - which is hidden behind a wall of macros.
pub enum ConstructionArgs {
	/// A value of a type that is known, allowing serialization (serde::Deserialize is not object safe)
	Value(MemoHash<value::TaggedValue>),
	/// A list of nodes used as inputs to the constructor function in `node_registry.rs`.
	/// The bool indicates whether to treat the node as lambda node.
	// TODO: use a struct for clearer naming.
	Nodes(Vec<NodeId>),
	/// Used for GPU computation to work around the limitations of rust-gpu.
	Inline(InlineRust),
}

impl Eq for ConstructionArgs {}

impl PartialEq for ConstructionArgs {
	fn eq(&self, other: &Self) -> bool {
		match (&self, &other) {
			(Self::Nodes(n1), Self::Nodes(n2)) => n1 == n2,
			(Self::Value(v1), Self::Value(v2)) => v1 == v2,
			_ => {
				use std::hash::Hasher;
				let hash = |input: &Self| {
					let mut hasher = rustc_hash::FxHasher::default();
					input.cache_hash(&mut hasher);
					hasher.finish()
				};
				hash(self) == hash(other)
			}
		}
	}
}

impl CacheHash for ConstructionArgs {
	fn cache_hash<H: std::hash::Hasher>(&self, state: &mut H) {
		core::mem::discriminant(self).hash(state);
		match self {
			Self::Nodes(nodes) => {
				for node in nodes {
					node.hash(state);
				}
			}
			Self::Value(value) => value.cache_hash(state),
			Self::Inline(inline) => inline.hash(state),
		}
	}
}

impl ConstructionArgs {
	pub fn new_function_args(&self) -> Vec<String> {
		match self {
			ConstructionArgs::Nodes(nodes) => nodes.iter().map(|n| format!("n{:0x}", n.0)).collect(),
			ConstructionArgs::Value(value) => vec![value.to_primitive_string()],
			ConstructionArgs::Inline(inline) => vec![inline.expr.clone()],
		}
	}
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Resolved {
	pub io: Option<NodeIOTypes>,
	pub layout_meta: Option<core_types::record::LayoutMeta>,
	pub layout: Option<core_types::record::RecordLayout>,
}

impl PartialEq for Resolved {
	fn eq(&self, _: &Self) -> bool {
		true
	}
}
impl Eq for Resolved {}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
/// A proto node is an intermediate step between the `DocumentNode` and the boxed struct that actually runs the node (found in the [`BorrowTree`]).
/// At different stages in the compilation process, this struct will be transformed into a reduced (more restricted) form acting as a subset of its original form, but that restricted form is still valid in the earlier stage in the compilation process before it was transformed.
pub struct ProtoNode {
	pub construction_args: ConstructionArgs,
	pub call_argument: Type,
	pub identifier: ProtoNodeIdentifier,
	pub original_location: OriginalLocation,
	pub skip_deduplication: bool,
	pub(crate) context_features: ContextDependencies,
	#[serde(skip)]
	pub(crate) resolved: Resolved,
}

impl Default for ProtoNode {
	fn default() -> Self {
		Self {
			identifier: graphene_core::ops::passthrough::IDENTIFIER,
			construction_args: ConstructionArgs::Value(value::TaggedValue::U32(0).into()),
			call_argument: concrete!(()),
			original_location: OriginalLocation::default(),
			skip_deduplication: false,
			context_features: Default::default(),
			resolved: Default::default(),
		}
	}
}

impl ProtoNode {
	/// A stable node ID is a hash of a node that should stay constant. This is used in order to remove duplicates from the graph.
	/// In the case of `skip_deduplication`, the `document_node_path` is also hashed in order to avoid duplicate monitor nodes from being removed (which would make it impossible to load thumbnails).
	pub fn stable_node_id(&self) -> Option<NodeId> {
		use std::hash::Hasher;
		let mut hasher = rustc_hash::FxHasher::default();

		self.identifier.as_str().hash(&mut hasher);
		self.construction_args.cache_hash(&mut hasher);
		if self.skip_deduplication {
			self.original_location.path.hash(&mut hasher);
		}

		std::mem::discriminant(&self.call_argument).hash(&mut hasher);
		self.call_argument.hash(&mut hasher);

		Some(NodeId(hasher.finish()))
	}

	/// Construct a new [`ProtoNode`] with the specified construction args and a `ClonedNode` implementation.
	pub fn value(value: ConstructionArgs, path: Vec<NodeId>) -> Self {
		let inputs_exposed = match &value {
			ConstructionArgs::Nodes(nodes) => nodes.len() + 1,
			_ => 2,
		};
		Self {
			identifier: ProtoNodeIdentifier::new("core_types::value::ClonedNode"),
			construction_args: value,
			call_argument: concrete!(Context),
			original_location: OriginalLocation {
				path: Some(path),
				inputs_exposed: vec![false; inputs_exposed],
				..Default::default()
			},
			skip_deduplication: false,
			context_features: Default::default(),
			resolved: Default::default(),
		}
	}

	/// Converts all references to other node IDs into new IDs by running the specified function on them.
	/// This can be used when changing the IDs of the nodes, for example in the case of generating stable IDs.
	pub fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId) {
		if let ConstructionArgs::Nodes(ids) = &mut self.construction_args {
			ids.iter_mut().for_each(|id| *id = f(*id));
		}
	}

	pub fn unwrap_construction_nodes(&self) -> Vec<NodeId> {
		match &self.construction_args {
			ConstructionArgs::Nodes(nodes) => nodes.clone(),
			_ => panic!("tried to unwrap nodes from non node construction args \n node: {self:#?}"),
		}
	}

	pub fn resolved_layout(&self) -> Option<&core_types::record::RecordLayout> {
		self.resolved.layout.as_ref()
	}
}

#[derive(Clone, Copy, PartialEq)]
enum NodeState {
	Unvisited,
	Visiting,
	Visited,
}

impl ProtoNetwork {
	fn check_ref(&self, ref_id: &NodeId, id: &NodeId) {
		debug_assert!(
			self.nodes.iter().any(|(check_id, _)| check_id == ref_id),
			"Node with ID {id} has a reference which uses the node with ID {ref_id} which doesn't exist in network {self:#?}"
		);
	}

	#[cfg(debug_assertions)]
	pub fn example() -> (Self, NodeId, ProtoNode) {
		let node_id = NodeId(1);
		let proto_node = ProtoNode::default();
		let proto_network = ProtoNetwork {
			inputs: vec![node_id],
			output: node_id,
			nodes: vec![(node_id, proto_node.clone())],
			..Default::default()
		};
		(proto_network, node_id, proto_node)
	}

	/// Construct a hashmap containing a list of the nodes that depend on this proto network.
	pub fn collect_outwards_edges(&self) -> HashMap<NodeId, Vec<NodeId>> {
		let mut edges: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
		for (id, node) in &self.nodes {
			if let ConstructionArgs::Nodes(ref_nodes) = &node.construction_args {
				for ref_id in ref_nodes {
					self.check_ref(ref_id, id);
					edges.entry(*ref_id).or_default().push(*id)
				}
			}
		}
		edges
	}

	/// Convert all node IDs to be stable (based on the hash generated by [`ProtoNode::stable_node_id`]).
	/// This function requires that the graph be topologically sorted.
	pub fn generate_stable_node_ids(&mut self) {
		debug_assert!(self.is_topologically_sorted());
		let outwards_edges = self.collect_outwards_edges();

		for index in 0..self.nodes.len() {
			let Some(sni) = self.nodes[index].1.stable_node_id() else {
				panic!("failed to generate stable node id for node {:#?}", self.nodes[index].1);
			};
			self.replace_node_id(&outwards_edges, NodeId(index as u64), sni);
			self.nodes[index].0 = sni;
		}

		// Equal nodes hash to one id; the copies must go, or a pass that
		// rewrites one of them (like adapter splicing) leaves the others stale.
		let mut seen = HashSet::new();
		self.nodes.retain(|(id, _)| seen.insert(*id));
	}

	// TODO: Remove
	/// Create a hashmap with the list of nodes this proto network depends on/uses as inputs.
	pub fn collect_inwards_edges(&self) -> HashMap<NodeId, Vec<NodeId>> {
		let mut edges: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
		for (id, node) in &self.nodes {
			if let ConstructionArgs::Nodes(ref_nodes) = &node.construction_args {
				for ref_id in ref_nodes {
					self.check_ref(ref_id, id);
					edges.entry(*id).or_default().push(*ref_id)
				}
			}
		}
		edges
	}

	fn collect_inwards_edges_with_mapping(&self) -> (Vec<Vec<usize>>, FxHashMap<NodeId, usize>) {
		let id_map: FxHashMap<_, _> = self.nodes.iter().enumerate().map(|(idx, (id, _))| (*id, idx)).collect();

		// Collect inwards edges using dense indices
		let mut inwards_edges = vec![Vec::new(); self.nodes.len()];
		for (node_id, node) in &self.nodes {
			let node_index = id_map[node_id];

			if let ConstructionArgs::Nodes(ref_nodes) = &node.construction_args {
				for ref_id in ref_nodes {
					self.check_ref(ref_id, &NodeId(node_index as u64));
					inwards_edges[node_index].push(id_map[ref_id]);
				}
			}
		}

		(inwards_edges, id_map)
	}

	pub fn source_ids(&self) -> Vec<SourceId> {
		self.nodes.iter().flat_map(|(_, node)| node.context_features.sources().iter().copied()).collect()
	}

	pub fn resolve_types(&mut self, registry: &Registry) -> Result<(), String> {
		self.reorder_ids()?;
		for index in 0..self.nodes.len() {
			let resolved = {
				let node = &self.nodes[index].1;
				match &node.construction_args {
					ConstructionArgs::Value(value) => Resolved {
						io: Some(NodeIOTypes::new(concrete!(Context), Type::Record(Box::new(value.ty())), vec![])),
						..Default::default()
					},
					_ => {
						let inputs: Vec<Type> = match &node.construction_args {
							ConstructionArgs::Nodes(nodes) => nodes
								.iter()
								.map(|input| {
									self.nodes[input.0 as usize]
										.1
										.resolved
										.io
										.as_ref()
										.map(|io| io.ty())
										.ok_or_else(|| format!("input {input:?} of {} is not yet typed", node.identifier.as_str()))
								})
								.collect::<Result<_, _>>()?,
							ConstructionArgs::Inline(inline) => vec![inline.ty.clone()],
							ConstructionArgs::Value(_) => unreachable!(),
						};
						let impls = registry.get(&node.identifier).ok_or_else(|| format!("no implementations for {}", node.identifier.as_str()))?;
						let (io, entry) = resolve_entry(node, &inputs, impls).map_err(|errors| format!("{errors:?}"))?;
						Resolved {
							io: Some(io),
							layout_meta: entry.layout_meta.clone(),
							layout: None,
						}
					}
				}
			};
			self.nodes[index].1.resolved = resolved;
		}
		Ok(())
	}

	pub fn compute_layouts(&mut self) {
		for index in 0..self.nodes.len() {
			let layout = {
				let node = &self.nodes[index].1;
				match &node.construction_args {
					ConstructionArgs::Value(value) => value.value_layout().map(|layout| core_types::record::RecordLayout {
						frame_bytes: layout.frame_bytes(),
						plan: Vec::new(),
						layout,
					}),
					ConstructionArgs::Nodes(inputs) => node.resolved.layout_meta.as_ref().and_then(|meta| {
						let input_layouts: Vec<Option<&core_types::record::Layout>> = inputs
							.iter()
							.map(|input| self.nodes[input.0 as usize].1.resolved.layout.as_ref().map(|resolved| &resolved.layout))
							.collect();
						meta.sources.iter().all(|&source| input_layouts[source as usize].is_some()).then(|| meta.resolve(&input_layouts))
					}),
					ConstructionArgs::Inline(_) => None,
				}
			};
			// Set GRAPHENE_LAYOUT_DEBUG to dump each node's resolved record depth.
			if let Some(resolved) = &layout
				&& std::env::var("GRAPHENE_LAYOUT_DEBUG").is_ok()
			{
				eprintln!("layout {} depth {}", self.nodes[index].1.identifier, resolved.layout.depth);
			}
			self.nodes[index].1.resolved.layout = layout;
		}
		self.stack_need = self.fold_stack_peak();
	}

	/// Peak record-stack bytes for evaluating [`output`](Self::output)'s cone. A node holds its
	/// inputs' frames until it returns, so its need is its own frame plus every input's frame plus
	/// the deepest input's peak. Memoized over shared cones. Runs while node IDs are still indices.
	fn fold_stack_peak(&self) -> usize {
		fn peak(index: usize, network: &ProtoNetwork, memo: &mut [Option<usize>]) -> usize {
			if let Some(cached) = memo[index] {
				return cached;
			}
			let frame = |i: usize| network.nodes[i].1.resolved.layout.as_ref().map_or(0, |resolved| resolved.frame_bytes);
			let mut held = 0;
			let mut deepest = 0;
			if let ConstructionArgs::Nodes(inputs) = &network.nodes[index].1.construction_args {
				for input in inputs {
					let child = input.0 as usize;
					let child_frame = frame(child);
					held += child_frame;
					deepest = deepest.max(peak(child, network, memo).saturating_sub(child_frame));
				}
			}
			let need = frame(index) + held + deepest;
			memo[index] = Some(need);
			need
		}
		let mut memo = vec![None; self.nodes.len()];
		peak(self.output.0 as usize, self, &mut memo)
	}

	/// Inserts context nullification nodes to optimize caching.
	/// This analysis is performed after topological sorting to ensure proper dependency tracking.
	pub fn insert_context_nullification_nodes(&mut self) -> Result<(), String> {
		// Perform topological sort once
		self.reorder_ids()?;

		self.find_context_dependencies(self.output);

		// Perform topological sort a second time to integrate the new nodes
		self.reorder_ids()?;

		Ok(())
	}

	fn insert_context_nullification_node(&mut self, node_id: NodeId, context_deps: ContextModification) -> NodeId {
		let (_, node) = &self.nodes[node_id.0 as usize];
		let mut path = node.original_location.path.clone();

		// Add a path extension with a placeholder value which should not conflict with existing paths
		if let Some(p) = path.as_mut() {
			p.push(NodeId(10))
		}

		let memoize_node_id = NodeId(self.nodes.len() as u64);

		self.nodes.push((
			memoize_node_id,
			ProtoNode {
				construction_args: ConstructionArgs::Nodes(vec![node_id]),
				call_argument: concrete!(Context),
				identifier: graphene_core::memo::memoize::IDENTIFIER,
				original_location: OriginalLocation {
					path: path.clone(),
					..Default::default()
				},
				..Default::default()
			},
		));

		let nullification_value_node_id = NodeId(self.nodes.len() as u64);

		self.nodes.push((
			nullification_value_node_id,
			ProtoNode {
				construction_args: ConstructionArgs::Value(MemoHash::new(TaggedValue::ContextModification(context_deps))),
				call_argument: concrete!(Context),
				identifier: ProtoNodeIdentifier::new("core_types::value::ClonedNode"),
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
				call_argument: concrete!(Context),
				identifier: graphene_core::context_modification::context_modification::IDENTIFIER,
				original_location: OriginalLocation {
					path: path.clone(),
					..Default::default()
				},
				..Default::default()
			},
		));
		nullification_node_id
	}

	/// The node's declared index levels and per-input pushed levels, both read
	/// from the registry since they follow the node's signature. A node with no
	/// registry entry pushes nothing and names no level.
	fn registry_index_levels(&self, node_index: usize) -> (core_types::context::IndexLevels, Vec<u8>) {
		let identifier = &self.nodes[node_index].1.identifier;
		let metadata = core_types::registry::NODE_METADATA.lock().unwrap();
		match metadata.get(identifier) {
			Some(entry) => (
				ContextDependencies::from(entry.context_features.as_slice()).index_levels,
				entry.fields.iter().map(|field| field.pushed_levels).collect(),
			),
			None => (core_types::context::IndexLevels::empty(), Vec::new()),
		}
	}

	fn find_context_dependencies(&mut self, id: NodeId) -> (ContextModification, Option<NodeId>) {
		let mut branch_dependencies = Vec::new();
		let mut combined_deps = ContextModification::default();
		let node_index = id.0 as usize;

		let (declared_levels, pushed_levels) = self.registry_index_levels(node_index);
		let (extract, inject, own_deps) = {
			let dependencies = &self.nodes[node_index].1.context_features;
			let index_levels = match dependencies.extract.contains(core_types::context::ContextFeatures::INDEX) {
				// A wrapper node declaring its dependencies by hand keeps `INDEX`
				// without the signature naming a level, and addresses the innermost.
				true if declared_levels.is_empty() => core_types::context::IndexLevels::innermost(),
				true => declared_levels,
				false => core_types::context::IndexLevels::empty(),
			};
			let own_deps = ContextModification::from_sources(dependencies.extract, dependencies.sources()).with_index_levels(index_levels);
			(dependencies.extract, dependencies.inject, own_deps)
		};

		let mut inputs = match &self.nodes[node_index].1.construction_args {
			// We pretend like we have already placed context modification nodes after ourselves because value nodes don't need to be cached
			ConstructionArgs::Value(_) => return (own_deps, Some(id)),
			ConstructionArgs::Nodes(items) => items.clone(),
			ConstructionArgs::Inline(_) => return (own_deps, Some(id)),
		};

		// Compute the dependencies for each branch and combine all of them
		for (input, &node) in inputs.iter().enumerate() {
			let branch = self.find_context_dependencies(node);

			let mut lifted = branch.0.clone();
			lifted.index_levels = lifted.index_levels.popped(pushed_levels.get(input).copied().unwrap_or(0));
			combined_deps |= &lifted;
			branch_dependencies.push(branch);
		}
		let mut new_deps = combined_deps.clone();

		// Remove requirements which this node provides
		new_deps &= !inject;
		// Add requirements we have
		new_deps |= own_deps;

		// If we either introduce new dependencies, we can cache all children which don't yet need that dependency
		let we_introduce_new_deps = !combined_deps.contains(&new_deps);

		// For diverging branches, we can add a cache node for all branches which don't reqire all dependencies
		for (child_node, (deps, new_id)) in inputs.iter_mut().zip(branch_dependencies) {
			if let Some(new_id) = new_id {
				*child_node = new_id;
			} else if we_introduce_new_deps || deps != combined_deps {
				*child_node = self.insert_context_nullification_node(*child_node, deps);
			}
		}
		self.nodes[node_index].1.construction_args = ConstructionArgs::Nodes(inputs);

		// Which dependencies do we supply (and don't need ourselves)?
		let net_injections = inject.difference(extract);

		// Which dependencies still need to be met after this node?
		let remaining_deps_from_children = combined_deps.features.difference(net_injections);

		// Do we satisfy any existing dependencies?
		let we_supply_existing_deps = !combined_deps.features.difference(remaining_deps_from_children).is_empty();

		let mut new_id = None;
		if we_supply_existing_deps {
			// Our set of context dependencies has shrunk so we can add a cache node after the current node
			new_id = Some(self.insert_context_nullification_node(id, new_deps.clone()));
		}

		(new_deps, new_id)
	}

	/// Update all of the references to a node ID in the graph with a new ID named `compose_node_id`.
	fn replace_node_id(&mut self, outwards_edges: &HashMap<NodeId, Vec<NodeId>>, node_id: NodeId, replacement_node_id: NodeId) {
		// Update references in other nodes to use the new node
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

	// Based on https://en.wikipedia.org/wiki/Topological_sorting#Depth-first_search
	// This approach excludes nodes that are not connected
	pub fn topological_sort(&self) -> Result<(Vec<NodeId>, FxHashMap<NodeId, usize>), String> {
		let (inwards_edges, id_map) = self.collect_inwards_edges_with_mapping();
		let mut sorted = Vec::with_capacity(self.nodes.len());
		let mut stack = vec![id_map[&self.output]];
		let mut state = vec![NodeState::Unvisited; self.nodes.len()];

		while let Some(&node_index) = stack.last() {
			match state[node_index] {
				NodeState::Unvisited => {
					state[node_index] = NodeState::Visiting;
					for &dep_index in inwards_edges[node_index].iter().rev() {
						match state[dep_index] {
							NodeState::Visiting => {
								return Err(format!("Cycle detected involving node {}", self.nodes[dep_index].0));
							}
							NodeState::Unvisited => {
								stack.push(dep_index);
							}
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

		Ok((sorted, id_map))
	}

	fn is_topologically_sorted(&self) -> bool {
		let mut visited = HashSet::new();

		let inwards_edges = self.collect_inwards_edges();
		for (id, _) in &self.nodes {
			for &dependency in inwards_edges.get(id).unwrap_or(&Vec::new()) {
				if !visited.contains(&dependency) {
					dbg!(id, dependency);
					dbg!(&visited);
					dbg!(&self.nodes);
					return false;
				}
			}
			visited.insert(*id);
		}
		true
	}

	/// Sort the nodes vec so it is in a topological order. This ensures that no node takes an input from a node that is found later in the list.
	fn reorder_ids(&mut self) -> Result<(), String> {
		let (order, _id_map) = self.topological_sort()?;

		// // Map of node ids to their current index in the nodes vector
		// let current_positions: FxHashMap<_, _> = self.nodes.iter().enumerate().map(|(pos, (id, _))| (*id, pos)).collect();

		// // Map of node ids to their new index based on topological order
		let new_positions: FxHashMap<_, _> = order.iter().enumerate().map(|(pos, id)| (self.nodes[id.0 as usize].0, pos)).collect();
		// assert_eq!(id_map, current_positions);

		// Create a new nodes vector based on the topological order

		let mut new_nodes = Vec::with_capacity(order.len());
		for (index, &id) in order.iter().enumerate() {
			let mut node = std::mem::take(&mut self.nodes[id.0 as usize].1);
			// Update node references to reflect the new order
			node.map_ids(|id| NodeId(*new_positions.get(&id).expect("node not found in lookup table") as u64));
			new_nodes.push((NodeId(index as u64), node));
		}

		// Update node references to reflect the new order
		// new_nodes.iter_mut().for_each(|(_, node)| {
		// 	node.map_ids(|id| *new_positions.get(&id).expect("node not found in lookup table"), false);
		// });

		// Update the nodes vector and other references
		self.nodes = new_nodes;
		self.inputs = self.inputs.iter().filter_map(|id| new_positions.get(id).map(|x| NodeId(*x as u64))).collect();
		self.output = NodeId(*new_positions.get(&self.output).unwrap() as u64);

		assert_eq!(order.len(), self.nodes.len());
		Ok(())
	}
}
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GraphErrorType {
	NodeNotFound(NodeId),
	UnexpectedGenerics {
		index: usize,
		inputs: Vec<Type>,
	},
	NoImplementations,
	NoConstructor,
	ConstructionFailed(String),
	/// The `inputs` represents a formatted list of input indices corresponding to their types.
	/// Each element in `error_inputs` represents a valid `NodeIOTypes` implementation.
	/// The inner Vec stores the inputs which need to be changed and what type each needs to be changed to.
	InvalidImplementations {
		inputs: String,
		error_inputs: Vec<Vec<(usize, (Type, Type))>>,
	},
	MultipleImplementations {
		inputs: String,
		valid: Vec<NodeIOTypes>,
	},
}
impl Debug for GraphErrorType {
	// TODO: format with the document graph context so the input index is the same as in the graph UI.
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			GraphErrorType::NodeNotFound(id) => write!(f, "Input node {id} is not present in the typing context"),
			GraphErrorType::UnexpectedGenerics { index, inputs } => write!(f, "Generic inputs should not exist but found at {index}: {inputs:?}"),
			GraphErrorType::NoImplementations => write!(f, "No implementations found"),
			GraphErrorType::NoConstructor => write!(f, "No construct found for node"),
			GraphErrorType::ConstructionFailed(error) => write!(f, "Construction failed: {error}"),
			GraphErrorType::InvalidImplementations { inputs, error_inputs } => {
				let format_error = |(index, (found, expected)): &(usize, (Type, Type))| {
					let index = index + 1;
					format!(
						"\
						• Input {index}:\n\
						…found:       {found}\n\
						…expected: {expected}\
						"
					)
				};
				let format_error_list = |errors: &Vec<(usize, (Type, Type))>| errors.iter().map(format_error).collect::<Vec<_>>().join("\n");
				let mut errors = error_inputs.iter().map(format_error_list).collect::<Vec<_>>();
				errors.sort();
				let errors = errors.join("\n");
				let incompatibility = if errors.chars().filter(|&c| c == '•').count() == 1 {
					"This input type is incompatible:"
				} else {
					"These input types are incompatible:"
				};

				write!(
					f,
					"\
					{incompatibility}\n\
					{errors}\n\
					\n\
					The node is currently receiving all of the following input types:\n\
					{inputs}\n\
					This is not a supported arrangement of types for the node.\
					"
				)
			}
			GraphErrorType::MultipleImplementations { inputs, valid } => write!(f, "Multiple implementations found ({inputs}):\n{valid:#?}"),
		}
	}
}
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GraphError {
	pub node_path: Vec<NodeId>,
	pub identifier: Cow<'static, str>,
	pub error: GraphErrorType,
}
impl GraphError {
	pub fn new(node: &ProtoNode, text: impl Into<GraphErrorType>) -> Self {
		Self {
			node_path: node.original_location.path.clone().unwrap_or_default(),
			identifier: Cow::Owned(node.identifier.as_str().to_string()),
			error: text.into(),
		}
	}
}
impl Debug for GraphError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("GraphError")
			.field("node_path", &self.node_path.iter().map(|id| id.0).collect::<Vec<_>>())
			.field("identifier", &self.identifier.to_string())
			.field("error", &self.error)
			.finish()
	}
}
pub type GraphErrors = Vec<GraphError>;

pub type Registry = HashMap<ProtoNodeIdentifier, Vec<RegistryEntry>>;

/// The `TypingContext` is used to store the types of the nodes indexed by their stable node id.
#[derive(Default, Clone, dyn_any::DynAny)]
pub struct TypingContext {
	lookup: Cow<'static, Registry>,
	inferred: HashMap<NodeId, NodeIOTypes>,
	constructor: HashMap<NodeId, NodeConstructor>,
}

impl TypingContext {
	/// Creates a new `TypingContext` with the given lookup table.
	pub fn new(lookup: &'static Registry) -> Self {
		Self {
			lookup: Cow::Borrowed(lookup),
			..Default::default()
		}
	}

	pub fn registry(&self) -> &Registry {
		&self.lookup
	}

	/// Updates the `TypingContext` with a given proto network. This will infer the types of the nodes
	/// and store them in the `inferred` field. The proto network has to be topologically sorted
	/// and contain fully resolved stable node ids.
	pub fn update(&mut self, network: &mut ProtoNetwork) -> Result<(), GraphErrors> {
		for (id, node) in &network.nodes {
			self.infer(*id, node)?;
		}

		Ok(())
	}

	pub fn remove_inference(&mut self, node_id: NodeId) -> Option<NodeIOTypes> {
		self.constructor.remove(&node_id);
		self.inferred.remove(&node_id)
	}

	/// Returns the node constructor for a given node id.
	pub fn constructor(&self, node_id: NodeId) -> Option<NodeConstructor> {
		self.constructor.get(&node_id).copied()
	}

	/// Returns the type of a given node id if it exists
	pub fn type_of(&self, node_id: NodeId) -> Option<&NodeIOTypes> {
		self.inferred.get(&node_id)
	}

	/// Returns the inferred types for a given node id.
	pub fn infer(&mut self, node_id: NodeId, node: &ProtoNode) -> Result<NodeIOTypes, GraphErrors> {
		// Return the inferred type if it is already known
		if let Some(inferred) = self.inferred.get(&node_id) {
			return Ok(inferred.clone());
		}

		let inputs = match node.construction_args {
			// A value node is a native record source, so it types as a record
			// of its value.
			ConstructionArgs::Value(ref v) => {
				let types = NodeIOTypes::new(concrete!(Context), Type::Record(Box::new(v.ty())), vec![]);
				self.inferred.insert(node_id, types.clone());
				return Ok(types);
			}
			// If the node has nodes as inputs we can infer the types from the node outputs
			ConstructionArgs::Nodes(ref nodes) => nodes
				.iter()
				.map(|id| {
					self.inferred
						.get(id)
						.ok_or_else(|| vec![GraphError::new(node, GraphErrorType::NodeNotFound(*id))])
						.map(|node| node.ty())
				})
				.collect::<Result<Vec<Type>, GraphErrors>>()?,
			ConstructionArgs::Inline(ref inline) => vec![inline.ty.clone()],
		};

		// Get the node input type from the proto node declaration
		let impls = self.lookup.get(&node.identifier).ok_or_else(|| vec![GraphError::new(node, GraphErrorType::NoImplementations)])?;
		let (node_io, entry) = resolve_entry(node, &inputs, impls)?;
		if std::env::var("GRAPHENE_TYPE_DEBUG").is_ok() {
			eprintln!("type {} {} -> {}", node_id, node.identifier, node_io.ty());
		}
		self.inferred.insert(node_id, node_io.clone());
		self.constructor.insert(node_id, entry.constructor);
		Ok(node_io)
	}
}

/// Selects the single registry entry matching the node's resolved input types,
/// substituting generics. Stateless and stable-id-free.
fn resolve_entry<'a>(node: &ProtoNode, inputs: &[Type], impls: &'a [RegistryEntry]) -> Result<(NodeIOTypes, &'a RegistryEntry), GraphErrors> {
	let call_argument = &node.call_argument;
	let candidates: Vec<(NodeIOTypes, &RegistryEntry)> = impls.iter().map(|entry| (entry.io.clone(), entry)).collect();

	if let Some(index) = inputs.iter().position(|p| {
		matches!(p,
		Type::Fn(_, b) if matches!(b.as_ref(), Type::Generic(_)))
	}) {
		return Err(vec![GraphError::new(node, GraphErrorType::UnexpectedGenerics { index, inputs: inputs.to_vec() })]);
	}

	// List of all implementations that match the input types
	let valid_output_types = candidates
		.iter()
		.filter(|(node_io, _)| valid_type(&node_io.call_argument, call_argument) && inputs.iter().zip(node_io.inputs.iter()).all(|(p1, p2)| valid_type(p1, p2)))
		.collect::<Vec<_>>();

	// Attempt to substitute generic types with concrete types and save the list of results
	let substitution_results = valid_output_types
		.iter()
		.map(|(node_io, entry)| {
			let generics_lookup: Result<HashMap<_, _>, _> = collect_generics(node_io)
				.iter()
				.map(|generic| check_generic(node_io, call_argument, inputs, generic).map(|x| (generic.to_string(), x)))
				.collect();

			generics_lookup.map(|generics_lookup| {
				let mut new_node_io = node_io.clone();
				replace_generics(&mut new_node_io, &generics_lookup);
				(new_node_io, *entry)
			})
		})
		.collect::<Vec<_>>();

	// Collect all substitutions that are valid
	let valid_impls = substitution_results.iter().filter_map(|result| result.as_ref().ok()).collect::<Vec<_>>();

	match valid_impls.as_slice() {
		[] => {
			let convert_node_index_offset = node.original_location.auto_convert_index.unwrap_or(0);
			let mut best_errors = usize::MAX;
			let mut error_inputs = Vec::new();
			for (node_io, _) in &candidates {
				// For errors on Convert nodes, offset the input index so it correctly corresponds to the node it is connected to.
				let current_errors = [call_argument]
					.into_iter()
					.chain(inputs)
					.cloned()
					.zip([&node_io.call_argument].into_iter().chain(&node_io.inputs).cloned())
					.enumerate()
					.filter(|(_, (p1, p2))| !valid_type(p1, p2))
					.map(|(index, expected)| (index - 1 + convert_node_index_offset, expected))
					.collect::<Vec<_>>();
				if current_errors.len() < best_errors {
					best_errors = current_errors.len();
					error_inputs.clear();
				}
				if current_errors.len() <= best_errors {
					error_inputs.push(current_errors);
				}
			}
			let inputs = [call_argument]
				.into_iter()
				.chain(inputs)
				.enumerate()
				.filter_map(|(i, t)| {
					if i == 0 {
						None
					} else {
						let number = i + convert_node_index_offset;
						Some(format!("• Input {number}: {t}"))
					}
				})
				.collect::<Vec<_>>()
				.join("\n");
			Err(vec![GraphError::new(node, GraphErrorType::InvalidImplementations { inputs, error_inputs })])
		}
		[(node_io, entry)] => Ok((node_io.clone(), *entry)),
		// If two types are available and one of them accepts () an input, always choose that one
		[first, second] => {
			if first.0.call_argument != second.0.call_argument {
				for (node_io, entry) in [first, second] {
					if node_io.call_argument != concrete!(()) {
						continue;
					}
					return Ok((node_io.clone(), *entry));
				}
			}
			let inputs = [call_argument].into_iter().chain(inputs).map(ToString::to_string).collect::<Vec<_>>().join(", ");
			let valid = valid_output_types.into_iter().map(|(node_io, _)| node_io.clone()).collect();
			Err(vec![GraphError::new(node, GraphErrorType::MultipleImplementations { inputs, valid })])
		}

		_ => {
			let inputs = [call_argument].into_iter().chain(inputs).map(ToString::to_string).collect::<Vec<_>>().join(", ");
			let valid = valid_output_types.into_iter().map(|(node_io, _)| node_io.clone()).collect();
			Err(vec![GraphError::new(node, GraphErrorType::MultipleImplementations { inputs, valid })])
		}
	}
}

/// Checks if a proposed input to a particular (primary or secondary) input connector is valid for its type signature.
/// `from` indicates the value given to a input, `to` indicates the input's allowed type as specified by its type signature.
fn valid_type(from: &Type, to: &Type) -> bool {
	match (from, to) {
		// Direct comparison of two concrete types.
		(Type::Concrete(type1), Type::Concrete(type2)) => type1 == type2,
		// Direct comparison of two function types.
		// Note: in the presence of subtyping, functions are considered on a "greater than or equal to" basis of its function type's generality.
		// That means we compare their types with a contravariant relationship, which means that a more general type signature may be substituted for a more specific type signature.
		// For example, we allow `T -> V` to be substituted with `T' -> V` or `() -> V` where T' and () are more specific than T.
		// This allows us to supply anything to a function that is satisfied with `()`.
		// In other words, we are implementing these two relations, where the >= operator means that the left side is more general than the right side:
		// - `T >= T' ⇒ (T' -> V) >= (T -> V)` (functions are contravariant in their input types)
		// - `V >= V' ⇒ (T -> V) >= (T -> V')` (functions are covariant in their output types)
		// While these two relations aren't a truth about the universe, they are a design decision that we are employing in our language design that is also common in other languages.
		// For example, Rust implements these same relations as it describes here: <https://doc.rust-lang.org/nomicon/subtyping.html>
		// Graphite doesn't have subtyping currently, but it used to have it, and may do so again, so we make sure to compare types in this way to make things easier.
		// More details explained here: <https://github.com/GraphiteEditor/Graphite/issues/1741>
		(Type::Fn(in1, out1), Type::Fn(in2, out2)) => valid_type(out2, out1) && valid_type(in1, in2),
		// A lend edge is substitutable exactly when the lent values are.
		// A record edge is substitutable exactly when the elements are.
		(Type::Record(in1), Type::Record(in2)) => valid_type(in1, in2),
		// If either the proposed input or the allowed input are generic, we allow the substitution (meaning this is a valid subtype).
		// TODO: Add proper generic counting which is not based on the name
		(Type::Generic(_), _) | (_, Type::Generic(_)) => true,
		// Reject unknown type relationships.
		_ => false,
	}
}

/// Returns a list of all generic types used in the node
fn collect_generics(types: &NodeIOTypes) -> Vec<Cow<'static, str>> {
	let inputs = [&types.call_argument].into_iter().chain(types.inputs.iter().map(|x| x.nested_type()));
	let mut generics = inputs
		.filter_map(|t| match t {
			Type::Generic(out) => Some(out.clone()),
			_ => None,
		})
		.collect::<Vec<_>>();
	if let Type::Generic(out) = &types.return_value {
		generics.push(out.clone());
	}
	generics.dedup();
	generics
}

/// Checks if a generic type can be substituted with a concrete type and returns the concrete type
fn check_generic(types: &NodeIOTypes, input: &Type, parameters: &[Type], generic: &str) -> Result<Type, String> {
	fn record_element(ty: Option<&Type>) -> Option<&Type> {
		match ty {
			Some(Type::Record(inner)) => Some(inner.as_ref()),
			_ => None,
		}
	}
	let inputs = [(Some(&types.call_argument), Some(input))]
		.into_iter()
		.chain(types.inputs.iter().map(|x| x.fn_input()).zip(parameters.iter().map(|x| x.fn_input())))
		.chain(types.inputs.iter().map(|x| x.fn_output()).zip(parameters.iter().map(|x| x.fn_output())))
		.chain(types.inputs.iter().map(|x| record_element(x.fn_output())).zip(parameters.iter().map(|x| record_element(x.fn_output()))));
	let concrete_inputs = inputs.filter(|(ni, _)| matches!(ni, Some(Type::Generic(input)) if generic == input));
	let mut outputs = concrete_inputs.flat_map(|(_, out)| out);
	let out_ty = outputs
		.next()
		.ok_or_else(|| format!("Generic output type {generic} is not dependent on input {input:?} or parameters {parameters:?}",))?;
	if outputs.any(|ty| ty != out_ty) {
		return Err(format!("Generic output type {generic} is dependent on multiple inputs or parameters",));
	}
	Ok(out_ty.clone())
}

/// Returns a list of all generic types used in the node
fn replace_generics(types: &mut NodeIOTypes, lookup: &HashMap<String, Type>) {
	let replace = |ty: &Type| {
		let Type::Generic(ident) = ty else { return None };
		lookup.get(ident.as_ref()).cloned()
	};
	types.call_argument.replace_nested(replace);
	types.return_value.replace_nested(replace);
	for input in &mut types.inputs {
		input.replace_nested(replace);
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::proto::{ConstructionArgs, ProtoNetwork, ProtoNode};

	#[test]
	fn stack_peak_folds_a_diamond_chain() {
		// S3 <- S2 <- S1 <- S0, each consuming the node below on both inputs; every frame is one byte.
		let node = |index: u64| {
			let args = if index == 0 {
				ConstructionArgs::Value(value::TaggedValue::U32(0).into())
			} else {
				ConstructionArgs::Nodes(vec![NodeId(index - 1), NodeId(index - 1)])
			};
			ProtoNode {
				construction_args: args,
				resolved: Resolved {
					layout: Some(core_types::record::RecordLayout { frame_bytes: 1, ..Default::default() }),
					..Default::default()
				},
				..Default::default()
			}
		};
		let mut network = ProtoNetwork {
			output: NodeId(3),
			nodes: (0..4).map(|index| (NodeId(index), node(index))).collect(),
			..Default::default()
		};
		assert_eq!(network.fold_stack_peak(), 7);
		network.output = NodeId(0);
		assert_eq!(network.fold_stack_peak(), 1);
	}

	#[test]
	fn topological_sort() {
		let construction_network = test_network();
		let (sorted, _) = construction_network.topological_sort().expect("Error when calling 'topological_sort' on 'construction_network.");
		let sorted: Vec<_> = sorted.iter().map(|x| construction_network.nodes[x.0 as usize].0).collect();
		println!("{sorted:#?}");
		assert_eq!(sorted, vec![NodeId(14), NodeId(10), NodeId(11), NodeId(1)]);
	}

	#[test]
	fn topological_sort_with_cycles() {
		let construction_network = test_network_with_cycles();
		let sorted = construction_network.topological_sort();

		assert!(sorted.is_err())
	}

	#[test]
	fn id_reordering() {
		let mut construction_network = test_network();
		construction_network.reorder_ids().expect("Error when calling 'reorder_ids' on 'construction_network.");
		let (sorted, _) = construction_network.topological_sort().expect("Error when calling 'topological_sort' on 'construction_network.");
		let sorted: Vec<_> = sorted.iter().map(|x| construction_network.nodes[x.0 as usize].0).collect();
		println!("nodes: {:#?}", construction_network.nodes);
		assert_eq!(sorted, vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)]);
		let ids: Vec<_> = construction_network.nodes.iter().map(|(id, _)| *id).collect();
		println!("{ids:#?}");
		println!("nodes: {:#?}", construction_network.nodes);
		assert_eq!(construction_network.nodes[0].1.identifier.as_str(), "value");
		assert_eq!(ids, vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)]);
	}

	#[test]
	fn id_reordering_idempotent() {
		let mut construction_network = test_network();
		construction_network.reorder_ids().expect("Error when calling 'reorder_ids' on 'construction_network.");
		construction_network.reorder_ids().expect("Error when calling 'reorder_ids' on 'construction_network.");
		let (sorted, _) = construction_network.topological_sort().expect("Error when calling 'topological_sort' on 'construction_network.");
		assert_eq!(sorted, vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)]);
		let ids: Vec<_> = construction_network.nodes.iter().map(|(id, _)| *id).collect();
		println!("{ids:#?}");
		assert_eq!(construction_network.nodes[0].1.identifier.as_str(), "value");
		assert_eq!(ids, vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)]);
	}

	#[test]
	fn stable_node_id_generation() {
		let mut construction_network = test_network();
		construction_network
			.insert_context_nullification_nodes()
			.expect("Error when calling 'insert_context_nullification_nodes' on 'construction_network.");
		construction_network.generate_stable_node_ids();
		assert_eq!(construction_network.nodes[0].1.identifier.as_str(), "value");
		let ids: Vec<_> = construction_network.nodes.iter().map(|(id, _)| *id).collect();

		// If this assert fails: These NodeIds seem to be changing when you modify TaggedValue, just update them.
		assert_eq!(
			ids,
			vec![NodeId(12815475172301479638), NodeId(13251389748338817266), NodeId(7166921994790432021), NodeId(15318519137317483318)]
		);
	}

	#[test]
	fn retain_filter_placement_on_source_free_branch() {
		let mut network = source_branch_network(vec![1], vec![]);
		network.insert_context_nullification_nodes().expect("Error when calling 'insert_context_nullification_nodes'");

		let filters = nullification_filters(&network);
		assert_eq!(filters.len(), 1, "only the source-free branch gets a filter");
		let (filter_id, wrapped, retained) = &filters[0];
		assert_eq!(wrapped, "source_b");
		assert!(retained.is_empty(), "the source-free branch retains no sources");

		let (source_a_id, _) = find_node(&network, "source_a");
		let (_, join) = find_node(&network, "join");
		let ConstructionArgs::Nodes(join_args) = &join.construction_args else {
			panic!("join args must be nodes")
		};
		assert_eq!(join_args, &vec![source_a_id, *filter_id], "the source branch stays direct, the filter replaces the source-free branch");
	}

	#[test]
	fn diverging_source_sets_filter_each_branch() {
		let mut network = source_branch_network(vec![1], vec![2]);
		network.insert_context_nullification_nodes().expect("Error when calling 'insert_context_nullification_nodes'");

		let mut filters = nullification_filters(&network);
		filters.sort_by(|(_, a, _), (_, b, _)| a.cmp(b));
		let summary: Vec<_> = filters.iter().map(|(_, wrapped, retained)| (wrapped.as_str(), retained.as_slice())).collect();
		assert_eq!(
			summary,
			vec![("source_a", &[1u64][..]), ("source_b", &[2u64][..])],
			"each diverging branch is filtered down to its own source set"
		);
	}

	#[test]
	fn matching_source_sets_insert_no_filter() {
		let mut network = source_branch_network(vec![1], vec![1]);
		network.insert_context_nullification_nodes().expect("Error when calling 'insert_context_nullification_nodes'");

		assert!(nullification_filters(&network).is_empty(), "equal branch source sets need no filter");
	}

	fn find_node<'a>(network: &'a ProtoNetwork, name: &str) -> (NodeId, &'a ProtoNode) {
		network
			.nodes
			.iter()
			.find(|(_, node)| node.identifier.as_str() == name)
			.map(|(id, node)| (*id, node))
			.unwrap_or_else(|| panic!("node {name} not found"))
	}

	fn nullification_filters(network: &ProtoNetwork) -> Vec<(NodeId, String, Vec<SourceId>)> {
		let node = |id: NodeId| &network.nodes[id.0 as usize].1;
		network
			.nodes
			.iter()
			.filter(|(_, candidate)| candidate.identifier.as_str() == graphene_core::context_modification::context_modification::IDENTIFIER.as_str())
			.map(|(id, candidate)| {
				let ConstructionArgs::Nodes(args) = &candidate.construction_args else {
					panic!("filter args must be nodes")
				};
				let ConstructionArgs::Nodes(memoized) = &node(args[0]).construction_args else {
					panic!("filter memoize args must be nodes")
				};
				let ConstructionArgs::Value(value) = &node(args[1]).construction_args else {
					panic!("filter payload must be a value")
				};
				let value::TaggedValue::ContextModification(modification) = &**value else {
					panic!("filter payload must be a context modification")
				};
				(*id, node(memoized[0]).identifier.as_str().to_string(), modification.sources().to_vec())
			})
			.collect()
	}

	fn test_network() -> ProtoNetwork {
		ProtoNetwork {
			inputs: vec![NodeId(10)],
			output: NodeId(1),
			nodes: [
				(
					NodeId(7),
					ProtoNode {
						identifier: ProtoNodeIdentifier::new("id"),
						call_argument: concrete!(()),
						construction_args: ConstructionArgs::Nodes(vec![NodeId(11)]),
						..Default::default()
					},
				),
				(
					NodeId(1),
					ProtoNode {
						identifier: ProtoNodeIdentifier::new("id"),
						call_argument: concrete!(()),
						construction_args: ConstructionArgs::Nodes(vec![NodeId(11)]),
						..Default::default()
					},
				),
				(
					NodeId(10),
					ProtoNode {
						identifier: ProtoNodeIdentifier::new("cons"),
						call_argument: concrete!(u32),
						construction_args: ConstructionArgs::Nodes(vec![NodeId(14)]),
						..Default::default()
					},
				),
				(
					NodeId(11),
					ProtoNode {
						identifier: ProtoNodeIdentifier::new("add"),
						call_argument: concrete!(()),
						construction_args: ConstructionArgs::Nodes(vec![NodeId(10)]),
						..Default::default()
					},
				),
				(
					NodeId(14),
					ProtoNode {
						identifier: ProtoNodeIdentifier::new("value"),
						call_argument: concrete!(()),
						construction_args: ConstructionArgs::Value(value::TaggedValue::U32(2).into()),
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		}
	}

	fn source_branch_network(branch_a_sources: Vec<SourceId>, branch_b_sources: Vec<SourceId>) -> ProtoNetwork {
		let branch = |name: &str, sources: Vec<SourceId>| ProtoNode {
			identifier: ProtoNodeIdentifier::with_owned_string(name.to_string()),
			call_argument: concrete!(()),
			construction_args: ConstructionArgs::Nodes(vec![NodeId(0)]),
			context_features: ContextDependencies::from_sources(&sources),
			..Default::default()
		};
		ProtoNetwork {
			inputs: vec![],
			output: NodeId(3),
			nodes: [
				(
					NodeId(0),
					ProtoNode {
						identifier: ProtoNodeIdentifier::new("value"),
						call_argument: concrete!(()),
						construction_args: ConstructionArgs::Value(value::TaggedValue::U32(2).into()),
						..Default::default()
					},
				),
				(NodeId(1), branch("source_a", branch_a_sources)),
				(NodeId(2), branch("source_b", branch_b_sources)),
				(
					NodeId(3),
					ProtoNode {
						identifier: ProtoNodeIdentifier::new("join"),
						call_argument: concrete!(()),
						construction_args: ConstructionArgs::Nodes(vec![NodeId(1), NodeId(2)]),
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		}
	}

	fn test_network_with_cycles() -> ProtoNetwork {
		ProtoNetwork {
			inputs: vec![NodeId(1)],
			output: NodeId(1),
			nodes: [
				(
					NodeId(1),
					ProtoNode {
						identifier: ProtoNodeIdentifier::new("id"),
						call_argument: concrete!(()),
						construction_args: ConstructionArgs::Nodes(vec![NodeId(2)]),
						..Default::default()
					},
				),
				(
					NodeId(2),
					ProtoNode {
						identifier: ProtoNodeIdentifier::new("id"),
						call_argument: concrete!(()),
						construction_args: ConstructionArgs::Nodes(vec![NodeId(1)]),
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		}
	}
}
