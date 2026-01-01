use graph_craft::ProtoNodeIdentifier;
use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork};
use graph_craft::generic;
use graph_craft::wasm_application_io::WasmEditorApi;
use graphene_std::Context;
use graphene_std::ContextFeatures;
use graphene_std::uuid::NodeId;
use std::sync::Arc;
use wgpu_executor::WgpuExecutor;

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

	let render_node = DocumentNode {
		inputs: vec![NodeInput::node(NodeId(0), 0)],
		implementation: DocumentNodeImplementation::Network(NodeNetwork {
			exports: vec![NodeInput::node(NodeId(2), 0)],
			nodes: [
				DocumentNode {
					call_argument: concrete!(Context),
					inputs: vec![NodeInput::import(core_types::Type::Fn(Box::new(concrete!(Context)), Box::new(generic!(T))), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_std::render_node::render_intermediate::IDENTIFIER),
					context_features: graphene_std::ContextDependencies {
						extract: ContextFeatures::VARARGS,
						inject: ContextFeatures::empty(),
					},
					..Default::default()
				},
				// Keep this in sync with the protonode in valid_input_types
				DocumentNode {
					call_argument: concrete!(Context),
					inputs: vec![NodeInput::scope("editor-api"), NodeInput::node(NodeId(0), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_std::render_node::render::IDENTIFIER),
					context_features: graphene_std::ContextDependencies {
						extract: ContextFeatures::FOOTPRINT | ContextFeatures::VARARGS,
						inject: ContextFeatures::empty(),
					},
					..Default::default()
				},
				DocumentNode {
					call_argument: concrete!(graphene_std::application_io::RenderConfig),
					inputs: vec![NodeInput::node(NodeId(1), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_std::render_node::create_context::IDENTIFIER),
					context_features: graphene_std::ContextDependencies {
						extract: ContextFeatures::empty(),
						inject: ContextFeatures::REAL_TIME | ContextFeatures::ANIMATION_TIME | ContextFeatures::POINTER | ContextFeatures::FOOTPRINT | ContextFeatures::VARARGS,
					},
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
	let mut nodes = vec![
		inner_network,
		render_node,
		DocumentNode {
			implementation: DocumentNodeImplementation::ProtoNode(graphene_std::ops::identity::IDENTIFIER),
			inputs: vec![NodeInput::value(TaggedValue::EditorApi(editor_api), false)],
			..Default::default()
		},
	];
	let mut scope_injections = vec![("editor-api".to_string(), (NodeId(2), concrete!(&WasmEditorApi)))];

	if cfg!(feature = "gpu") {
		nodes.push(DocumentNode {
			implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::from("graphene_core::ops::IntoNode<&WgpuExecutor>")),
			inputs: vec![NodeInput::node(NodeId(2), 0)],
			..Default::default()
		});
		scope_injections.push(("wgpu-executor".to_string(), (NodeId(3), concrete!(&WgpuExecutor))));
	}

	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(1), 0)],
		nodes: nodes.into_iter().enumerate().map(|(id, node)| (NodeId(id as u64), node)).collect(),
		scope_injections: scope_injections.into_iter().collect(),
		// TODO(TrueDoctor): check if it makes sense to set `generated` to `true`
		generated: false,
	}
}
