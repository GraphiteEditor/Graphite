use core_types::ProtoNodeIdentifier;
use graph_craft::application_io::PlatformEditorApi;
use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork};
use graph_craft::generic;
use graphene_std::Context;
use graphene_std::ContextDependencies;
use graphene_std::registry::{NODE_METADATA, RegistryValueSource};
use graphene_std::uuid::NodeId;
use std::sync::Arc;

fn proto_node_template(identifier: ProtoNodeIdentifier, regular_inputs: Vec<NodeInput>) -> DocumentNode {
	let metadata_lock = NODE_METADATA.lock().unwrap();
	let metadata = metadata_lock
		.get(&identifier)
		.unwrap_or_else(|| panic!("Node `{}` not registered in NODE_METADATA", identifier.as_str()));

	let mut regular_iter = regular_inputs.into_iter();
	let inputs: Vec<NodeInput> = metadata
		.fields
		.iter()
		.map(|field| match &field.value_source {
			RegistryValueSource::Scope(name) => NodeInput::scope(*name),
			_ => regular_iter
				.next()
				.unwrap_or_else(|| panic!("Not enough non-scope inputs for `{}`", identifier.as_str())),
		})
		.collect();
	assert!(regular_iter.next().is_none(), "Too many non-scope inputs for `{}`", identifier.as_str());

	let context_features = ContextDependencies::from(metadata.context_features.as_slice());

	DocumentNode {
		inputs,
		implementation: DocumentNodeImplementation::ProtoNode(identifier),
		context_features,
		..Default::default()
	}
}

pub fn wrap_network_in_scope(network: NodeNetwork, editor_api: Arc<PlatformEditorApi>) -> NodeNetwork {
	let inner_network = DocumentNode {
		implementation: DocumentNodeImplementation::Network(network),
		inputs: vec![],
		..Default::default()
	};

	let output_node = DocumentNode {
		inputs: vec![NodeInput::node(NodeId(0), 0)],
		implementation: DocumentNodeImplementation::Network(NodeNetwork {
			exports: vec![NodeInput::node(NodeId(5), 0)],
			nodes: [
				proto_node_template(
					graphene_std::render_node::render_intermediate::IDENTIFIER,
					vec![NodeInput::import(core_types::Type::Fn(Box::new(concrete!(Context)), Box::new(generic!(T))), 0)],
				),
				proto_node_template(graphene_std::render_node::render::IDENTIFIER, vec![NodeInput::node(NodeId(0), 0)]),
				proto_node_template(graphene_std::render_cache::render_output_cache::IDENTIFIER, vec![NodeInput::node(NodeId(1), 0)]),
				proto_node_template(graphene_std::render_pixel_preview::render_pixel_preview::IDENTIFIER, vec![NodeInput::node(NodeId(2), 0)]),
				proto_node_template(graphene_std::render_background::render_background::IDENTIFIER, vec![NodeInput::node(NodeId(3), 0)]),
				DocumentNode {
					call_argument: concrete!(graphene_std::application_io::RenderConfig),
					..proto_node_template(graphene_std::render_node::create_context::IDENTIFIER, vec![NodeInput::node(NodeId(4), 0)])
				},
			]
			.into_iter()
			.enumerate()
			.map(|(id, node)| (NodeId(id as u64), node))
			.collect(),
			..Default::default()
		}),
		..Default::default()
	};

	// wrap the inner network in a scope
	let nodes = vec![
		inner_network,
		output_node,
		DocumentNode {
			implementation: DocumentNodeImplementation::ProtoNode(graphene_std::ops::passthrough::IDENTIFIER),
			inputs: vec![NodeInput::value(TaggedValue::EditorApi(editor_api), false)],
			..Default::default()
		},
	];
	let scope_injections = vec![("editor-api".to_string(), (NodeId(2), concrete!(&PlatformEditorApi)))];

	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(1), 0)],
		nodes: nodes.into_iter().enumerate().map(|(id, node)| (NodeId(id as u64), node)).collect(),
		scope_injections: scope_injections.into_iter().collect(),
		generated: true,
	}
}
