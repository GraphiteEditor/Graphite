use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use xxhash_rust::xxh3::Xxh3;

use crate::document::value;
use crate::document::NodeId;
use dyn_any::DynAny;
use graphene_core::*;
use std::pin::Pin;

pub type Any<'n> = Box<dyn DynAny<'n> + 'n>;
pub type TypeErasedNode<'n> = dyn for<'i> NodeIO<'i, Any<'i>, Output = Any<'i>> + 'n + Send + Sync;
pub type TypeErasedPinnedRef<'n> = Pin<&'n (dyn for<'i> NodeIO<'i, Any<'i>, Output = Any<'i>> + 'n + Send + Sync)>;
pub type TypeErasedPinned<'n> = Pin<Box<dyn for<'i> NodeIO<'i, Any<'i>, Output = Any<'i>> + 'n + Send + Sync>>;

pub type NodeConstructor = for<'a> fn(Vec<TypeErasedPinnedRef<'static>>) -> TypeErasedPinned<'static>;

#[derive(Debug, Default, PartialEq)]
pub struct ProtoNetwork {
	// Should a proto Network even allow inputs? Don't think so
	pub inputs: Vec<NodeId>,
	pub output: NodeId,
	pub nodes: Vec<(NodeId, ProtoNode)>,
}

impl core::fmt::Display for ProtoNetwork {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.write_str("Proto Network with nodes: ")?;
		fn write_node(f: &mut core::fmt::Formatter<'_>, network: &ProtoNetwork, id: NodeId, indent: usize) -> core::fmt::Result {
			f.write_str(&"\t".repeat(indent))?;
			let Some((_, node)) = network.nodes.iter().find(|(node_id, _)|*node_id == id) else{
				return f.write_str("{{Unknown Node}}");
			};
			f.write_str("Node: ")?;
			f.write_str(&node.identifier.name)?;

			f.write_str("\n")?;
			f.write_str(&"\t".repeat(indent))?;
			f.write_str("{\n")?;

			f.write_str(&"\t".repeat(indent + 1))?;
			f.write_str("Primary input: ")?;
			match &node.input {
				ProtoNodeInput::None => f.write_str("None")?,
				ProtoNodeInput::Network(ty) => f.write_fmt(format_args!("Network (type = {:?})", ty))?,
				ProtoNodeInput::ShortCircut(ty) => f.write_fmt(format_args!("Lambda (type = {:?})", ty))?,
				ProtoNodeInput::Node(_, _) => f.write_str("Node")?,
			}
			f.write_str("\n")?;

			match &node.construction_args {
				ConstructionArgs::Value(value) => {
					f.write_str(&"\t".repeat(indent + 1))?;
					f.write_fmt(format_args!("Value construction argument: {value:?}"))?
				}
				ConstructionArgs::Nodes(nodes) => {
					for id in nodes {
						write_node(f, network, id.0, indent + 1)?;
					}
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

#[derive(Debug, Clone)]
pub enum ConstructionArgs {
	Value(value::TaggedValue),
	// the bool indicates whether to treat the node as lambda node
	Nodes(Vec<(NodeId, bool)>),
}

impl PartialEq for ConstructionArgs {
	fn eq(&self, other: &Self) -> bool {
		match (&self, &other) {
			(Self::Nodes(n1), Self::Nodes(n2)) => n1 == n2,
			(Self::Value(v1), Self::Value(v2)) => v1 == v2,
			_ => core::mem::discriminant(self) == core::mem::discriminant(other),
		}
	}
}

impl Hash for ConstructionArgs {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		match self {
			Self::Nodes(nodes) => {
				"nodes".hash(state);
				for node in nodes {
					node.hash(state);
				}
			}
			Self::Value(value) => value.hash(state),
		}
	}
}

impl ConstructionArgs {
	pub fn new_function_args(&self) -> Vec<String> {
		match self {
			ConstructionArgs::Nodes(nodes) => nodes.iter().map(|n| format!("n{}", n.0)).collect(),
			ConstructionArgs::Value(value) => vec![format!("{:?}", value)],
		}
	}
}

#[derive(Debug, PartialEq, Clone)]
pub struct ProtoNode {
	pub construction_args: ConstructionArgs,
	pub input: ProtoNodeInput,
	pub identifier: NodeIdentifier,
}

/// A ProtoNodeInput represents the input of a node in a ProtoNetwork.
/// For documentation on the meaning of the variants, see the documentation of the `NodeInput` enum
/// in the `document` module
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ProtoNodeInput {
	None,
	Network(Type),
	/// A ShortCircut input represents an input that is not resolved through function composition but
	/// actually consuming the provided input instead of passing it to its predecessor
	ShortCircut(Type),
	/// the bool indicates whether to treat the node as lambda node.
	/// When treating it as a lambda, only the node that is connected itself is fed as input.
	/// Otherwise, the the entire network of which the node is the output is fed as input.
	Node(NodeId, bool),
}

impl ProtoNodeInput {
	pub fn unwrap_node(self) -> NodeId {
		match self {
			ProtoNodeInput::Node(id, _) => id,
			_ => panic!("tried to unwrap id from non node input \n node: {:#?}", self),
		}
	}
}

impl ProtoNode {
	pub fn stable_node_id(&self) -> Option<NodeId> {
		use std::hash::Hasher;
		let mut hasher = Xxh3::new();

		self.identifier.name.hash(&mut hasher);
		self.construction_args.hash(&mut hasher);
		match self.input {
			ProtoNodeInput::None => "none".hash(&mut hasher),
			ProtoNodeInput::ShortCircut(ref ty) => {
				"lambda".hash(&mut hasher);
				ty.hash(&mut hasher);
			}
			ProtoNodeInput::Network(ref ty) => {
				"network".hash(&mut hasher);
				ty.hash(&mut hasher);
			}
			ProtoNodeInput::Node(id, lambda) => (id, lambda).hash(&mut hasher),
		};
		Some(hasher.finish() as NodeId)
	}

	pub fn value(value: ConstructionArgs) -> Self {
		Self {
			identifier: NodeIdentifier::new("graphene_core::value::ValueNode"),
			construction_args: value,
			input: ProtoNodeInput::None,
		}
	}

	pub fn map_ids(&mut self, f: impl Fn(NodeId) -> NodeId, skip_lambdas: bool) {
		if let ProtoNodeInput::Node(id, lambda) = self.input {
			if !(skip_lambdas && lambda) {
				self.input = ProtoNodeInput::Node(f(id), lambda)
			}
		}
		if let ConstructionArgs::Nodes(ids) = &mut self.construction_args {
			ids.iter_mut().filter(|(_, lambda)| !(skip_lambdas && *lambda)).for_each(|(id, _)| *id = f(*id));
		}
	}

	pub fn unwrap_construction_nodes(&self) -> Vec<(NodeId, bool)> {
		match &self.construction_args {
			ConstructionArgs::Nodes(nodes) => nodes.clone(),
			_ => panic!("tried to unwrap nodes from non node construction args \n node: {:#?}", self),
		}
	}
}

impl ProtoNetwork {
	fn check_ref(&self, ref_id: &NodeId, id: &NodeId) {
		assert!(
			self.nodes.iter().any(|(check_id, _)| check_id == ref_id),
			"Node id:{} has a reference which uses node id:{} which doesn't exist in network {:#?}",
			id,
			ref_id,
			self
		);
	}

	pub fn collect_outwards_edges(&self) -> HashMap<NodeId, Vec<NodeId>> {
		let mut edges: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
		for (id, node) in &self.nodes {
			if let ProtoNodeInput::Node(ref_id, _) = &node.input {
				self.check_ref(ref_id, id);
				edges.entry(*ref_id).or_default().push(*id)
			}
			if let ConstructionArgs::Nodes(ref_nodes) = &node.construction_args {
				for (ref_id, _) in ref_nodes {
					self.check_ref(ref_id, id);
					edges.entry(*ref_id).or_default().push(*id)
				}
			}
		}
		edges
	}

	pub fn generate_stable_node_ids(&mut self) {
		for i in 0..self.nodes.len() {
			self.generate_stable_node_id(i);
		}
	}

	pub fn generate_stable_node_id(&mut self, index: usize) -> NodeId {
		let mut lookup = self.nodes.iter().map(|(id, _)| (*id, *id)).collect::<HashMap<_, _>>();
		if let Some(sni) = self.nodes[index].1.stable_node_id() {
			lookup.insert(self.nodes[index].0, sni);
			self.replace_node_references(&lookup, false);
			self.nodes[index].0 = sni;
			sni
		} else {
			panic!("failed to generate stable node id for node {:#?}", self.nodes[index].1);
		}
	}

	pub fn collect_inwards_edges(&self) -> HashMap<NodeId, Vec<NodeId>> {
		let mut edges: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
		for (id, node) in &self.nodes {
			if let ProtoNodeInput::Node(ref_id, _) = &node.input {
				self.check_ref(ref_id, id);
				edges.entry(*id).or_default().push(*ref_id)
			}
			if let ConstructionArgs::Nodes(ref_nodes) = &node.construction_args {
				for (ref_id, _) in ref_nodes {
					self.check_ref(ref_id, id);
					edges.entry(*id).or_default().push(*ref_id)
				}
			}
		}
		edges
	}

	pub fn resolve_inputs(&mut self) {
		let mut resolved = HashSet::new();
		while !self.resolve_inputs_impl(&mut resolved) {}
	}
	fn resolve_inputs_impl(&mut self, resolved: &mut HashSet<NodeId>) -> bool {
		self.reorder_ids();

		let mut lookup = self.nodes.iter().map(|(id, _)| (*id, *id)).collect::<HashMap<_, _>>();
		let compose_node_id = self.nodes.len() as NodeId;
		let inputs = self.nodes.iter().map(|(_, node)| node.input.clone()).collect::<Vec<_>>();

		let resolved_lookup = resolved.clone();
		if let Some((input_node, id, input)) = self.nodes.iter_mut().filter(|(id, _)| !resolved_lookup.contains(id)).find_map(|(id, node)| {
			if let ProtoNodeInput::Node(input_node, false) = node.input {
				resolved.insert(*id);
				let pre_node_input = inputs.get(input_node as usize).expect("input node should exist");
				Some((input_node, *id, pre_node_input.clone()))
			} else {
				resolved.insert(*id);
				None
			}
		}) {
			lookup.insert(id, compose_node_id);
			self.replace_node_references(&lookup, true);
			self.nodes.push((
				compose_node_id,
				ProtoNode {
					identifier: NodeIdentifier::new("graphene_core::structural::ComposeNode<_, _, _>"),
					construction_args: ConstructionArgs::Nodes(vec![(input_node, false), (id, true)]),
					input,
				},
			));
			return false;
		}

		true
	}

	// Based on https://en.wikipedia.org/wiki/Topological_sorting#Depth-first_search
	// This approach excludes nodes that are not connected
	pub fn topological_sort(&self) -> Vec<NodeId> {
		let mut sorted = Vec::new();
		let inwards_edges = self.collect_inwards_edges();
		fn visit(node_id: NodeId, temp_marks: &mut HashSet<NodeId>, sorted: &mut Vec<NodeId>, inwards_edges: &HashMap<NodeId, Vec<NodeId>>) {
			if sorted.contains(&node_id) {
				return;
			};
			if temp_marks.contains(&node_id) {
				panic!("Cycle detected");
			}

			if let Some(dependencies) = inwards_edges.get(&node_id) {
				temp_marks.insert(node_id);
				for &dependant in dependencies {
					visit(dependant, temp_marks, sorted, inwards_edges);
				}
				temp_marks.remove(&node_id);
			}
			sorted.push(node_id);
		}
		assert!(self.nodes.iter().any(|(id, _)| *id == self.output), "Output id {} does not exist", self.output);
		visit(self.output, &mut HashSet::new(), &mut sorted, &inwards_edges);

		sorted
	}

	/*// Based on https://en.wikipedia.org/wiki/Topological_sorting#Kahn's_algorithm
	pub fn topological_sort(&self) -> Vec<NodeId> {
		let mut sorted = Vec::new();
		let outwards_edges = self.collect_outwards_edges();
		let mut inwards_edges = self.collect_inwards_edges();
		let mut no_incoming_edges: Vec<_> = self.nodes.iter().map(|entry| entry.0).filter(|id| !inwards_edges.contains_key(id)).collect();

		assert_ne!(no_incoming_edges.len(), 0, "Acyclic graphs must have at least one node with no incoming edge");

		while let Some(node_id) = no_incoming_edges.pop() {
			sorted.push(node_id);

			if let Some(outwards_edges) = outwards_edges.get(&node_id) {
				for &ref_id in outwards_edges {
					let dependencies = inwards_edges.get_mut(&ref_id).unwrap();
					dependencies.retain(|&id| id != node_id);
					if dependencies.is_empty() {
						no_incoming_edges.push(ref_id)
					}
				}
			}
		}
		debug!("Sorted order {sorted:?}");
		sorted
	}*/

	pub fn reorder_ids(&mut self) {
		let order = self.topological_sort();
		// Map of node ids to indexes (which become the node ids as they are inserted into the borrow stack)
		let lookup: HashMap<_, _> = order.iter().enumerate().map(|(pos, id)| (*id, pos as NodeId)).collect();
		self.nodes = order
			.iter()
			.enumerate()
			.map(|(pos, id)| {
				let node = self.nodes.swap_remove(self.nodes.iter().position(|(test_id, _)| test_id == id).unwrap()).1;
				(pos as NodeId, node)
			})
			.collect();
		self.replace_node_references(&lookup, false);
		assert_eq!(order.len(), self.nodes.len());
	}

	fn replace_node_references(&mut self, lookup: &HashMap<u64, u64>, skip_lambdas: bool) {
		self.nodes.iter_mut().for_each(|(_, node)| {
			node.map_ids(|id| *lookup.get(&id).expect("node not found in lookup table"), skip_lambdas);
		});
		self.inputs = self.inputs.iter().filter_map(|id| lookup.get(id).copied()).collect();
		self.output = *lookup.get(&self.output).unwrap();
	}
}

/// The `TypingContext` is used to store the types of the nodes indexed by their stable node id.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TypingContext {
	lookup: Cow<'static, HashMap<NodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>>>,
	inferred: HashMap<NodeId, NodeIOTypes>,
	constructor: HashMap<NodeId, NodeConstructor>,
}

impl TypingContext {
	/// Creates a new `TypingContext` with the given lookup table.
	pub fn new(lookup: &'static HashMap<NodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>>) -> Self {
		Self {
			lookup: Cow::Borrowed(lookup),
			..Default::default()
		}
	}

	/// Updates the `TypingContext` wtih a given proto network. This will infer the types of the nodes
	/// and store them in the `inferred` field. The proto network has to be topologically sorted
	/// and contain fully resolved stable node ids.
	pub fn update(&mut self, network: &ProtoNetwork) -> Result<(), String> {
		for (id, node) in network.nodes.iter() {
			self.infer(*id, node)?;
		}
		Ok(())
	}

	/// Returns the node constructor for a given node id.
	pub fn constructor(&self, node_id: NodeId) -> Option<NodeConstructor> {
		self.constructor.get(&node_id).copied()
	}

	/// Returns the inferred types for a given node id.
	pub fn infer(&mut self, node_id: NodeId, node: &ProtoNode) -> Result<NodeIOTypes, String> {
		let identifier = node.identifier.name.clone();

		// Return the inferred type if it is already known
		if let Some(infered) = self.inferred.get(&node_id) {
			return Ok(infered.clone());
		}

		let parameters = match node.construction_args {
			// If the node has a value parameter we can infer the return type from it
			ConstructionArgs::Value(ref v) => {
				assert!(matches!(node.input, ProtoNodeInput::None));
                // TODO: This should return a reference to the value
				let types = NodeIOTypes::new(concrete!(()), v.ty(), vec![v.ty()]);
				self.inferred.insert(node_id, types.clone());
				return Ok(types);
			}
			// If the node has nodes as parameters we can infer the types from the node outputs
			ConstructionArgs::Nodes(ref nodes) => nodes
				.iter()
				.map(|(id, _)| {
					self.inferred
						.get(id)
						.ok_or(format!("Inferring type of {node_id} depends on {id} which is not present in the typing context"))
						.map(|node| node.ty())
				})
				.collect::<Result<Vec<Type>, String>>()?,
		};

		// Get the node input type from the proto node declaration
		let input = match node.input {
			ProtoNodeInput::None => concrete!(()),
			ProtoNodeInput::ShortCircut(ref ty) => ty.clone(),
			ProtoNodeInput::Network(ref ty) => ty.clone(),
			ProtoNodeInput::Node(id, _) => {
				let input = self
					.inferred
					.get(&id)
					.ok_or(format!("Inferring type of {node_id} depends on {id} which is not present in the typing context"))?;
				input.output.clone()
			}
		};
		let impls = self.lookup.get(&node.identifier).ok_or(format!("No implementations found for {:?}", node.identifier))?;

		if matches!(input, Type::Generic(_)) {
			return Err(format!("Generic types are not supported as inputs yet {:?} occured in {:?}", &input, node.identifier));
		}
		if parameters.iter().any(|p| match p {
			Type::Fn(_, b) if matches!(b.as_ref(), Type::Generic(_)) => true,
			_ => false,
		}) {
			return Err(format!("Generic types are not supported in parameters: {:?} occured in {:?}", parameters, node.identifier));
		}
		fn covariant(from: &Type, to: &Type) -> bool {
			match (from, to) {
				(Type::Concrete(t1), Type::Concrete(t2)) => t1 == t2,
				(Type::Fn(a1, b1), Type::Fn(a2, b2)) => covariant(a1, a2) && covariant(b1, b2),
				// TODO: relax this requirement when allowing generic types as inputs
				(Type::Generic(_), _) => false,
				(_, Type::Generic(_)) => true,
                _ => false,
			}
		}

		// List of all implementations that match the input and parameter types
		let valid_output_types = impls
			.keys()
			.filter(|node_io| covariant(&input, &node_io.input) && parameters.iter().zip(node_io.parameters.iter()).all(|(p1, p2)| covariant(p1, p2) && covariant(p1, p2)))
			.collect::<Vec<_>>();

		// Attempt to substitute generic types with concrete types and save the list of results
		let substitution_results = valid_output_types
			.iter()
			.map(|node_io| {
				collect_generics(node_io)
					.iter()
					.try_for_each(|generic| check_generic(node_io, &input, &parameters, generic).map(|_| ()))
					.map(|_| {
						if let Type::Generic(out) = &node_io.output {
							((*node_io).clone(), check_generic(node_io, &input, &parameters, out).unwrap())
						} else {
							((*node_io).clone(), node_io.output.clone())
						}
					})
			})
			.collect::<Vec<_>>();

		// Collect all substitutions that are valid
		let valid_impls = substitution_results.iter().filter_map(|result| result.as_ref().ok()).collect::<Vec<_>>();

		match valid_impls.as_slice() {
			[] => {
				dbg!(&self.inferred);
				Err(format!(
					"No implementations found for {identifier} with \ninput: {input:?} and \nparameters: {parameters:?}.\nOther Implementations found: {:?}",
					impls,
				))
			}
			[(org_nio, output)] => {
				let node_io = NodeIOTypes::new(input, (*output).clone(), parameters);

				// Save the inferred type
				self.inferred.insert(node_id, node_io.clone());
				self.constructor.insert(node_id, impls[org_nio]);
				Ok(node_io)
			}
			_ => Err(format!(
				"Multiple implementations found for {identifier} with input {input:?} and parameters {parameters:?} (valid types: {valid_output_types:?}"
			)),
		}
	}
}

/// Returns a list of all generic types used in the node
fn collect_generics(types: &NodeIOTypes) -> Vec<Cow<'static, str>> {
	let inputs = [&types.input].into_iter().chain(types.parameters.iter().flat_map(|x| x.second()));
	let mut generics = inputs
		.filter_map(|t| match t {
			Type::Generic(out) => Some(out.clone()),
			_ => None,
		})
		.collect::<Vec<_>>();
	if let Type::Generic(out) = &types.output {
		generics.push(out.clone());
	}
	generics.dedup();
	generics
}

/// Checks if a generic type can be substituted with a concrete type and returns the concrete type
fn check_generic(types: &NodeIOTypes, input: &Type, parameters: &[Type], generic: &str) -> Result<Type, String> {
	let inputs = [(Some(&types.input), Some(input))]
		.into_iter()
		.chain(types.parameters.iter().map(|x| x.second()).zip(parameters.iter().map(|x| x.second())));
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

#[cfg(test)]
mod test {
	use super::*;
	use crate::proto::{ConstructionArgs, ProtoNetwork, ProtoNode, ProtoNodeInput};

	#[test]
	fn topological_sort() {
		let construction_network = test_network();
		let sorted = construction_network.topological_sort();

		println!("{:#?}", sorted);
		assert_eq!(sorted, vec![14, 10, 11, 1]);
	}

	#[test]
	fn id_reordering() {
		let mut construction_network = test_network();
		construction_network.reorder_ids();
		let sorted = construction_network.topological_sort();
		println!("nodes: {:#?}", construction_network.nodes);
		assert_eq!(sorted, vec![0, 1, 2, 3]);
		let ids: Vec<_> = construction_network.nodes.iter().map(|(id, _)| *id).collect();
		println!("{:#?}", ids);
		println!("nodes: {:#?}", construction_network.nodes);
		assert_eq!(construction_network.nodes[0].1.identifier.name.as_ref(), "value");
		assert_eq!(ids, vec![0, 1, 2, 3]);
	}

	#[test]
	fn id_reordering_idempotent() {
		let mut construction_network = test_network();
		construction_network.reorder_ids();
		construction_network.reorder_ids();
		let sorted = construction_network.topological_sort();
		assert_eq!(sorted, vec![0, 1, 2, 3]);
		let ids: Vec<_> = construction_network.nodes.iter().map(|(id, _)| *id).collect();
		println!("{:#?}", ids);
		assert_eq!(construction_network.nodes[0].1.identifier.name.as_ref(), "value");
		assert_eq!(ids, vec![0, 1, 2, 3]);
	}

	#[test]
	fn input_resolution() {
		let mut construction_network = test_network();
		construction_network.resolve_inputs();
		println!("{:#?}", construction_network);
		assert_eq!(construction_network.nodes[0].1.identifier.name.as_ref(), "value");
		assert_eq!(construction_network.nodes.len(), 6);
		assert_eq!(construction_network.nodes[5].1.construction_args, ConstructionArgs::Nodes(vec![(3, false), (4, true)]));
	}

	#[test]
	fn stable_node_id_generation() {
		let mut construction_network = test_network();
		construction_network.reorder_ids();
		construction_network.generate_stable_node_ids();
		construction_network.resolve_inputs();
		construction_network.generate_stable_node_ids();
		assert_eq!(construction_network.nodes[0].1.identifier.name.as_ref(), "value");
		let ids: Vec<_> = construction_network.nodes.iter().map(|(id, _)| *id).collect();
		assert_eq!(
			ids,
			vec![
				10739226043134366700,
				17332796976541881019,
				7897288931440576543,
				7388412494950743023,
				359700384277940942,
				12822947441562012352
			]
		);
	}

	fn test_network() -> ProtoNetwork {
		ProtoNetwork {
			inputs: vec![10],
			output: 1,
			nodes: [
				(
					7,
					ProtoNode {
						identifier: "id".into(),
						input: ProtoNodeInput::Node(11, false),
						construction_args: ConstructionArgs::Nodes(vec![]),
					},
				),
				(
					1,
					ProtoNode {
						identifier: "id".into(),
						input: ProtoNodeInput::Node(11, false),
						construction_args: ConstructionArgs::Nodes(vec![]),
					},
				),
				(
					10,
					ProtoNode {
						identifier: "cons".into(),
						input: ProtoNodeInput::Network(concrete!(u32)),
						construction_args: ConstructionArgs::Nodes(vec![(14, false)]),
					},
				),
				(
					11,
					ProtoNode {
						identifier: "add".into(),
						input: ProtoNodeInput::Node(10, false),
						construction_args: ConstructionArgs::Nodes(vec![]),
					},
				),
				(
					14,
					ProtoNode {
						identifier: "value".into(),
						input: ProtoNodeInput::None,
						construction_args: ConstructionArgs::Value(value::TaggedValue::U32(2)),
					},
				),
			]
			.into_iter()
			.collect(),
		}
	}
}
