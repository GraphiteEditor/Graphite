use std::collections::{HashMap, HashSet};

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNodeImplementation, InlineRust, NodeInput};
use graph_craft::proto::{GraphErrorType, GraphErrors, Promotion};
use graph_craft::{ProtoNodeIdentifier, Type, concrete};
use graphene_std::NodeIOTypes;
use graphene_std::uuid::NodeId;
use interpreted_executor::dynamic_executor::{NodeTypes, ResolvedDocumentNodeTypesDelta};
use interpreted_executor::node_registry::NODE_REGISTRY;
use once_cell::sync::Lazy;

use crate::messages::portfolio::document::node_graph::utility_types::FrontendGraphDataType;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeNetworkInterface, OutputConnector};

/// The source types an `input_adapter` converts into each destination type, keyed by destination.
type AdapterConversions = HashMap<Type, HashSet<Type>>;

/// The conversions of every `input_adapter` in the registry.
static ADAPTER_CONVERSIONS: Lazy<HashMap<ProtoNodeIdentifier, AdapterConversions>> = Lazy::new(|| {
	let mut adapter_conversions: HashMap<ProtoNodeIdentifier, AdapterConversions> = HashMap::new();

	for (identifier, implementations) in NODE_REGISTRY.iter().filter(|(identifier, _)| identifier.as_str().starts_with("input_adapter<")) {
		let conversions = adapter_conversions.entry(identifier.clone()).or_default();
		for node_io in implementations.keys() {
			if let Some(source) = node_io.inputs.first() {
				conversions.entry(node_io.return_value.nested_type().clone()).or_default().insert(source.nested_type().clone());
			}
		}
	}

	adapter_conversions
});

/// For each input of each proto node, the conversions of the `input_adapter` the compiler places ahead of it, or `None` where wires pass through unconverted.
static INPUT_CONVERSIONS: Lazy<HashMap<ProtoNodeIdentifier, Vec<Option<&'static AdapterConversions>>>> = Lazy::new(|| {
	// Initialized before the registry locks below, since its own initialization takes them
	let adapter_conversions = &*ADAPTER_CONVERSIONS;
	let (Ok(node_registry), Ok(node_metadata)) = (graphene_std::registry::NODE_REGISTRY.lock(), graphene_std::registry::NODE_METADATA.lock()) else {
		log::error!("Could not lock the node registry to read the input adapter of each input");
		return HashMap::new();
	};

	// O(nodes * inputs * implementations), once
	node_metadata
		.keys()
		.filter_map(|identifier| {
			let implementations = node_registry.get(identifier)?;
			let number_of_inputs = implementations.iter().map(|(_, node_io)| node_io.inputs.len()).max().unwrap_or_default();

			let conversions = (0..number_of_inputs)
				.map(|index| {
					let input_types = implementations.iter().filter_map(|(_, node_io)| node_io.inputs.get(index).cloned()).collect();
					preprocessor::input_adapter_identifier(&input_types, &NODE_REGISTRY).and_then(|adapter| adapter_conversions.get(&adapter))
				})
				.collect();

			Some((identifier.clone(), conversions))
		})
		.collect()
});

/// Proto nodes whose every implementation returns its one input's type unchanged (like Monitor and Memoize), so a wire through one is constrained by what it feeds.
static TYPE_PRESERVING_NODES: Lazy<HashSet<ProtoNodeIdentifier>> = Lazy::new(|| {
	NODE_REGISTRY
		.iter()
		.filter(|(_, implementations)| {
			!implementations.is_empty()
				&& implementations
					.keys()
					.all(|node_io| matches!(node_io.inputs.as_slice(), [input] if input.nested_type() == node_io.return_value.nested_type()))
		})
		.map(|(identifier, _)| identifier.clone())
		// The compiler removes passthrough nodes, so they're absent from the registry, but their output carries their input's type too
		.chain(std::iter::once(graphene_std::ops::passthrough::IDENTIFIER))
		.collect()
});

/// The conversions ahead of each input of a proto node, indexed by input.
fn input_conversions(proto_node_identifier: &ProtoNodeIdentifier) -> &'static [Option<&'static AdapterConversions>] {
	INPUT_CONVERSIONS.get(proto_node_identifier).map(Vec::as_slice).unwrap_or_default()
}

/// Whether a `source` type can feed an input registered as `connector`, through the `conversions` the compiler places ahead of that input
/// and any rank promotion (such as raising an item onto a list input) that type resolution inserts after them.
fn type_feeds_input(source: &Type, connector: &Type, conversions: Option<&AdapterConversions>) -> bool {
	let (source, connector) = (source.nested_type(), connector.nested_type());

	// The types the wire can leave the input adapter as: unchanged, or converted into any destination accepting it
	let converted = conversions.into_iter().flatten().filter(|(_, sources)| sources.contains(source)).map(|(destination, _)| destination);

	std::iter::once(source)
		.chain(converted)
		.any(|adapted| adapted == connector || Promotion::exists_between(adapted, connector))
}

// This file contains utility methods for interfacing with the resolved types returned from the compiler
#[derive(Debug, Default)]
pub struct ResolvedDocumentNodeTypes {
	pub types: HashMap<Vec<NodeId>, NodeTypes>,
	pub node_graph_errors: GraphErrors,
	pub node_graph_warnings: GraphErrors,
}

impl ResolvedDocumentNodeTypes {
	pub fn update(&mut self, delta: ResolvedDocumentNodeTypesDelta, errors: GraphErrors, warnings: GraphErrors) {
		for (path, node_type) in delta.add {
			self.types.insert(path.to_vec(), node_type);
		}
		for path in delta.remove {
			self.types.remove(&path.to_vec());
		}
		self.node_graph_errors = errors;
		self.node_graph_warnings = warnings;
	}
}

/// Represents the result of a type query for an input or output connector.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeSource {
	/// A type that has been compiled based on all upstream types.
	Compiled(Type),
	/// The type of value inputs.
	TaggedValue(Type),
	/// When the input/output is not compiled. The Type is from the document node definition, or () if it doesn't exist.
	Unknown,
	/// When there is a node graph error for the inputs to a node. The Type is from the document node definition, or () if it doesn't exist.
	Invalid,
	/// When there is an error in the algorithm for determining the input/output type (indicates a bug in the editor).
	Error(&'static str),
}

impl TypeSource {
	/// The reduced set of frontend types for displaying color.
	pub fn displayed_type(&self) -> FrontendGraphDataType {
		if matches!(self, TypeSource::Invalid) {
			return FrontendGraphDataType::Invalid;
		};
		match self.compiled_nested_type() {
			Some(nested_type) => FrontendGraphDataType::from_type(nested_type),
			None => FrontendGraphDataType::General,
		}
	}

	/// Whether the compiled type is a rank-1 `List<T>`, as opposed to a rank-0 `Item<T>` or a bare value.
	/// A bundled cell carrying a whole list displays as the list it carries.
	pub fn is_list(&self) -> bool {
		self.compiled_nested_type().is_some_and(|ty| matches!(ty, Type::List(_)) || ty.bundle_element_name().is_some())
	}

	/// The element type with any rank-0 `Item` or rank-1 `List` wrapper peeled, so semantic type checks can be rank-agnostic.
	pub fn compiled_element_type(&self) -> Option<&Type> {
		Some(element_of(self.compiled_nested_type()?))
	}

	/// The identifier name of [`Self::compiled_element_type`].
	pub fn compiled_element_name(&self) -> Option<String> {
		Some(self.compiled_element_type()?.identifier_name())
	}

	pub fn compiled_nested_type(&self) -> Option<&Type> {
		match self {
			TypeSource::Compiled(compiled_type) => Some(compiled_type.nested_type()),
			TypeSource::TaggedValue(value_type) => Some(value_type.nested_type()),
			_ => None,
		}
	}

	/// Used when searching for nodes in the add node menu.
	pub fn add_node_string(self) -> Option<String> {
		self.compiled_nested_type().map(|ty| format!("type:{ty}"))
	}

	/// The type to display in the tooltip label.
	pub fn resolved_type_tooltip_string(&self) -> String {
		match self {
			TypeSource::Compiled(compiled_type) => compiled_type.nested_type().to_string(),
			TypeSource::TaggedValue(value_type) => value_type.nested_type().to_string(),
			TypeSource::Unknown => "Unknown Data Type".to_string(),
			TypeSource::Invalid => "Invalid Type Combination".to_string(),
			TypeSource::Error(_) => "Error Getting Data Type".to_string(),
		}
	}

	/// The type to display in the node row.
	pub fn resolved_type_node_string(&self) -> String {
		match self {
			TypeSource::Compiled(compiled_type) => compiled_type.nested_type().to_string(),
			TypeSource::TaggedValue(value_type) => value_type.nested_type().to_string(),
			TypeSource::Unknown => "Unknown".to_string(),
			TypeSource::Invalid => "Invalid".to_string(),
			TypeSource::Error(_) => "Error".to_string(),
		}
	}
}

/// Peels any `Fn`/`Future` wrapper, then any rank-0 `Item` or rank-1 `List` wrapper, down to the element type.
fn element_of(ty: &Type) -> &Type {
	match ty.nested_type() {
		Type::Item(element) | Type::List(element) => element.as_ref(),
		other => other,
	}
}

impl NodeNetworkInterface {
	fn input_has_error(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> bool {
		match input_connector {
			InputConnector::Node { node_id, input_index } => {
				let Some(implementation) = self.implementation(node_id, network_path) else {
					log::error!("Could not get implementation in input_has_error");
					return false;
				};
				let node_path = [network_path, &[*node_id]].concat();
				match implementation {
					DocumentNodeImplementation::Network(_) => {
						let outward_wires = self.with_outward_wires(&node_path, |map| map.get(&OutputConnector::Import(*input_index)).cloned()).flatten();
						let Some(outward_wires) = outward_wires else { return false };
						outward_wires.iter().any(|connector| match connector {
							InputConnector::Node { node_id, input_index } => self.input_has_error(&InputConnector::node_at_index(*node_id, *input_index), &node_path),
							InputConnector::Export(_) => false,
						})
					}
					DocumentNodeImplementation::ProtoNode(_) => self.resolved_types.node_graph_errors.iter().any(|error| {
						error.node_path == node_path
							&& match &error.error {
								GraphErrorType::InvalidImplementations { error_inputs, .. } => error_inputs.iter().any(|solution| solution.iter().any(|(index, _)| index == input_index)),
								_ => true,
							}
					}),

					DocumentNodeImplementation::Extract => false,
				}
			}
			InputConnector::Export(_) => false,
		}
	}

	pub fn input_type_not_invalid(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> TypeSource {
		let Some(input) = self.input_from_connector(input_connector, network_path) else {
			return TypeSource::Error("Could not get input from connector");
		};

		match input {
			NodeInput::Node { node_id, output_index } => {
				let output_connector = OutputConnector::node(*node_id, *output_index);

				self.output_type(&output_connector, network_path)
			}

			NodeInput::Value { tagged_value, .. } => TypeSource::TaggedValue(tagged_value.ty()),
			NodeInput::Import { import_index, .. } => {
				// Get the input type of the encapsulating node input
				let Some((encapsulating_node, encapsulating_path)) = network_path.split_last() else {
					return TypeSource::Error("Could not get type of import in document network since it has no imports");
				};
				self.input_type(&InputConnector::node_at_index(*encapsulating_node, *import_index), encapsulating_path)
			}
			NodeInput::Scope(_) => TypeSource::Compiled(concrete!(())),
			NodeInput::Reflection(document_node_metadata) => TypeSource::Compiled(document_node_metadata.ty()),
			NodeInput::Inline(_) => TypeSource::Compiled(concrete!(InlineRust)),
		}
	}

	/// Whether the given node has a registered implementation accepting the layer chain's element type as its content input.
	/// A chain awaiting compilation has no resolved type yet, so only a known-wrong type or a type error locks the layer out.
	pub fn layer_chain_hosts_node(&self, node_id: &NodeId, network_path: &[NodeId], node: &ProtoNodeIdentifier) -> bool {
		let secondary_input = InputConnector::layer_secondary_input(*node_id);
		if !self.input_from_connector(&secondary_input, network_path).is_some_and(|input| input.is_exposed()) {
			return false;
		}

		let chain_type = self.input_type(&secondary_input, network_path);
		match chain_type.compiled_element_type() {
			Some(element) => {
				let Some(implementations) = NODE_REGISTRY.get(node) else {
					log::error!("Proto node {node:?} not found in the node registry, in layer_chain_hosts_node");
					return false;
				};
				implementations.keys().any(|node_io| node_io.inputs.first().is_some_and(|content| element_of(content) == element))
			}
			None => !matches!(chain_type, TypeSource::Invalid),
		}
	}

	/// Whether the blending nodes (blend mode, opacity, clipping mask) can be spliced into this layer's chain.
	pub fn layer_hosts_blending_nodes(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		// Blend Mode stands in for the trio since they share one implementations list
		self.layer_chain_hosts_node(node_id, network_path, &graphene_std::blending_nodes::blend_mode::IDENTIFIER)
	}

	/// Whether the Fill and Stroke nodes can be spliced into this layer's chain.
	pub fn layer_hosts_paint_nodes(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		// Fill stands in for both since they share one implementations list
		self.layer_chain_hosts_node(node_id, network_path, &graphene_std::vector_nodes::fill::IDENTIFIER)
	}

	/// Get the [`TypeSource`] for any InputConnector.
	/// If the input is not compiled, then an Unknown or default from the definition is returned.
	pub fn input_type(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> TypeSource {
		// First check if there is an error with this node or any protonodes it is connected to
		if self.input_has_error(input_connector, network_path) {
			return TypeSource::Invalid;
		}
		self.input_type_not_invalid(input_connector, network_path)
	}

	/// Gets the default tagged value for an input. If its not compiled, then it tries to get a valid type. If there are no valid types, then it picks a random implementation.
	pub fn tagged_value_from_input(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> TaggedValue {
		let guaranteed_type = match self.input_type(input_connector, network_path) {
			TypeSource::Compiled(compiled) => compiled,
			TypeSource::TaggedValue(value) => value,
			TypeSource::Unknown | TypeSource::Invalid => {
				// Pick a random type from the complete valid types
				// TODO: Add a NodeInput::Indeterminate which can be resolved at compile time to be any type that prevents an error. This may require bidirectional typing.
				self.complete_valid_input_types(input_connector, network_path)
					.into_iter()
					.min_by_key(|ty| ty.nested_type().identifier_name())
					// Pick a random type from the potential valid types
					.or_else(|| {
						self.potential_valid_input_types(input_connector, network_path)
							.into_iter()
							.min_by_key(|ty| ty.nested_type().identifier_name())
					}).unwrap_or(concrete!(()))
			}
			TypeSource::Error(e) => {
				log::error!("Error getting tagged_value_from_input for {input_connector:?} {e}");
				concrete!(())
			}
		};

		// A List-typed default drops to its Item counterpart (when that has a default value and the connector accepts rank 0),
		// since a stored Item default can promote back onto a List connector but a stored List default can never return to rank 0
		if let Some(element) = guaranteed_type.nested_type().list_element()
			&& let Some(item_type) = self
				.potential_valid_input_types(input_connector, network_path)
				.into_iter()
				.find(|ty| matches!(ty.nested_type(), Type::Item(item_element) if item_element.as_ref() == element))
			&& let Some(item_default) = TaggedValue::from_type(&item_type)
		{
			return item_default;
		}

		TaggedValue::from_type_or_none(&guaranteed_type)
	}

	/// A list of all valid input types for this specific node.
	pub fn potential_valid_input_types(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> Vec<Type> {
		let InputConnector::Node { node_id, input_index } = input_connector else {
			// An export can have any type connected to it
			return vec![graph_craft::generic!(T)];
		};
		let Some(implementation) = self.implementation(node_id, network_path) else {
			log::error!("Could not get node implementation in potential_valid_input_types");
			return Vec::new();
		};
		match implementation {
			DocumentNodeImplementation::Network(_) => {
				let nested_path = [network_path, &[*node_id]].concat();
				let inputs_from_import = self
					.with_outward_wires(&nested_path, |outward_wires| outward_wires.get(&OutputConnector::Import(*input_index)).cloned())
					.flatten();
				let Some(inputs_from_import) = inputs_from_import else {
					log::error!("Could not get inputs from import in potential_valid_input_types");
					return Vec::new();
				};

				self.types_accepted_by_all(&inputs_from_import, &nested_path)
			}
			DocumentNodeImplementation::ProtoNode(proto_node_identifier) => self
				.implementations_matching_inputs(proto_node_identifier, node_id, Some(*input_index), network_path)
				.filter_map(|node_io| node_io.inputs.get(*input_index).cloned())
				.collect::<Vec<_>>(),
			DocumentNodeImplementation::Extract => {
				log::error!("Input types for extract node not supported");
				Vec::new()
			}
		}
	}

	/// Whether a wire of `source_type` can feed the input given the node's other inputs, directly or through what the compiler converts and promotes.
	pub fn input_accepts_type(&self, input_connector: &InputConnector, source_type: &Type, network_path: &[NodeId]) -> bool {
		let InputConnector::Node { node_id, input_index } = input_connector else {
			return self.type_acceptor(input_connector, network_path)(source_type);
		};

		match self.implementation(node_id, network_path) {
			Some(DocumentNodeImplementation::ProtoNode(proto_node_identifier)) => {
				let conversions = input_conversions(proto_node_identifier).get(*input_index).copied().flatten();
				self.implementations_matching_inputs(proto_node_identifier, node_id, Some(*input_index), network_path)
					.filter_map(|node_io| node_io.inputs.get(*input_index))
					.any(|connector| type_feeds_input(source_type, connector, conversions))
			}
			// The wire reaches every interior input fed by the import, so each of them has to accept it
			Some(DocumentNodeImplementation::Network(_)) => {
				let nested_path = [network_path, &[*node_id]].concat();
				let interior_inputs = self
					.with_outward_wires(&nested_path, |outward_wires| outward_wires.get(&OutputConnector::Import(*input_index)).cloned())
					.flatten()
					.unwrap_or_default();

				!interior_inputs.is_empty() && interior_inputs.iter().all(|interior_input| self.type_acceptor(interior_input, &nested_path)(source_type))
			}
			_ => false,
		}
	}

	/// A check of which wire types an input accepts, built once so testing many types searches the node's matching implementations only once.
	/// Unlike [`Self::input_accepts_type`], a node passing its input's type through unchanged (like Monitor) also needs the inputs it feeds to accept the type.
	fn type_acceptor(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> Box<dyn Fn(&Type) -> bool> {
		let accepts_here: Box<dyn Fn(&Type) -> bool> = match input_connector {
			InputConnector::Export(export_index) => match network_path.split_last() {
				// The document network's export is the canvas, which takes anything
				None => Box::new(|_| true),
				// A subgraph's export passes the wire on out of its node's output, so it accepts only what that output feeds in the parent network
				Some((subgraph_node_id, parent_path)) => {
					let consumers = self
						.with_outward_wires(parent_path, |outward_wires| outward_wires.get(&OutputConnector::node(*subgraph_node_id, *export_index)).cloned())
						.flatten()
						.unwrap_or_default();
					let consumer_acceptors = consumers.iter().map(|consumer| self.type_acceptor(consumer, parent_path)).collect::<Vec<_>>();

					Box::new(move |source_type| consumer_acceptors.iter().all(|accepts| accepts(source_type)))
				}
			},
			InputConnector::Node { node_id, input_index } => match self.implementation(node_id, network_path) {
				// A passthrough node takes anything itself, leaving the inputs it feeds to constrain the wire
				Some(DocumentNodeImplementation::ProtoNode(proto_node_identifier)) if *proto_node_identifier == graphene_std::ops::passthrough::IDENTIFIER => Box::new(|_| true),
				Some(DocumentNodeImplementation::ProtoNode(proto_node_identifier)) => {
					let conversions = input_conversions(proto_node_identifier).get(*input_index).copied().flatten();
					let connectors = self
						.implementations_matching_inputs(proto_node_identifier, node_id, Some(*input_index), network_path)
						.filter_map(|node_io| node_io.inputs.get(*input_index))
						.collect::<Vec<_>>();

					Box::new(move |source_type| connectors.iter().any(|connector| type_feeds_input(source_type, connector, conversions)))
				}
				// The wire reaches every interior input fed by the import, so each of them has to accept it
				Some(DocumentNodeImplementation::Network(_)) => {
					let nested_path = [network_path, &[*node_id]].concat();
					let interior_inputs = self
						.with_outward_wires(&nested_path, |outward_wires| outward_wires.get(&OutputConnector::Import(*input_index)).cloned())
						.flatten()
						.unwrap_or_default();
					let interior_acceptors = interior_inputs.iter().map(|interior_input| self.type_acceptor(interior_input, &nested_path)).collect::<Vec<_>>();

					Box::new(move |source_type| !interior_acceptors.is_empty() && interior_acceptors.iter().all(|accepts| accepts(source_type)))
				}
				_ => Box::new(|_| false),
			},
		};

		let Some(fed_inputs) = self.inputs_fed_by_type_preserving_node(input_connector, network_path) else {
			return accepts_here;
		};
		let fed_acceptors = fed_inputs.iter().map(|fed_input| self.type_acceptor(fed_input, network_path)).collect::<Vec<_>>();
		Box::new(move |source_type| accepts_here(source_type) && fed_acceptors.iter().all(|accepts| accepts(source_type)))
	}

	/// The inputs fed by the node of `input_connector`, if that node passes its input's type through unchanged, since those constrain a wire through it more
	/// than the node's own broad set of types.
	fn inputs_fed_by_type_preserving_node(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> Option<Vec<InputConnector>> {
		let InputConnector::Node { node_id, .. } = input_connector else { return None };
		let Some(DocumentNodeImplementation::ProtoNode(proto_node_identifier)) = self.implementation(node_id, network_path) else {
			return None;
		};
		if !TYPE_PRESERVING_NODES.contains(proto_node_identifier) {
			return None;
		}

		let fed_inputs = self
			.with_outward_wires(network_path, |outward_wires| outward_wires.get(&OutputConnector::primary_output(*node_id)).cloned())
			.flatten();
		Some(fed_inputs.unwrap_or_default())
	}

	/// The types that every one of `input_connectors` accepts, which is what a single wire feeding all of them can carry.
	fn types_accepted_by_all(&self, input_connectors: &[InputConnector], network_path: &[NodeId]) -> Vec<Type> {
		input_connectors
			.iter()
			.map(|input_connector| {
				let valid_types = self.potential_valid_input_types(input_connector, network_path).into_iter();
				match self.inputs_fed_by_type_preserving_node(input_connector, network_path) {
					// A node passing its input's type through unchanged accepts only what the inputs it feeds accept
					Some(fed_inputs) => {
						let fed_acceptors = fed_inputs.iter().map(|fed_input| self.type_acceptor(fed_input, network_path)).collect::<Vec<_>>();
						valid_types.filter(|valid_type| fed_acceptors.iter().all(|accepts| accepts(valid_type))).collect::<HashSet<_>>()
					}
					None => valid_types.collect(),
				}
			})
			.fold(None, |acc: Option<HashSet<Type>>, set| match acc {
				Some(acc_set) => Some(acc_set.intersection(&set).cloned().collect()),
				None => Some(set),
			})
			.unwrap_or_default()
			.into_iter()
			.collect()
	}

	/// The types an output could produce: its compiled type, or else what [`Self::possible_output_types`] finds.
	pub fn potential_output_types(&self, output_connector: &OutputConnector, network_path: &[NodeId]) -> Vec<Type> {
		if let Some(compiled_type) = self.output_type(output_connector, network_path).compiled_nested_type() {
			return vec![compiled_type.clone()];
		}

		// Nodes outside the compiled graph, such as disconnected ones, have no resolved type
		self.possible_output_types(output_connector, network_path)
	}

	/// Every type an output could produce given its node's current inputs, regardless of the one it was compiled with. An import can carry whatever its
	/// subgraph node's input accepts. Only the primary output of a proto node is known, since a node's other outputs are fields of its return value.
	pub fn possible_output_types(&self, output_connector: &OutputConnector, network_path: &[NodeId]) -> Vec<Type> {
		match output_connector {
			OutputConnector::Import(import_index) => {
				let Some((encapsulating_node_id, encapsulating_path)) = network_path.split_last() else {
					return Vec::new();
				};
				self.potential_valid_input_types(&InputConnector::node_at_index(*encapsulating_node_id, *import_index), encapsulating_path)
			}
			OutputConnector::Node { node_id, output_index } => match self.implementation(node_id, network_path) {
				Some(DocumentNodeImplementation::ProtoNode(proto_node_identifier)) if *output_index == 0 => self
					.implementations_matching_inputs(proto_node_identifier, node_id, None, network_path)
					.map(|node_io| node_io.return_value.nested_type().clone())
					.collect(),
				// A subgraph's output carries whatever feeds its export
				Some(DocumentNodeImplementation::Network(_)) => {
					let nested_path = [network_path, &[*node_id]].concat();
					let export = InputConnector::Export(*output_index);
					match self.upstream_output_connector(&export, &nested_path) {
						Some(upstream_output) => self.possible_output_types(&upstream_output, &nested_path),
						// A value given to the export, unless it's a new export's empty placeholder
						None => self
							.input_type(&export, &nested_path)
							.compiled_nested_type()
							.filter(|value_type| **value_type != concrete!(()))
							.cloned()
							.into_iter()
							.collect(),
					}
				}
				_ => Vec::new(),
			},
		}
	}

	/// The registry implementations of a proto node whose inputs accept the node's current input types, ignoring `disregarded_input_index`.
	fn implementations_matching_inputs<'a>(
		&'a self,
		proto_node_identifier: &ProtoNodeIdentifier,
		node_id: &'a NodeId,
		disregarded_input_index: Option<usize>,
		network_path: &'a [NodeId],
	) -> impl Iterator<Item = &'static NodeIOTypes> + 'a {
		let implementations = NODE_REGISTRY.get(proto_node_identifier);

		// The compiler removes the passthrough node, so it's expected to be absent from the registry
		if implementations.is_none() && proto_node_identifier != &graphene_std::ops::passthrough::IDENTIFIER {
			log::error!("Proto node `{proto_node_identifier:?}` not found in the node registry");
		}

		let number_of_inputs = self.number_of_inputs(node_id, network_path);
		let conversions = input_conversions(proto_node_identifier);
		implementations.into_iter().flat_map(|implementations| implementations.keys()).filter(move |node_io| {
			(0..number_of_inputs).filter(|index| Some(*index) != disregarded_input_index).all(|index| {
				let input_type = self.input_type_not_invalid(&InputConnector::node_at_index(*node_id, index), network_path);
				// TODO: Fix type checking for different call arguments
				// For example a node input of (Footprint) -> Vector would not be compatible with a node that is called with () and returns Vector
				match (input_type.compiled_nested_type(), node_io.inputs.get(index)) {
					(Some(source), Some(connector)) => type_feeds_input(source, connector, conversions.get(index).copied().flatten()),
					(None, None) => true,
					_ => false,
				}
			})
		})
	}

	/// Performs a downstream traversal to ensure input type will work in the full context of the graph.
	pub fn complete_valid_input_types(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> Vec<Type> {
		match input_connector {
			InputConnector::Node { node_id, input_index } => {
				let Some(implementation) = self.implementation(node_id, network_path) else {
					log::error!("Could not get node implementation for {:?} {} in complete_valid_input_types", network_path, *node_id);
					return Vec::new();
				};
				match implementation {
					DocumentNodeImplementation::Network(_) => self.valid_output_types(&OutputConnector::Import(input_connector.input_index()), &[network_path, &[*node_id]].concat()),
					DocumentNodeImplementation::ProtoNode(proto_node_identifier) => {
						let Some(implementations) = NODE_REGISTRY.get(proto_node_identifier) else {
							log::error!("Protonode {proto_node_identifier:?} not found in registry in complete_valid_input_types");
							return Vec::new();
						};
						let valid_output_types = self.valid_output_types(&OutputConnector::primary_output(*node_id), network_path);

						implementations
							.keys()
							.filter_map(|node_io| {
								if !valid_output_types.iter().any(|output_type| output_type.nested_type() == node_io.return_value.nested_type()) {
									return None;
								}

								let valid_inputs = (0..node_io.inputs.len()).filter(|iterator_index| iterator_index != input_index).all(|iterator_index| {
									let input_type = self.input_type_not_invalid(&InputConnector::node_at_index(*node_id, iterator_index), network_path);
									match input_type.compiled_nested_type() {
										Some(input_type) => node_io.inputs.get(iterator_index).is_some_and(|node_io_input_type| node_io_input_type.nested_type() == input_type),
										None => true,
									}
								});
								if valid_inputs { node_io.inputs.get(*input_index).cloned() } else { None }
							})
							.collect::<Vec<_>>()
					}
					DocumentNodeImplementation::Extract => Vec::new(),
				}
			}
			InputConnector::Export(export_index) => {
				match network_path.split_last() {
					Some((encapsulating_node, encapsulating_path)) => self.valid_output_types(&OutputConnector::node(*encapsulating_node, *export_index), encapsulating_path),
					None => {
						// Valid types for the export are all types that can be fed into the render node
						let render_node = graphene_std::render_node::render_intermediate::IDENTIFIER;
						let Some(implementations) = NODE_REGISTRY.get(&render_node) else {
							log::error!("Protonode {render_node:?} not found in registry");
							return Vec::new();
						};
						implementations.keys().filter_map(|types| types.inputs.first().cloned()).collect()
					}
				}
			}
		}
	}

	/// Whether a multi-output node is framed over a list, which happens when a list reaches any input its single-item variant takes as an `Item`.
	fn multi_output_node_is_mapped(&self, node_id: &NodeId, identifier: &ProtoNodeIdentifier, mapped_type: &Type, network_path: &[NodeId]) -> bool {
		let Some(implementations) = NODE_REGISTRY.get(identifier) else { return false };
		let Some(single_item_variant) = implementations.keys().find(|node_io| node_io.return_value.nested_type() != mapped_type) else {
			return false;
		};

		single_item_variant
			.inputs
			.iter()
			.enumerate()
			.any(|(input_index, declared_type)| matches!(declared_type.nested_type(), Type::Item(_)) && self.input_type(&InputConnector::node_at_index(*node_id, input_index), network_path).is_list())
	}

	pub fn output_type(&self, output_connector: &OutputConnector, network_path: &[NodeId]) -> TypeSource {
		match output_connector {
			OutputConnector::Node { node_id, output_index } => {
				// A hidden node is replaced by a passthrough during flattening, so its output carries its primary input's type
				if *output_index == 0 && !self.is_visible(node_id, network_path) {
					return self.input_type(&InputConnector::primary_input(*node_id), network_path);
				}

				// First try iterating upstream to the first protonode and try get its compiled type
				let Some(implementation) = self.implementation(node_id, network_path) else {
					return TypeSource::Error("Could not get implementation");
				};
				match implementation {
					DocumentNodeImplementation::Network(_) => self.input_type(&InputConnector::Export(*output_index), &[network_path, &[*node_id]].concat()),
					// The compiler removes passthrough nodes so they resolve no type of their own, but their output carries their primary input's type
					DocumentNodeImplementation::ProtoNode(identifier) if *identifier == graphene_std::ops::passthrough::IDENTIFIER => {
						self.input_type(&InputConnector::primary_input(*node_id), network_path)
					}
					DocumentNodeImplementation::ProtoNode(identifier) => {
						// A multi-output proto node's field output types are recorded in the registry, as lists in its mapped variant.
						// The hidden primary output has no field, so it falls through to the compiled types below.
						if let Some(metadata) = graphene_std::registry::MULTI_OUTPUT_NODES.get(identifier)
							&& let Some(field_index) = metadata.field_index_for_output(*output_index)
						{
							return match metadata.fields.get(field_index) {
								Some(field) => {
									let is_mapped = self.multi_output_node_is_mapped(node_id, identifier, &metadata.mapped_type, network_path);
									TypeSource::Compiled(if is_mapped { field.mapped_ty.clone() } else { field.ty.clone() })
								}
								None => TypeSource::Error("Output index out of range for proto node"),
							};
						}

						match self.resolved_types.types.get(&[network_path, &[*node_id]].concat()) {
							Some(resolved_type) => TypeSource::Compiled(resolved_type.output.clone()),
							None => TypeSource::Unknown,
						}
					}
					DocumentNodeImplementation::Extract => TypeSource::Compiled(concrete!(())),
				}
			}
			OutputConnector::Import(import_index) => {
				let Some((encapsulating_node, encapsulating_path)) = network_path.split_last() else {
					return TypeSource::Error("Cannot get import type in document network since it has no imports");
				};
				let mut input_type = self.input_type(&InputConnector::node_at_index(*encapsulating_node, *import_index), encapsulating_path);
				if matches!(input_type, TypeSource::Invalid) {
					input_type = TypeSource::Unknown
				}
				input_type
			}
		}
	}

	/// The valid output types are all types that are valid for each downstream connection.
	fn valid_output_types(&self, output_connector: &OutputConnector, network_path: &[NodeId]) -> Vec<Type> {
		let inputs_from_import = self.with_outward_wires(network_path, |outward_wires| outward_wires.get(output_connector).cloned()).flatten();
		let Some(inputs_from_import) = inputs_from_import else {
			log::error!("Could not get inputs from import in valid_output_types");
			return Vec::new();
		};

		self.types_accepted_by_all(&inputs_from_import, network_path)
	}
}

#[cfg(test)]
mod tests {
	use super::{input_conversions, type_feeds_input};
	use graph_craft::{item, list};
	use graphene_std::vector::fill::{ContentInput, PaintInput};
	use graphene_std::{Color, Graphic, NodeParameter};

	#[test]
	fn only_inputs_with_an_input_adapter_convert() {
		let fill_conversions = input_conversions(&PaintInput::NODE_IDENTIFIER);
		let paint_conversions = fill_conversions.get(PaintInput::INDEX).copied().flatten();
		let content_conversions = fill_conversions.get(ContentInput::INDEX).copied().flatten();

		assert!(type_feeds_input(&item!(f64), &item!(f64), None), "identical types should match without an input adapter");
		assert!(
			type_feeds_input(&item!(Color), &item!(Graphic), paint_conversions),
			"a color should feed Fill's paint input, registered with one type, through its input adapter"
		);
		assert!(
			!type_feeds_input(&item!(Color), &item!(Graphic), content_conversions),
			"Fill's content input is registered with several types, so it gets no input adapter to convert a color"
		);
		assert!(!type_feeds_input(&item!(f64), &item!(Graphic), paint_conversions), "a number has no conversion into a graphic");
	}

	#[test]
	fn items_promote_onto_list_inputs() {
		let paint_conversions = input_conversions(&PaintInput::NODE_IDENTIFIER).get(PaintInput::INDEX).copied().flatten();

		assert!(type_feeds_input(&item!(f64), &list!(f64), None), "a number should be raised onto a list input as a one-element list");
		assert!(
			type_feeds_input(&item!(Color), &list!(Graphic), paint_conversions),
			"a color should feed Fill's list paint input by converting through its input adapter and then being raised to a list"
		);
		assert!(!type_feeds_input(&list!(f64), &item!(f64), None), "a list has no promotion down to an item");
	}

	#[tokio::test]
	async fn subgraph_inputs_look_through_type_preserving_nodes() {
		use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
		use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
		use crate::test_utils::test_prelude::*;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let path_node_id = editor.create_node_by_name(DefinitionIdentifier::Network("Path".into())).await;

		// The Path subgraph's content goes through a Monitor node, which accepts most types, before the node that needs a vector
		let content_types = editor
			.active_document()
			.network_interface
			.potential_valid_input_types(&InputConnector::node_at_index(path_node_id, 0), &[])
			.iter()
			.map(ToString::to_string)
			.collect::<Vec<_>>();
		assert!(content_types.iter().any(|ty| ty == "Vector"), "Path's content should accept a vector, found {content_types:?}");
		assert!(
			!content_types.iter().any(|ty| ty == "Color"),
			"Path's content should not accept a color just because its Monitor does, found {content_types:?}"
		);
	}

	#[tokio::test]
	async fn a_subgraph_input_passed_through_to_its_export_accepts_only_what_the_subgraph_output_feeds() {
		use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
		use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, OutputConnector};
		use crate::test_utils::test_prelude::*;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let merge = editor.create_node_by_name_at(DefinitionIdentifier::Network("Merge".into()), 0, 0).await;
		let fill = editor
			.create_node_by_name_at(DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::fill::IDENTIFIER), 20, 0)
			.await;
		let fill_content = InputConnector::node_at_index(fill, 0);
		editor
			.handle_message(NodeGraphMessage::CreateWire {
				output_connector: OutputConnector::node(merge, 0),
				input_connector: fill_content,
			})
			.await;

		// A new import passed straight through to Merge's export, which the editor bridges with a Passthrough node
		editor.handle_message(DocumentMessage::EnterNestedNetwork { node_id: merge }).await;
		let import_index = editor.active_document().network_interface.number_of_imports(&[merge]);
		editor
			.handle_message(NodeGraphMessage::AddImportAndConnect {
				import_index,
				input_connector: InputConnector::Export(0),
			})
			.await;

		let network_interface = &editor.active_document().network_interface;
		let passed_through = InputConnector::node_at_index(merge, import_index);
		let fill_content_type = network_interface
			.potential_valid_input_types(&fill_content, &[])
			.first()
			.cloned()
			.expect("Fill's content should accept some type");
		assert!(
			network_interface.input_accepts_type(&passed_through, &fill_content_type, &[]),
			"the input should accept what the Fill fed by Merge's output takes"
		);
		assert!(
			!network_interface.input_accepts_type(&passed_through, &item!(f64), &[]),
			"the input should reject a number, which the Fill fed by Merge's output can't take"
		);
	}
}
