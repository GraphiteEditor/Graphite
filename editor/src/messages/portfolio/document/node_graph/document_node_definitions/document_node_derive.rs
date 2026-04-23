use super::DocumentNodeDefinition;
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{
	DocumentNodeMetadata, DocumentNodePersistentMetadata, InputMetadata, NodeNetworkMetadata, NodeNetworkPersistentMetadata, NodeTemplate, NodeTypePersistentMetadata, WidgetOverride,
};
use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::*;
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
			if let DocumentNodeImplementation::ProtoNode(proto_node_identifier) = &definition.node_template.document_node.implementation {
				definitions_map.insert(DefinitionIdentifier::ProtoNode(proto_node_identifier.clone()), definition);
				return None;
			};
			Some(definition)
		})
		.collect::<Vec<_>>();

	// Add the rest of the protonodes from the macro
	let node_registry = NODE_REGISTRY.lock().unwrap();
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
			output_fields,
		} = metadata;

		let Some(implementations) = &node_registry.get(id) else { continue };

		let first_node_io = implementations.first().map(|(_, node_io)| node_io).unwrap_or(const { &NodeIOTypes::empty() });

		let valid_inputs: HashSet<_> = implementations.iter().map(|(_, node_io)| node_io.call_argument.clone()).collect();
		let input_type = if valid_inputs.len() > 1 { &const { generic!(D) } } else { &first_node_io.call_argument };

		let inputs = preprocessor::node_inputs(fields, first_node_io);
		definitions_map.insert(
			identifier,
			DocumentNodeDefinition {
				identifier: display_name,
				node_template: NodeTemplate {
					document_node: DocumentNode {
						inputs,
						call_argument: input_type.clone(),
						implementation: DocumentNodeImplementation::ProtoNode(id.clone()),
						visible: true,
						skip_deduplication: false,
						context_features: ContextDependencies::from(context_features.as_slice()),
						..Default::default()
					},
					persistent_node_metadata: DocumentNodePersistentMetadata {
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
						output_names: if output_fields.is_empty() {
							Vec::new()
						} else {
							// A leading field with empty node_path is a hidden primary output placeholder, included as ""
							output_fields.iter().map(|field| field.name.to_string()).collect()
						},
						locked: false,
						..Default::default()
					},
				},
				category,
				description: Cow::Borrowed(description),
				properties: *properties,
			},
		);
	}
	drop(node_registry);

	// For nodes with destructured outputs, build a Network implementation so the UI knows about
	// the multiple output connectors. The network contains the source proto node plus one extractor
	// proto node per output field, wired directly (no identity pass-through or memo nodes).
	let destructured_info = {
		let node_metadata_registry = NODE_METADATA.lock().unwrap();
		let node_registry = NODE_REGISTRY.lock().unwrap();
		definitions_map
			.iter()
			.filter_map(|(_, def)| {
				let DocumentNodeImplementation::ProtoNode(id) = &def.node_template.document_node.implementation else {
					return None;
				};
				let meta = node_metadata_registry.get(id)?;
				if meta.output_fields.is_empty() {
					return None;
				}

				let has_hidden_primary = meta.output_fields.first().is_some_and(|f| f.node_path.is_empty());
				let real_fields = if has_hidden_primary { &meta.output_fields[1..] } else { meta.output_fields };

				let extractors: Vec<_> = real_fields
					.iter()
					.filter_map(|field| {
						let extractor_id = ProtoNodeIdentifier::with_owned_string(field.node_path.to_string());
						let call_arg = node_registry.get(&extractor_id).and_then(|impls| impls.first().map(|(_, node_io)| node_io.call_argument.clone()))?;
						Some((extractor_id, call_arg))
					})
					.collect();

				if extractors.is_empty() {
					return None;
				}

				Some((id.clone(), has_hidden_primary, extractors))
			})
			.collect::<Vec<_>>()
	};

	// Build the definition-time network for each destructured node
	for (id, has_hidden_primary, extractors) in &destructured_info {
		let identifier = DefinitionIdentifier::ProtoNode(id.clone());
		let Some(definition) = definitions_map.get_mut(&identifier) else { continue };

		// The source node runs the original proto node (e.g. split_vec2)
		let source_node_id = NodeId(0);
		let source_node = DocumentNode {
			inputs: definition
				.node_template
				.document_node
				.inputs
				.iter()
				.enumerate()
				.map(|(i, input)| NodeInput::import(input.ty(), i))
				.collect(),
			implementation: DocumentNodeImplementation::ProtoNode(id.clone()),
			call_argument: definition.node_template.document_node.call_argument.clone(),
			..Default::default()
		};

		// One extractor node per output field, each taking input from the source node
		let mut inner_nodes: Vec<(NodeId, DocumentNode)> = vec![(source_node_id, source_node)];

		let mut exports = if *has_hidden_primary { vec![NodeInput::value(TaggedValue::None, false)] } else { Vec::new() };

		for (i, (extractor_id, call_arg)) in extractors.iter().enumerate() {
			let extractor_node_id = NodeId((i + 1) as u64);
			inner_nodes.push((
				extractor_node_id,
				DocumentNode {
					inputs: vec![NodeInput::node(source_node_id, 0)],
					implementation: DocumentNodeImplementation::ProtoNode(extractor_id.clone()),
					call_argument: call_arg.clone(),
					..Default::default()
				},
			));
			exports.push(NodeInput::node(extractor_node_id, 0));
		}

		// Generate network metadata for each inner node
		let node_metadata: HashMap<_, _> = inner_nodes
			.iter()
			.map(|(nid, _)| {
				(
					*nid,
					DocumentNodeMetadata {
						persistent_metadata: DocumentNodePersistentMetadata {
							node_type_metadata: NodeTypePersistentMetadata::node(glam::IVec2::ZERO),
							..Default::default()
						},
						..Default::default()
					},
				)
			})
			.collect();

		definition.node_template.persistent_node_metadata.network_metadata = Some(NodeNetworkMetadata {
			persistent_metadata: NodeNetworkPersistentMetadata {
				reference: Some(definition.identifier.to_string()),
				node_metadata,
				..Default::default()
			},
			..Default::default()
		});
		definition.node_template.persistent_node_metadata.display_name = definition.identifier.to_string();

		definition.node_template.document_node.implementation = DocumentNodeImplementation::Network(NodeNetwork {
			exports,
			nodes: inner_nodes.into_iter().collect(),
			scope_injections: Default::default(),
			generated: false,
		});
	}

	// If any protonode does not have metadata then set its display name to its identifier string
	for definition in definitions_map.values_mut() {
		let metadata = NODE_METADATA.lock().unwrap();
		if let DocumentNodeImplementation::ProtoNode(id) = &definition.node_template.document_node.implementation
			&& !metadata.contains_key(id)
		{
			definition.node_template.persistent_node_metadata.display_name = definition.identifier.to_string();
		}
	}

	// Fill in inner proto node metadata for the destructured output network definitions
	for (id, _, _) in destructured_info {
		let identifier = DefinitionIdentifier::ProtoNode(id);
		// We need to split the borrow: extract the node and metadata, traverse, then put metadata back
		let Some(definition) = definitions_map.get(&identifier) else { continue };
		let document_node = definition.node_template.document_node.clone();
		let mut persistent_metadata = definition.node_template.persistent_node_metadata.clone();
		traverse_node(&document_node, &mut persistent_metadata, &definitions_map);
		definitions_map.get_mut(&identifier).unwrap().node_template.persistent_node_metadata = persistent_metadata;
	}

	// Add the rest of the network nodes to the map and add the metadata for their internal protonodes
	for mut network_node in network_nodes {
		traverse_node(&network_node.node_template.document_node, &mut network_node.node_template.persistent_node_metadata, &definitions_map);
		// Set the reference to the node identifier
		if let Some(nested_metadata) = network_node.node_template.persistent_node_metadata.network_metadata.as_mut() {
			nested_metadata.persistent_metadata.reference = Some(network_node.identifier.to_string());
			// If it is not a merge node, then set the display name to the identifier/reference
			if network_node.identifier != "Merge" {
				network_node.node_template.persistent_node_metadata.display_name = network_node.identifier.to_string();
			}
		}
		definitions_map.insert(DefinitionIdentifier::Network(network_node.identifier.to_string()), network_node);
	}

	definitions_map
}

/// Traverses a document node template and metadata in parallel to add metadata to the protonodes
fn traverse_node(node: &DocumentNode, node_metadata: &mut DocumentNodePersistentMetadata, definitions_map: &HashMap<DefinitionIdentifier, DocumentNodeDefinition>) {
	match &node.implementation {
		DocumentNodeImplementation::Network(node_network) => {
			for (nested_node_id, nested_node) in node_network.nodes.iter() {
				let nested_metadata = node_metadata.network_metadata.as_mut().unwrap().persistent_metadata.node_metadata.get_mut(nested_node_id).unwrap();
				traverse_node(nested_node, &mut nested_metadata.persistent_metadata, definitions_map);
			}
		}
		DocumentNodeImplementation::ProtoNode(id) => {
			// Set all the metadata except the position to the proto node information from the macro
			// TODO: Use options in the template to specify what you want to default and what you want to override
			// If this fails then the proto node id in the definition doesn't match what is generated by the macro
			let Some(definition) = definitions_map.get(&DefinitionIdentifier::ProtoNode(id.clone())) else {
				// log::error!("Could not get definition for id {} when filling in protonode metadata for a custom node", id.clone());
				return;
			};
			let mut new_metadata = definition.node_template.persistent_node_metadata.clone();
			new_metadata.node_type_metadata = node_metadata.node_type_metadata.clone();
			*node_metadata = new_metadata
		}
		DocumentNodeImplementation::Extract => {}
	}
}
