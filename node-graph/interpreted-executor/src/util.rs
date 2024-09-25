use std::sync::Arc;

use graph_craft::{
	concrete,
	document::{value::TaggedValue, DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork},
	generic,
	wasm_application_io::WasmEditorApi,
	ProtoNodeIdentifier,
};
use graphene_std::{transform::Footprint, uuid::NodeId};

// TODO: this is copy pasta from the editor (and does get out of sync)
pub fn wrap_network_in_scope(mut network: NodeNetwork, editor_api: Arc<WasmEditorApi>) -> NodeNetwork {
	network.generate_node_paths(&[]);

	let inner_network = DocumentNode {
		implementation: DocumentNodeImplementation::Network(network),
		inputs: vec![],
		..Default::default()
	};

	// TODO: Replace with "Output" definition?
	// let render_node = resolve_document_node_type("Output")
	// 	.expect("Output node type not found")
	// 	.node_template_input_override(vec![Some(NodeInput::node(NodeId(1), 0)), Some(NodeInput::node(NodeId(0), 1))])
	// 	.document_node;

	let render_node = graph_craft::document::DocumentNode {
		inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::node(NodeId(2), 0)],
		implementation: graph_craft::document::DocumentNodeImplementation::Network(NodeNetwork {
			exports: vec![NodeInput::node(NodeId(2), 0)],
			nodes: [
				DocumentNode {
					inputs: vec![NodeInput::scope("editor-api")],
					manual_composition: Some(concrete!(())),
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::CreateGpuSurfaceNode")),
					skip_deduplication: true,
					..Default::default()
				},
				DocumentNode {
					manual_composition: Some(concrete!(())),
					inputs: vec![NodeInput::node(NodeId(0), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode")),
					..Default::default()
				},
				// TODO: Add conversion step
				DocumentNode {
					manual_composition: Some(concrete!(graphene_std::application_io::RenderConfig)),
					inputs: vec![
						NodeInput::scope("editor-api"),
						NodeInput::network(graphene_core::Type::Fn(Box::new(concrete!(Footprint)), Box::new(generic!(T))), 0),
						NodeInput::node(NodeId(1), 0),
					],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::RenderNode")),
					..Default::default()
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
		render_node,
		DocumentNode {
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
			inputs: vec![NodeInput::value(TaggedValue::EditorApi(editor_api), false)],
			..Default::default()
		},
	];

	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(1), 0)],
		nodes: nodes.into_iter().enumerate().map(|(id, node)| (NodeId(id as u64), node)).collect(),
		scope_injections: [("editor-api".to_string(), (NodeId(2), concrete!(&WasmEditorApi)))].into_iter().collect(),
	}
}
