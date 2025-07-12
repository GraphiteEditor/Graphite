use crate::document::value::TaggedValue;
use crate::document::{AbsoluteInputConnector, InlineRust, ProtonodeEntry};
pub use graphene_core::registry::*;
use graphene_core::uuid::{NodeId, ProtonodePath, SNI};
use graphene_core::*;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;

#[derive(Debug, Default, Clone)]
/// A list of [`ProtoNode`]s, which is an intermediate step between the [`crate::document::NodeNetwork`] and the `BorrowTree` containing a single flattened network.
pub struct ProtoNetwork {
	/// A list of nodes stored in a Vec to allow for sorting.
	nodes: Vec<ProtonodeEntry>,
	/// The most downstream node in the protonetwork
	pub output: NodeId,
}

impl ProtoNetwork {
	pub fn from_vec(nodes: Vec<ProtonodeEntry>) -> Self {
		let last_entry = nodes.last().expect("Cannot compile empty protonetwork");
		let output = match last_entry {
			ProtonodeEntry::Protonode(proto_node) => proto_node.stable_node_id,
			ProtonodeEntry::Deduplicated => {
				panic!("Not possible for the output protonode to be deduplicated");
			}
		};
		ProtoNetwork { nodes, output }
	}

	pub fn nodes(&self) -> impl Iterator<Item = &ProtoNode> {
		self.nodes
			.iter()
			.filter_map(|entry| if let ProtonodeEntry::Protonode(protonode) = entry { Some(protonode) } else { None })
	}
	pub fn into_nodes(self) -> impl Iterator<Item = ProtoNode> {
		self.nodes
			.into_iter()
			.filter_map(|entry| if let ProtonodeEntry::Protonode(protonode) = entry { Some(protonode) } else { None })
	}
}

// impl core::fmt::Display for ProtoNetwork {
// 	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
// 		f.write_str("Proto Network with nodes: ")?;
// 		fn write_node(f: &mut core::fmt::Formatter<'_>, network: &ProtoNetwork, id: NodeId, indent: usize) -> core::fmt::Result {
// 			f.write_str(&"\t".repeat(indent))?;
// 			let Some((_, node)) = network.nodes.iter().find(|(node_id, _)| *node_id == id) else {
// 				return f.write_str("{{Unknown Node}}");
// 			};
// 			f.write_str("Node: ")?;
// 			f.write_str(&node.identifier.name)?;

// 			f.write_str("\n")?;
// 			f.write_str(&"\t".repeat(indent))?;
// 			f.write_str("{\n")?;

// 			f.write_str(&"\t".repeat(indent + 1))?;
// 			f.write_str("Input: ")?;
// 			match &node.input {
// 				ProtoNodeInput::None => f.write_str("None")?,
// 				ProtoNodeInput::ManualComposition(ty) => f.write_fmt(format_args!("Manual Composition (type = {ty:?})"))?,
// 				ProtoNodeInput::Node(_) => f.write_str("Node")?,
// 				ProtoNodeInput::NodeLambda(_) => f.write_str("Lambda Node")?,
// 			}
// 			f.write_str("\n")?;

// 			match &node.construction_args {
// 				ConstructionArgs::Value(value) => {
// 					f.write_str(&"\t".repeat(indent + 1))?;
// 					f.write_fmt(format_args!("Value construction argument: {value:?}"))?
// 				}
// 				ConstructionArgs::Nodes(nodes) => {
// 					for id in nodes {
// 						write_node(f, network, id.0, indent + 1)?;
// 					}
// 				}
// 				ConstructionArgs::Inline(inline) => {
// 					f.write_str(&"\t".repeat(indent + 1))?;
// 					f.write_fmt(format_args!("Inline construction argument: {inline:?}"))?
// 				}
// 			}
// 			f.write_str(&"\t".repeat(indent))?;
// 			f.write_str("}\n")?;
// 			Ok(())
// 		}

// 		let id = self.output;
// 		write_node(f, self, id, 0)
// 	}
// }

#[derive(Clone, Debug)]
pub struct UpstreamInputMetadata {
	pub input_sni: SNI,
	// Context dependencies are accumulated during compilation, then replaced with the difference between the node's dependencies and the inputs dependencies
	pub context_dependencies: ContextDependencies,
	// If the upstream node is a value node, then do not nullify since the value nodes do not have a cache inserted after them
	pub is_value: bool,
}

#[derive(Debug, Clone)]
pub struct NodeConstructionArgs {
	// Used to get the constructor from the function in `node_registry.rs`.
	pub identifier: ProtoNodeIdentifier,
	/// A list of stable node ids used as inputs to the constructor
	// A node is dependent on whatever is marked in its implementation, as well as all inputs
	// If a node is dependent on more than its input, then a context nullification node is placed on the input
	// Starts as None, and is populated during stable node id generation
	pub inputs: Vec<Option<UpstreamInputMetadata>>,
	// The union of all input context dependencies and the nodes context dependency. Used to generate the context nullification for the editor entry point
	pub context_dependencies: ContextDependencies,
}

#[derive(Debug, Clone)]
/// Defines the arguments used to construct the boxed node struct. This is used to call the constructor function in the `node_registry.rs` file - which is hidden behind a wall of macros.
pub enum ConstructionArgs {
	/// A value of a type that is known, allowing serialization (serde::Deserialize is not object safe)
	Value(MemoHash<TaggedValue>),
	Nodes(NodeConstructionArgs),
	/// Used for GPU computation to work around the limitations of rust-gpu.
	Inline(InlineRust),
}

// impl ConstructionArgs {
// 	// TODO: what? Used in the gpu_compiler crate for something.
// 	pub fn new_function_args(&self) -> Vec<String> {
// 		match self {
// 			ConstructionArgs::Nodes(nodes) => nodes.inputs.iter().map(|n| format!("n{:0x}", n.0)).collect(),
// 			ConstructionArgs::Value(value) => vec![value.to_primitive_string()],
// 			ConstructionArgs::Inline(inline) => vec![inline.expr.clone()],
// 		}
// 	}
// }

#[derive(Debug, Clone)]
/// A proto node is an intermediate step between the `DocumentNode` and the boxed struct that actually runs the node (found in the [`BorrowTree`]).
/// At different stages in the compilation process, this struct will be transformed into a reduced (more restricted) form acting as a subset of its original form, but that restricted form is still valid in the earlier stage in the compilation process before it was transformed.
// If the the protonode has ConstructionArgs::Value, then its identifier is not used, and is replaced with an UpcastNode with a value of the tagged value
pub struct ProtoNode {
	pub construction_args: ConstructionArgs,
	pub original_location: OriginalLocation,
	pub input: Type,
	pub stable_node_id: SNI,
	// Each protonode stores the input of the protonode which called it in order to map input SNI
	pub callers: Vec<(ProtonodePath, usize)>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
/// Stores the origin of the protonode in the document network``, which is either an inserted value protonode SNI for an input connector, or a protonode SNI for a protonode
pub enum OriginalLocation {
	Value(AbsoluteInputConnector),
	Node(ProtonodePath),
}

impl Default for ProtoNode {
	fn default() -> Self {
		Self {
			construction_args: ConstructionArgs::Value(TaggedValue::U32(0).into()),
			input: concrete!(Context),
			stable_node_id: NodeId(0),
			callers: Vec::new(),
			original_location: OriginalLocation::Node(Vec::new()),
		}
	}
}

impl ProtoNode {
	/// Construct a new [`ProtoNode`] with the specified construction args and a `ClonedNode` implementation.
	pub fn value(value: ConstructionArgs, stable_node_id: SNI) -> Self {
		Self {
			construction_args: value,
			input: concrete!(Context),
			stable_node_id,
			callers: Vec::new(),
			original_location: OriginalLocation::Value(AbsoluteInputConnector {
				network_path: Vec::new(),
				connector: crate::document::InputConnector::Export(0),
			}),
		}
	}

	// Hashes the inputs and implementation of non value nodes, and the value for value nodes
	pub fn generate_stable_node_id(&mut self) {
		use std::hash::Hasher;
		let mut hasher = rustc_hash::FxHasher::default();
		match &self.construction_args {
			ConstructionArgs::Nodes(nodes) => {
				for upstream_input in &nodes.inputs {
					upstream_input.as_ref().unwrap().input_sni.hash(&mut hasher);
				}
				nodes.identifier.hash(&mut hasher);
			}
			ConstructionArgs::Value(value) => value.hash(&mut hasher),
			ConstructionArgs::Inline(_) => todo!(),
		}

		self.stable_node_id = NodeId(hasher.finish());
	}
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GraphErrorType {
	InputNodeNotFound(NodeId),
	UnexpectedGenerics { index: usize, inputs: Vec<Type> },
	NoImplementations,
	NoConstructor,
	InvalidImplementations { inputs: String, error_inputs: Vec<Vec<(usize, (Type, Type))>> },
	MultipleImplementations { inputs: String, valid: Vec<NodeIOTypes> },
	UnresolvedType,
}
impl Debug for GraphErrorType {
	// TODO: format with the document graph context so the input index is the same as in the graph UI.
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			GraphErrorType::InputNodeNotFound(id) => write!(f, "Input node {id} is not present in the typing context"),
			GraphErrorType::UnexpectedGenerics { index, inputs } => write!(f, "Generic inputs should not exist but found at {index}: {inputs:?}"),
			GraphErrorType::NoImplementations => write!(f, "No implementations found"),
			GraphErrorType::NoConstructor => write!(f, "No construct found for node"),
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
			GraphErrorType::UnresolvedType => write!(f, "Could not determine type of node"),
		}
	}
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GraphError {
	pub original_location: OriginalLocation,
	pub identifier: Cow<'static, str>,
	pub error: GraphErrorType,
}
impl GraphError {
	pub fn new(construction_args: &ConstructionArgs, original_location: OriginalLocation, text: impl Into<GraphErrorType>) -> Self {
		let identifier = match &construction_args {
			ConstructionArgs::Nodes(node_construction_args) => node_construction_args.identifier.name.clone(),
			// Values are inserted into upcast nodes
			ConstructionArgs::Value(value) => format!("{:?} Value Node", value.deref().ty()).into(),
			ConstructionArgs::Inline(_) => "Inline".into(),
		};
		Self {
			original_location,
			identifier,
			error: text.into(),
		}
	}
}
impl Debug for GraphError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("NodeGraphError").field("identifier", &self.identifier.to_string()).field("error", &self.error).finish()
	}
}
pub type GraphErrors = Vec<GraphError>;

/// The `TypingContext` is used to store the types of the nodes indexed by their stable node id.
#[derive(Default, Clone, dyn_any::DynAny)]
pub struct TypingContext {
	lookup: Cow<'static, HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>>>,
	cache_lookup: Cow<'static, HashMap<Type, CacheConstructor>>,
	inferred: HashMap<NodeId, NodeIOTypes>,
	constructor: HashMap<NodeId, NodeConstructor>,
}

impl TypingContext {
	/// Creates a new `TypingContext` with the given lookup table.
	pub fn new(lookup: &'static HashMap<ProtoNodeIdentifier, HashMap<NodeIOTypes, NodeConstructor>>, cache_lookup: &'static HashMap<Type, CacheConstructor>) -> Self {
		Self {
			lookup: Cow::Borrowed(lookup),
			cache_lookup: Cow::Borrowed(cache_lookup),
			..Default::default()
		}
	}

	/// Updates the `TypingContext` with a given proto network. This will infer the types of the nodes
	/// and store them in the `inferred` field. The proto network has to be topologically sorted
	/// and contain fully resolved stable node ids.
	pub fn update(&mut self, network: &ProtoNetwork) -> Result<(), GraphErrors> {
		// Update types from the most upstream nodes first
		for node in network.nodes() {
			self.infer(node.stable_node_id, node)?;
		}
		Ok(())
	}

	pub fn remove_inference(&mut self, node_id: &NodeId) -> Option<NodeIOTypes> {
		self.constructor.remove(node_id);
		self.inferred.remove(node_id)
	}

	/// Returns the node constructor for a given node id.
	pub fn constructor(&self, node_id: NodeId) -> Option<NodeConstructor> {
		self.constructor.get(&node_id).copied()
	}

	// Returns the cache node constructor for a given type {
	pub fn cache_constructor(&self, cache_type: &Type) -> Option<CacheConstructor> {
		self.cache_lookup.get(cache_type).copied()
	}

	/// Returns the type of a given node id if it exists
	pub fn type_of(&self, node_id: NodeId) -> Option<&NodeIOTypes> {
		self.inferred.get(&node_id)
	}

	/// Returns the inferred types for a given node id.
	pub fn infer(&mut self, node_id: NodeId, node: &ProtoNode) -> Result<(), GraphErrors> {
		// Return the inferred type if it is already known
		if self.inferred.contains_key(&node_id) {
			return Ok(());
		}

		let (inputs, id) = match node.construction_args {
			// If the node has a value input we can infer the return type from it
			ConstructionArgs::Value(ref v) => {
				// assert!(matches!(node.input, ProtoNodeInput::None) || matches!(node.input, ProtoNodeInput::ManualComposition(ref x) if x == &concrete!(Context)));
				// TODO: This should return a reference to the value
				let types = NodeIOTypes::new(concrete!(Context), Type::Future(Box::new(v.ty())), vec![]);
				self.inferred.insert(node_id, types);
				return Ok(());
			}
			// If the node has nodes as inputs we can infer the types from the node outputs
			ConstructionArgs::Nodes(ref construction_args) => {
				let inputs = construction_args
					.inputs
					.iter()
					.map(|id| id.as_ref().unwrap().input_sni)
					.map(|id| {
						self.inferred
							.get(&id)
							.ok_or_else(|| vec![GraphError::new(&node.construction_args, node.original_location.clone(), GraphErrorType::InputNodeNotFound(id))])
							.map(|node| node.ty())
					})
					.collect::<Result<Vec<Type>, GraphErrors>>()?;
				(inputs, &construction_args.identifier)
			}
			ConstructionArgs::Inline(ref inline) => (vec![inline.ty.clone()], &*Box::new(ProtoNodeIdentifier::new("Extract"))),
		};

		let Some(impls) = self.lookup.get(id) else {
			return Err(vec![GraphError::new(&node.construction_args, node.original_location.clone(), GraphErrorType::NoImplementations)]);
		};

		if let Some(index) = inputs.iter().position(|p| {
			matches!(p,
			Type::Fn(_, b) if matches!(b.as_ref(), Type::Generic(_)))
		}) {
			return Err(vec![GraphError::new(
				&node.construction_args,
				node.original_location.clone(),
				GraphErrorType::UnexpectedGenerics { index, inputs },
			)]);
		}

		/// Checks if a proposed input to a particular (primary or secondary) input connector is valid for its type signature.
		/// `from` indicates the value given to a input, `to` indicates the input's allowed type as specified by its type signature.
		pub fn valid_type(from: &Type, to: &Type) -> bool {
			match (from, to) {
				// Direct comparison of two concrete types.
				(Type::Concrete(type1), Type::Concrete(type2)) => type1 == type2,
				// Check inner type for futures
				(Type::Future(type1), Type::Future(type2)) => valid_type(type1, type2),
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
				// If either the proposed input or the allowed input are generic, we allow the substitution (meaning this is a valid subtype).
				// TODO: Add proper generic counting which is not based on the name
				(Type::Generic(_), _) | (_, Type::Generic(_)) => true,
				// Reject unknown type relationships.
				_ => false,
			}
		}

		// List of all implementations that match the call argument type
		let valid_output_types = impls
			.keys()
			.filter(|node_io| valid_type(&node_io.call_argument, &node.input) && inputs.iter().zip(node_io.inputs.iter()).all(|(p1, p2)| valid_type(p1, p2)))
			.collect::<Vec<_>>();

		// Attempt to substitute generic types with concrete types and save the list of results
		let substitution_results = valid_output_types
			.iter()
			.map(|node_io| {
				let generics_lookup: Result<HashMap<_, _>, _> = collect_generics(node_io)
					.iter()
					.map(|generic| check_generic(node_io, &node.input, &inputs, generic).map(|x| (generic.to_string(), x)))
					.collect();

				generics_lookup.map(|generics_lookup| {
					let orig_node_io = (*node_io).clone();
					let mut new_node_io = orig_node_io.clone();
					replace_generics(&mut new_node_io, &generics_lookup);
					(new_node_io, orig_node_io)
				})
			})
			.collect::<Vec<_>>();

		// Collect all substitutions that are valid
		let valid_impls = substitution_results.iter().filter_map(|result| result.as_ref().ok()).collect::<Vec<_>>();

		match valid_impls.as_slice() {
			[] => {
				let mut best_errors = usize::MAX;
				let mut error_inputs = Vec::new();
				for node_io in impls.keys() {
					let current_errors = [&node.input]
						.into_iter()
						.chain(&inputs)
						.cloned()
						.zip([&node_io.call_argument].into_iter().chain(&node_io.inputs).cloned())
						.enumerate()
						.filter(|(_, (p1, p2))| !valid_type(p1, p2))
						.collect::<Vec<_>>();
					if current_errors.len() < best_errors {
						best_errors = current_errors.len();
						error_inputs.clear();
					}
					if current_errors.len() <= best_errors {
						error_inputs.push(current_errors);
					}
				}
				let inputs = inputs.iter()
					.enumerate()
					// TODO: Make the following line's if statement conditional on being a call argument or primary input
					.map(|(i, t)| {let input_number = i + 1; format!("• Input {input_number}: {t}")})
					.collect::<Vec<_>>()
					.join("\n");
				Err(vec![GraphError::new(
					&node.construction_args,
					node.original_location.clone(),
					GraphErrorType::InvalidImplementations { inputs, error_inputs },
				)])
			}
			[(node_io, org_nio)] => {
				// Save the inferred type
				self.inferred.insert(node_id, node_io.clone());
				self.constructor.insert(node_id, impls[org_nio]);
				Ok(())
			}
			// If two types are available and one of them accepts () an input, always choose that one
			[first, second] => {
				if first.0.call_argument != second.0.call_argument {
					for (node_io, orig_nio) in [first, second] {
						if node_io.call_argument != concrete!(()) {
							continue;
						}

						// Save the inferred type
						self.inferred.insert(node_id, node_io.clone());
						self.constructor.insert(node_id, impls[orig_nio]);
						return Ok(());
					}
				}
				let inputs = [&node.input].into_iter().chain(&inputs).map(|t| t.to_string()).collect::<Vec<_>>().join(", ");
				let valid = valid_output_types.into_iter().cloned().collect();
				Err(vec![GraphError::new(
					&node.construction_args,
					node.original_location.clone(),
					GraphErrorType::MultipleImplementations { inputs, valid },
				)])
			}
			_ => {
				let inputs = [&node.input].into_iter().chain(&inputs).map(|t| t.to_string()).collect::<Vec<_>>().join(", ");
				let valid = valid_output_types.into_iter().cloned().collect();
				Err(vec![GraphError::new(
					&node.construction_args,
					node.original_location.clone(),
					GraphErrorType::MultipleImplementations { inputs, valid },
				)])
			}
		}
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
	let inputs = [(Some(&types.call_argument), Some(input))]
		.into_iter()
		.chain(types.inputs.iter().map(|x| x.fn_input()).zip(parameters.iter().map(|x| x.fn_input())))
		.chain(types.inputs.iter().map(|x| x.fn_output()).zip(parameters.iter().map(|x| x.fn_output())));
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
		let Type::Generic(ident) = ty else {
			return None;
		};
		lookup.get(ident.as_ref()).cloned()
	};
	types.call_argument.replace_nested(replace);
	types.return_value.replace_nested(replace);
	for input in &mut types.inputs {
		input.replace_nested(replace);
	}
}

// #[cfg(test)]
// mod test {
// 	use super::*;
// 	use crate::proto::{ConstructionArgs, ProtoNetwork, ProtoNode, ProtoNodeInput};

// 	#[test]
// 	fn topological_sort() {
// 		let construction_network = test_network();
// 		let (sorted, _) = construction_network.topological_sort().expect("Error when calling 'topological_sort' on 'construction_network.");
// 		let sorted: Vec<_> = sorted.iter().map(|x| construction_network.nodes[x.0 as usize].0).collect();
// 		println!("{sorted:#?}");
// 		assert_eq!(sorted, vec![NodeId(14), NodeId(10), NodeId(11), NodeId(1)]);
// 	}

// 	#[test]
// 	fn topological_sort_with_cycles() {
// 		let construction_network = test_network_with_cycles();
// 		let sorted = construction_network.topological_sort();

// 		assert!(sorted.is_err())
// 	}

// 	#[test]
// 	fn id_reordering() {
// 		let mut construction_network = test_network();
// 		construction_network.reorder_ids().expect("Error when calling 'reorder_ids' on 'construction_network.");
// 		let (sorted, _) = construction_network.topological_sort().expect("Error when calling 'topological_sort' on 'construction_network.");
// 		let sorted: Vec<_> = sorted.iter().map(|x| construction_network.nodes[x.0 as usize].0).collect();
// 		println!("nodes: {:#?}", construction_network.nodes);
// 		assert_eq!(sorted, vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)]);
// 		let ids: Vec<_> = construction_network.nodes.iter().map(|(id, _)| *id).collect();
// 		println!("{ids:#?}");
// 		println!("nodes: {:#?}", construction_network.nodes);
// 		assert_eq!(construction_network.nodes[0].1.identifier.name.as_ref(), "value");
// 		assert_eq!(ids, vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)]);
// 	}

// 	#[test]
// 	fn id_reordering_idempotent() {
// 		let mut construction_network = test_network();
// 		construction_network.reorder_ids().expect("Error when calling 'reorder_ids' on 'construction_network.");
// 		construction_network.reorder_ids().expect("Error when calling 'reorder_ids' on 'construction_network.");
// 		let (sorted, _) = construction_network.topological_sort().expect("Error when calling 'topological_sort' on 'construction_network.");
// 		assert_eq!(sorted, vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)]);
// 		let ids: Vec<_> = construction_network.nodes.iter().map(|(id, _)| *id).collect();
// 		println!("{ids:#?}");
// 		assert_eq!(construction_network.nodes[0].1.identifier.name.as_ref(), "value");
// 		assert_eq!(ids, vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)]);
// 	}

// 	#[test]
// 	fn input_resolution() {
// 		let mut construction_network = test_network();
// 		construction_network.resolve_inputs().expect("Error when calling 'resolve_inputs' on 'construction_network.");
// 		println!("{construction_network:#?}");
// 		assert_eq!(construction_network.nodes[0].1.identifier.name.as_ref(), "value");
// 		assert_eq!(construction_network.nodes.len(), 6);
// 		assert_eq!(construction_network.nodes[5].1.construction_args, ConstructionArgs::Nodes(vec![(NodeId(3), false), (NodeId(4), true)]));
// 	}

// 	#[test]
// 	fn stable_node_id_generation() {
// 		let mut construction_network = test_network();
// 		construction_network.resolve_inputs().expect("Error when calling 'resolve_inputs' on 'construction_network.");
// 		construction_network.generate_stable_node_ids();
// 		assert_eq!(construction_network.nodes[0].1.identifier.name.as_ref(), "value");
// 		let ids: Vec<_> = construction_network.nodes.iter().map(|(id, _)| *id).collect();
// 		assert_eq!(
// 			ids,
// 			vec![
// 				NodeId(16997244687192517417),
// 				NodeId(12226224850522777131),
// 				NodeId(9162113827627229771),
// 				NodeId(12793582657066318419),
// 				NodeId(16945623684036608820),
// 				NodeId(2640415155091892458)
// 			]
// 		);
// 	}

// 	fn test_network() -> ProtoNetwork {
// 		ProtoNetwork {
// 			inputs: vec![NodeId(10)],
// 			output: NodeId(1),
// 			nodes: [
// 				(
// 					NodeId(7),
// 					ProtoNode {
// 						identifier: "id".into(),
// 						input: ProtoNodeInput::Node(NodeId(11)),
// 						construction_args: ConstructionArgs::Nodes(vec![]),
// 						..Default::default()
// 					},
// 				),
// 				(
// 					NodeId(1),
// 					ProtoNode {
// 						identifier: "id".into(),
// 						input: ProtoNodeInput::Node(NodeId(11)),
// 						construction_args: ConstructionArgs::Nodes(vec![]),
// 						..Default::default()
// 					},
// 				),
// 				(
// 					NodeId(10),
// 					ProtoNode {
// 						identifier: "cons".into(),
// 						input: ProtoNodeInput::ManualComposition(concrete!(u32)),
// 						construction_args: ConstructionArgs::Nodes(vec![(NodeId(14), false)]),
// 						..Default::default()
// 					},
// 				),
// 				(
// 					NodeId(11),
// 					ProtoNode {
// 						identifier: "add".into(),
// 						input: ProtoNodeInput::Node(NodeId(10)),
// 						construction_args: ConstructionArgs::Nodes(vec![]),
// 						..Default::default()
// 					},
// 				),
// 				(
// 					NodeId(14),
// 					ProtoNode {
// 						identifier: "value".into(),
// 						input: ProtoNodeInput::None,
// 						construction_args: ConstructionArgs::Value(value::TaggedValue::U32(2).into()),
// 						..Default::default()
// 					},
// 				),
// 			]
// 			.into_iter()
// 			.collect(),
// 		}
// 	}

// 	fn test_network_with_cycles() -> ProtoNetwork {
// 		ProtoNetwork {
// 			inputs: vec![NodeId(1)],
// 			output: NodeId(1),
// 			nodes: [
// 				(
// 					NodeId(1),
// 					ProtoNode {
// 						identifier: "id".into(),
// 						input: ProtoNodeInput::Node(NodeId(2)),
// 						construction_args: ConstructionArgs::Nodes(vec![]),
// 						..Default::default()
// 					},
// 				),
// 				(
// 					NodeId(2),
// 					ProtoNode {
// 						identifier: "id".into(),
// 						input: ProtoNodeInput::Node(NodeId(1)),
// 						construction_args: ConstructionArgs::Nodes(vec![]),
// 						..Default::default()
// 					},
// 				),
// 			]
// 			.into_iter()
// 			.collect(),
// 		}
// 	}
// }
