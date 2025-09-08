use super::DocumentNodeDefinition;
use crate::messages::portfolio::document::utility_types::network_interface::{DocumentNodePersistentMetadata, InputMetadata, NodeTemplate, WidgetOverride};
use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::*;
use graphene_std::registry::*;
use graphene_std::*;
use std::collections::HashSet;

pub(super) fn post_process_nodes(mut custom: Vec<DocumentNodeDefinition>) -> Vec<DocumentNodeDefinition> {
	// Remove struct generics
	for DocumentNodeDefinition { node_template, .. } in custom.iter_mut() {
		let NodeTemplate {
			document_node: DocumentNode { implementation, .. },
			..
		} = node_template;

		if let DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier { name }) = implementation {
			if let Some((new_name, _suffix)) = name.rsplit_once("<") {
				*name = Cow::Owned(new_name.to_string())
			}
		};
	}

	let node_registry = NODE_REGISTRY.lock().unwrap();
	'outer: for (id, metadata) in NODE_METADATA.lock().unwrap().iter() {
		for node in custom.iter() {
			let DocumentNodeDefinition {
				node_template: NodeTemplate {
					document_node: DocumentNode { implementation, .. },
					..
				},
				..
			} = node;
			match implementation {
				DocumentNodeImplementation::ProtoNode(name) if name == id => continue 'outer,
				_ => (),
			}
		}

		let NodeMetadata {
			display_name,
			category,
			fields,
			description,
			properties,
			context_features,
		} = metadata;

		let Some(implementations) = &node_registry.get(id) else { continue };

		let valid_inputs: HashSet<_> = implementations.iter().map(|(_, node_io)| node_io.call_argument.clone()).collect();
		let first_node_io = implementations.first().map(|(_, node_io)| node_io).unwrap_or(const { &NodeIOTypes::empty() });

		let input_type = if valid_inputs.len() > 1 { &const { generic!(D) } } else { &first_node_io.call_argument };
		let output_type = &first_node_io.return_value;

		let inputs = preprocessor::node_inputs(fields, first_node_io);
		let node = DocumentNodeDefinition {
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
					output_names: vec![output_type.to_string()],
					locked: false,
					..Default::default()
				},
			},
			category: category.unwrap_or("UNCATEGORIZED"),
			description: Cow::Borrowed(description),
			properties: *properties,
		};

		custom.push(node);
	}

	custom
}
