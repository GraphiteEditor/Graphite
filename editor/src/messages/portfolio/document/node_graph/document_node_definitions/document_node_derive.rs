use super::DocumentNodeDefinition;
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputMetadata, NodeTemplate, NodeTemplateImplementation, WidgetOverride};
use graphene_std::registry::*;
use graphene_std::*;
use std::collections::{HashMap, HashSet};

pub(super) fn post_process_nodes(custom: Vec<DocumentNodeDefinition>) -> HashMap<DefinitionIdentifier, DocumentNodeDefinition> {
	// Create hashmap for the protonodes added by the macro.
	let mut definitions_map = HashMap::new();
	// First remove the custom protonodes and add them to the definitions map since they contain different metadata
	// from the macro and must be inserted first so that network nodes which reference them use the correct metadata.
	let network_nodes = custom
		.into_iter()
		.filter_map(|definition| {
			if let NodeTemplateImplementation::ProtoNode(proto_node_identifier) = &definition.node_template.implementation {
				definitions_map.insert(DefinitionIdentifier::ProtoNode(proto_node_identifier.clone()), definition);
				return None;
			};
			Some(definition)
		})
		.collect::<Vec<_>>();

	// Add the rest of the protonodes from the macro.
	// Typed nodes are registered in `core_types::NODE_REGISTRY` via the macro's auto-generated `register_node` codegen.
	// `skip_impl` nodes (e.g. Cache, Monitor) bypass that registration but are wired up manually in
	// `interpreted_executor::node_registry::NODE_REGISTRY` via `async_node!`. We consult that extended registry as a
	// fallback when deriving `call_argument` so it reflects the impls actually registered, which will usually be `Context`.
	let extended_node_registry = &*interpreted_executor::node_registry::NODE_REGISTRY;
	let node_registry = NODE_REGISTRY.lock().unwrap();
	let empty_implementations: Vec<(NodeConstructor, NodeIOTypes)> = Vec::new();
	let context_type = concrete!(Context);
	for (id, metadata) in NODE_METADATA.lock().unwrap().iter() {
		let identifier = DefinitionIdentifier::ProtoNode(id.clone());
		if definitions_map.contains_key(&identifier) {
			continue;
		};
		let NodeMetadata {
			display_name,
			category,
			fields,
			description,
			properties,
			context_features,
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
		definitions_map.insert(
			identifier,
			DocumentNodeDefinition {
				identifier: display_name,
				node_template: NodeTemplate {
					inputs,
					call_argument: input_type.clone(),
					implementation: NodeTemplateImplementation::ProtoNode(id.clone()),
					context_features: ContextDependencies::from(context_features.as_slice()),
					// TODO: Store information for input overrides in the node macro
					input_metadata: fields
						.iter()
						.map(|f| match f.widget_override {
							RegistryWidgetOverride::None => (f.name, f.description).into(),
							RegistryWidgetOverride::Hidden => InputMetadata::with_name_description_override(f.name, f.description, WidgetOverride::Hidden),
							RegistryWidgetOverride::String(str) => InputMetadata::with_name_description_override(f.name, f.description, WidgetOverride::String(str.to_string())),
							RegistryWidgetOverride::Custom(str) => InputMetadata::with_name_description_override(f.name, f.description, WidgetOverride::Custom(str.to_string())),
						})
						.collect(),
					..Default::default()
				},
				category,
				description: Cow::Borrowed(description),
				properties: *properties,
			},
		);
	}

	// Add the rest of the network nodes to the map and add the metadata for their internal protonodes
	for mut network_node in network_nodes {
		fill_proto_node_metadata(&mut network_node.node_template, &definitions_map);

		// Set the reference to the node identifier
		if let NodeTemplateImplementation::Network(network_template) = &mut network_node.node_template.implementation {
			network_template.reference = Some(network_node.identifier.to_string());
			// If it is not a merge node, then set the display name to the identifier/reference
			if network_node.identifier != "Merge" {
				network_node.node_template.display_name = network_node.identifier.to_string();
			}
		}
		definitions_map.insert(DefinitionIdentifier::Network(network_node.identifier.to_string()), network_node);
	}

	definitions_map
}

/// Recursively fills each nested proto node's editor metadata from its definition, preserving only the authored position.
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

			let definition_template = &definition.node_template;
			node_template.display_name = definition_template.display_name.clone();
			node_template.input_metadata = definition_template.input_metadata.clone();
			node_template.output_names = definition_template.output_names.clone();
			node_template.locked = definition_template.locked;
			node_template.pinned = definition_template.pinned;
		}
		NodeTemplateImplementation::Extract => {}
	}
}
