use super::DocumentNodeDefinition;
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{
	InputMetadata, NodeNetworkTemplate, NodeTemplate, NodeTemplateImplementation, NodeTypePersistentMetadata, WidgetOverride,
};
use glam::IVec2;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graphene_std::registry::*;
use graphene_std::*;
use std::collections::{HashMap, HashSet};

/// Generates every definition, from the node macro's metadata for proto nodes and the graph macro's for built-in subgraphs.
pub(super) fn derive_definitions() -> HashMap<DefinitionIdentifier, DocumentNodeDefinition> {
	let mut definitions_map = HashMap::new();

	// Add the protonodes from the macro.
	// Typed nodes are registered in `core_types::NODE_REGISTRY` via the macro's auto-generated `register_node` codegen.
	// `skip_impl` nodes (e.g. Cache) bypass that registration but are wired up manually in
	// `interpreted_executor::node_registry::NODE_REGISTRY` via `async_node!`. We consult that extended registry as a
	// fallback when deriving `call_argument` so it reflects the impls actually registered, which will usually be `Context`.
	let extended_node_registry = &*interpreted_executor::node_registry::NODE_REGISTRY;
	let node_registry = NODE_REGISTRY.lock().unwrap();
	let empty_implementations: Vec<(NodeConstructor, NodeIOTypes)> = Vec::new();
	let context_type = concrete!(Context);
	for (id, metadata) in NODE_METADATA.lock().unwrap().iter() {
		let identifier = DefinitionIdentifier::ProtoNode(id.clone());
		let NodeMetadata {
			display_name,
			category,
			icon,
			fields,
			description,
			inspector,
			output_fields,
			version,
			..
		} = metadata;

		let implementations = node_registry.get(id).unwrap_or(&empty_implementations);

		let first_node_io = implementations.first().map(|(_, node_io)| node_io).unwrap_or(const { &NodeIOTypes::empty() });

		let call_arguments: Vec<&Type> = if !implementations.is_empty() {
			implementations.iter().map(|(_, io)| &io.call_argument).collect()
		} else if let Some(impls) = extended_node_registry.get(id) {
			impls.keys().map(|io| &io.call_argument).collect()
		} else {
			Vec::new()
		};
		let valid_inputs: HashSet<&Type> = call_arguments.iter().copied().collect();
		let input_type = if valid_inputs.is_empty() {
			&context_type
		} else if valid_inputs.len() > 1 {
			&const { generic!(D) }
		} else {
			call_arguments[0]
		};

		let inputs = preprocessor::node_inputs(fields, first_node_io);

		// A multi-output node names each output after its field, leaving the hidden primary output unnamed
		let output_names = output_fields
			.as_ref()
			.map(|destructure| {
				(0..destructure.number_of_outputs())
					.map(|output_index| {
						let field = destructure.field_index_for_output(output_index).and_then(|field_index| destructure.fields.get(field_index));
						field.map_or_else(String::new, |field| field.name.to_string())
					})
					.collect()
			})
			.unwrap_or_default();

		definitions_map.insert(
			identifier,
			DocumentNodeDefinition {
				identifier: display_name,
				version: *version,
				node_template: NodeTemplate {
					inputs,
					call_argument: input_type.clone(),
					implementation: NodeTemplateImplementation::ProtoNode(id.clone()),
					input_metadata: input_metadata(fields),
					output_names,
					..Default::default()
				},
				category,
				icon: *icon,
				description: Cow::Borrowed(description),
				inspector: *inspector,
			},
		);
	}

	let graphs = GRAPH_METADATA.lock().unwrap().clone();
	add_graph_definitions(graphs, &mut definitions_map);

	definitions_map
}

/// The name, description, and any widget override of each parameter, for a proto node's fields or a graph's imports.
/// The rest of a parameter's widget settings are read from its field when the Inspector draws it.
fn input_metadata(fields: &[FieldMetadata]) -> Vec<InputMetadata> {
	fields
		.iter()
		.map(|field| {
			let (name, description) = (field.name, field.description);
			match field.widget_override {
				RegistryWidgetOverride::None => (name, description).into(),
				RegistryWidgetOverride::Hidden => InputMetadata::with_name_description_override(name, description, WidgetOverride::Hidden),
				RegistryWidgetOverride::String(text) => InputMetadata::with_name_description_override(name, description, WidgetOverride::String(text.to_string())),
				RegistryWidgetOverride::Custom(key) => InputMetadata::with_name_description_override(name, description, WidgetOverride::Custom(key.to_string())),
			}
		})
		.collect()
}

/// Why a `#[graph]` node has no definition yet, or can't have one.
enum GraphDefinitionError {
	/// A node inside it has no definition, which may be another graph not yet added.
	MissingNode(String),
	/// The graph's wiring is invalid, so it's left out of the catalog.
	Invalid(String),
}

/// Adds the nodes defined as subgraphs by `#[graph]`, each once the definitions of the nodes inside it exist.
fn add_graph_definitions(graphs: Vec<GraphMetadata>, definitions_map: &mut HashMap<DefinitionIdentifier, DocumentNodeDefinition>) {
	// Every graph sharing an identifier is left out, so the catalog doesn't depend on registration order
	let mut identifier_counts = HashMap::new();
	for graph in &graphs {
		*identifier_counts.entry(graph.identifier).or_insert(0) += 1;
	}
	let mut rejected = HashSet::new();
	let mut pending_graphs: Vec<(GraphMetadata, String)> = graphs
		.into_iter()
		.filter(|graph| {
			let taken = identifier_counts[graph.identifier] > 1;
			if taken {
				log::error!("The '{}' graph has no definition, since another graph has the same identifier", graph.identifier);
				rejected.insert(graph.identifier);
			}
			!taken
		})
		.map(|graph| (graph, String::new()))
		.collect();

	loop {
		let pending_count = pending_graphs.len();

		let mut ready = Vec::new();
		pending_graphs.retain_mut(|(graph, missing)| {
			// A graph built on a left-out graph is left out too, rather than embedding another definition of that name
			let rejected_inner_graph = graph.nodes.iter().find_map(|node| match node.implementation {
				GraphNodeImplementation::Graph(identifier) if rejected.contains(identifier) => Some(identifier),
				_ => None,
			});
			if let Some(inner_graph) = rejected_inner_graph {
				log::error!("The '{}' graph has no definition, since the '{inner_graph}' graph inside it has none", graph.identifier);
				rejected.insert(graph.identifier);
				return false;
			}

			match graph_definition(graph, definitions_map) {
				Ok(definition) => {
					ready.push(definition);
					false
				}
				Err(GraphDefinitionError::MissingNode(missing_node)) => {
					*missing = missing_node;
					true
				}
				Err(GraphDefinitionError::Invalid(reason)) => {
					log::error!("The '{}' graph has no definition, since {reason}", graph.identifier);
					rejected.insert(graph.identifier);
					false
				}
			}
		});
		for definition in ready {
			let definition = prepare_network_definition(definition, definitions_map);
			definitions_map.insert(DefinitionIdentifier::Network(definition.identifier.to_string()), definition);
		}

		if pending_graphs.is_empty() || pending_graphs.len() == pending_count {
			break;
		}
	}
	for (graph, missing_node) in pending_graphs {
		log::error!("The '{}' graph has no definition, since the '{missing_node}' node inside it has none", graph.identifier);
	}
}

/// Fills a network definition's nested proto node metadata and points its network at the definition.
fn prepare_network_definition(mut network_node: DocumentNodeDefinition, definitions_map: &HashMap<DefinitionIdentifier, DocumentNodeDefinition>) -> DocumentNodeDefinition {
	fill_proto_node_metadata(&mut network_node.node_template, definitions_map);

	// Set the reference to the node identifier
	if let NodeTemplateImplementation::Network(network_template) = &mut network_node.node_template.implementation {
		network_template.reference = Some(network_node.identifier.to_string());
		// If it is not a merge node, then set the display name to the identifier/reference
		if network_node.identifier != "Merge" {
			network_node.node_template.display_name = network_node.identifier.to_string();
		}
	}

	network_node
}

/// Builds the catalog definition of a node defined as a subgraph by `#[graph]`.
fn graph_definition(graph: &GraphMetadata, definitions_map: &HashMap<DefinitionIdentifier, DocumentNodeDefinition>) -> Result<DocumentNodeDefinition, GraphDefinitionError> {
	let mut nodes = HashMap::new();
	for (index, node) in graph.nodes.iter().enumerate() {
		// Start from the node's own definition, so each input the graph leaves out matches a new instance of it
		let (identifier, name) = match &node.implementation {
			GraphNodeImplementation::ProtoNode(identifier) => (DefinitionIdentifier::ProtoNode(identifier.clone()), identifier.as_str().to_string()),
			GraphNodeImplementation::Graph(identifier) => (DefinitionIdentifier::Network(identifier.to_string()), identifier.to_string()),
		};
		let Some(definition) = definitions_map.get(&identifier) else {
			return Err(GraphDefinitionError::MissingNode(name));
		};
		let mut template = definition.default_node_template();

		if let Some(call_argument) = &node.call_argument {
			template.call_argument = call_argument.clone();
		}
		template.visible = !node.hidden;
		template.locked = node.locked;
		template.pinned = node.pinned;
		if let Some(display_name) = node.display_name {
			template.display_name = display_name.to_string();
		}

		let position = node.position.map_or(IVec2::ZERO, |(x, y)| IVec2::new(x, y));
		template.node_type_metadata = if node.layer {
			NodeTypePersistentMetadata::layer(position)
		} else {
			NodeTypePersistentMetadata::node(position)
		};

		for (input_index, source) in &node.inputs {
			let Some(input) = template.inputs.get_mut(*input_index) else {
				return Err(GraphDefinitionError::Invalid(format!("its node {index}, '{name}', has no input {input_index} for it to set")));
			};

			// An input the compiler fills in can't be set, which the macro already prevents for the graphs it builds
			if matches!(input, NodeInput::Scope(_) | NodeInput::Reflection(_)) {
				return Err(GraphDefinitionError::Invalid(format!(
					"it sets input {input_index} of its node {index}, '{name}', which the compiler fills in"
				)));
			}

			*input = graph_input(source, input);
		}

		nodes.insert(NodeId(index as u64), template);
	}

	// Each import starts from its `#[default(...)]` or else its type, as a proto node's parameter does
	let inputs = preprocessor::node_inputs(&graph.imports, &NodeIOTypes::empty());

	let exports = graph.exports.iter().map(|export| graph_input(&export.source, &NodeInput::value(TaggedValue::None, false))).collect();

	Ok(DocumentNodeDefinition {
		identifier: graph.identifier,
		version: graph.version,
		category: graph.category,
		icon: graph.icon,
		description: Cow::Borrowed(graph.description),
		inspector: None,
		node_template: NodeTemplate {
			implementation: NodeTemplateImplementation::Network(NodeNetworkTemplate { exports, nodes, ..Default::default() }),
			inputs,
			input_metadata: input_metadata(&graph.imports),
			output_names: graph.exports.iter().map(|export| export.name.to_string()).collect(),
			output_descriptions: graph.exports.iter().map(|export| export.description.to_string()).collect(),
			node_type_metadata: if graph.layer {
				NodeTypePersistentMetadata::layer(IVec2::ZERO)
			} else {
				NodeTypePersistentMetadata::node(IVec2::ZERO)
			},
			..Default::default()
		},
	})
}

/// The input a graph definition writes, where `default` is what the node's own definition starts that input with.
fn graph_input(source: &GraphInputSource, default: &NodeInput) -> NodeInput {
	match source {
		GraphInputSource::Node { node, output } => NodeInput::node(NodeId(*node as u64), *output),
		GraphInputSource::Import { index, ty } => NodeInput::import(ty.clone(), *index),
		GraphInputSource::Value { value, exposed } => {
			let ty = match default {
				NodeInput::Value { tagged_value, .. } => tagged_value.ty(),
				NodeInput::Import { import_type, .. } => import_type.clone(),
				_ => generic!(T),
			};
			NodeInput::value(graph_value(value, &ty), exposed.unwrap_or(default.is_exposed()))
		}
		GraphInputSource::Scope(key) => NodeInput::scope(*key),
		GraphInputSource::NodePath => NodeInput::Reflection(graph_craft::document::DocumentNodeMetadata::DocumentNodePath),
		GraphInputSource::SourceId => NodeInput::Reflection(graph_craft::document::DocumentNodeMetadata::SourceId),
	}
}

/// The type of the elements a wire of this type carries, past any `Item`, `List`, or lazy wrapping.
fn element_type(ty: &Type) -> &Type {
	let mut element = ty;
	while let Type::Item(inner) | Type::List(inner) | Type::Fn(_, inner) = element {
		element = inner;
	}
	element
}

/// Converts a value written in a graph definition to the type of the input it feeds.
fn graph_value(value: &GraphValue, ty: &Type) -> TaggedValue {
	// An integer literal feeds an integer input as an integer, and any other as a number
	let is_integer = *element_type(ty) == concrete!(i64);

	match value {
		GraphValue::Integer(integer) if is_integer => TaggedValue::Integer(*integer),
		GraphValue::Integer(integer) => TaggedValue::Number(*integer as f64),
		GraphValue::Number(number) => TaggedValue::Number(*number),
		GraphValue::Bool(boolean) => TaggedValue::Bool(*boolean),
		GraphValue::String(string) => TaggedValue::String(string.clone()),
		GraphValue::Vec2(vec2) => TaggedValue::DVec2(*vec2),
		GraphValue::Color(color) => TaggedValue::Color(*color),
	}
}

/// Recursively gives each nested proto node its definition's input and output metadata, and its display name if the network's author gave none.
fn fill_proto_node_metadata(node_template: &mut NodeTemplate, definitions_map: &HashMap<DefinitionIdentifier, DocumentNodeDefinition>) {
	match &mut node_template.implementation {
		NodeTemplateImplementation::Network(network_template) => {
			for nested_template in network_template.nodes.values_mut() {
				fill_proto_node_metadata(nested_template, definitions_map);
			}
		}
		NodeTemplateImplementation::ProtoNode(id) => {
			// If this lookup fails then the proto node id in the definition doesn't match what is generated by the macro
			let Some(definition) = definitions_map.get(&DefinitionIdentifier::ProtoNode(id.clone())) else {
				return;
			};

			// A display name, lock, or pin the network's author gave the node is kept
			let definition_template = &definition.node_template;
			if node_template.display_name.is_empty() {
				node_template.display_name = definition_template.display_name.clone();
			}
			node_template.input_metadata = definition_template.input_metadata.clone();
			node_template.output_names = definition_template.output_names.clone();
			node_template.locked |= definition_template.locked;
			node_template.pinned |= definition_template.pinned;
		}
		NodeTemplateImplementation::Definition(_) => {}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;

	fn number_import() -> FieldMetadata {
		FieldMetadata {
			name: "Value",
			description: "",
			internal: false,
			exposed: true,
			widget_override: RegistryWidgetOverride::None,
			value_source: RegistryValueSource::None,
			default_colors: None,
			default_type: Some(Type::Item(Box::new(concrete!(f64)))),
			number_soft_min: None,
			number_soft_max: None,
			number_hard_min: None,
			number_hard_max: None,
			number_mode_range: false,
			number_display_decimal_places: None,
			number_step: None,
			unit: None,
			multiline: false,
			progression: false,
			constant: false,
			labels: None,
			integer: false,
			blank_assist: true,
		}
	}

	/// A graph exporting one node, which is fed by the given inputs.
	fn one_node_graph(identifier: &'static str, implementation: GraphNodeImplementation, inputs: Vec<(usize, GraphInputSource)>) -> GraphMetadata {
		GraphMetadata {
			identifier,
			category: "Test",
			icon: None,
			description: "",
			version: 1,
			layer: false,
			imports: vec![number_import()],
			exports: vec![GraphExportMetadata {
				name: "Out",
				description: "",
				source: GraphInputSource::Node { node: 0, output: 0 },
			}],
			nodes: vec![GraphNodeMetadata {
				implementation,
				inputs,
				position: None,
				layer: false,
				display_name: None,
				hidden: false,
				locked: false,
				pinned: false,
				call_argument: None,
			}],
		}
	}

	fn from_import() -> Vec<(usize, GraphInputSource)> {
		let ty = Type::Item(Box::new(concrete!(f64)));
		vec![(0, GraphInputSource::Import { index: 0, ty })]
	}

	/// The definitions a test graph builds on: only the As Number proto node.
	fn proto_definitions() -> HashMap<DefinitionIdentifier, DocumentNodeDefinition> {
		let identifier = DefinitionIdentifier::ProtoNode(math_nodes::as_number::IDENTIFIER);
		let definition = resolve_document_node_type(&identifier).expect("As Number should have a definition").clone();
		HashMap::from([(identifier, definition)])
	}

	fn has_graph(definitions_map: &HashMap<DefinitionIdentifier, DocumentNodeDefinition>, identifier: &str) -> bool {
		definitions_map.contains_key(&DefinitionIdentifier::Network(identifier.to_string()))
	}

	#[test]
	fn a_graph_resolves_when_registered_before_a_graph_inside_it() {
		let outer = one_node_graph("Test Outer", GraphNodeImplementation::Graph("Test Inner"), from_import());
		let inner = one_node_graph("Test Inner", math_nodes::as_number::IDENTIFIER.into(), from_import());

		let mut definitions_map = proto_definitions();
		add_graph_definitions(vec![outer, inner], &mut definitions_map);

		assert!(has_graph(&definitions_map, "Test Inner"));
		assert!(has_graph(&definitions_map, "Test Outer"));
	}

	#[test]
	fn a_graph_setting_an_input_its_node_lacks_is_left_out() {
		let mut inputs = from_import();
		inputs.push((
			100,
			GraphInputSource::Value {
				value: GraphValue::Number(1.),
				exposed: None,
			},
		));
		let graph = one_node_graph("Test Missing Input", math_nodes::as_number::IDENTIFIER.into(), inputs);

		let mut definitions_map = proto_definitions();
		add_graph_definitions(vec![graph], &mut definitions_map);

		assert!(!has_graph(&definitions_map, "Test Missing Input"));
	}

	#[test]
	fn graphs_sharing_an_identifier_and_graphs_built_on_them_are_left_out() {
		let first = one_node_graph("Test Twin", math_nodes::as_number::IDENTIFIER.into(), from_import());
		let second = one_node_graph("Test Twin", math_nodes::as_number::IDENTIFIER.into(), from_import());
		let dependent = one_node_graph("Test Dependent", GraphNodeImplementation::Graph("Test Twin"), from_import());

		let mut definitions_map = proto_definitions();
		add_graph_definitions(vec![first, second, dependent], &mut definitions_map);

		assert!(!has_graph(&definitions_map, "Test Twin"));
		assert!(!has_graph(&definitions_map, "Test Dependent"));
	}

	#[test]
	fn an_integer_value_takes_the_type_of_the_input_it_feeds() {
		let integer = Type::Item(Box::new(concrete!(i64)));
		let number = Type::Item(Box::new(concrete!(f64)));

		assert!(matches!(graph_value(&GraphValue::Integer(3), &integer), TaggedValue::Integer(3)));
		assert!(matches!(graph_value(&GraphValue::Integer(3), &number), TaggedValue::Number(value) if value == 3.));
	}

	#[test]
	fn imports_start_from_their_default_or_type() {
		let unit_primary = FieldMetadata {
			default_type: Some(concrete!(())),
			..number_import()
		};
		let with_default = FieldMetadata {
			value_source: RegistryValueSource::Default("2.5"),
			..number_import()
		};
		let graph = GraphMetadata {
			imports: vec![unit_primary, with_default],
			..one_node_graph("Test Defaults", math_nodes::as_number::IDENTIFIER.into(), from_import())
		};

		let mut definitions_map = proto_definitions();
		add_graph_definitions(vec![graph], &mut definitions_map);
		let inputs = &definitions_map[&DefinitionIdentifier::Network("Test Defaults".to_string())].node_template.inputs;

		// A `()` primary has nothing to connect, and an exposed parameter's default is parsed by its type
		assert!(!inputs[0].is_exposed());
		assert!(matches!(inputs[1].as_value(), Some(TaggedValue::Number(value)) if *value == 2.5));
		assert!(inputs[1].is_exposed());
	}

	#[test]
	fn a_graph_nodes_widget_settings_come_from_its_import() {
		use crate::messages::prelude::DocumentMessageHandler;

		let node_id = NodeId(1);
		let mut document = DocumentMessageHandler::default();
		let artboard = resolve_document_node_type(&DefinitionIdentifier::Network("Artboard".into())).expect("Artboard should exist");
		document.network_interface.insert_node(node_id, artboard.default_node_template(), &[]);

		let location = graphene_std::subgraphs::artboard::LocationInput::INDEX;
		let field = document
			.network_interface
			.field_metadata(&node_id, location, &[])
			.expect("the location input should have its import's settings");
		assert_eq!(field.labels, Some(("X", "Y")));
		assert!(field.integer);
		assert!(!document.network_interface.is_internal_input(&node_id, location, &[]));
	}
}
