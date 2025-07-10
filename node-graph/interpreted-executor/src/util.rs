use graph_craft::ProtoNodeIdentifier;
use graph_craft::concrete;
use graph_craft::document::value::EditorMetadata;
use graph_craft::document::value::RenderOutput;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork};
use graph_craft::generic;
use graph_craft::wasm_application_io::WasmApplicationIo;
use graphene_std::Context;
use graphene_std::application_io::ApplicationIoValue;
use graphene_std::text::FontCache;
use graphene_std::uuid::NodeId;
use std::sync::Arc;

pub fn wrap_network_in_scope(network: NodeNetwork, font_cache: Arc<FontCache>, editor_metadata: EditorMetadata, application_io: Arc<WasmApplicationIo>) -> NodeNetwork {
	let inner_network = DocumentNode {
		implementation: DocumentNodeImplementation::Network(network),
		inputs: vec![],
		..Default::default()
	};

	let render_node = DocumentNode {
		inputs: vec![NodeInput::node(NodeId(0), 0)],
		implementation: DocumentNodeImplementation::Network(NodeNetwork {
			exports: vec![NodeInput::node(NodeId(2), 0)],
			nodes: [
				DocumentNode {
					inputs: vec![NodeInput::scope("application-io")],
					manual_composition: Some(concrete!(Context)),
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::CreateGpuSurfaceNode")),
					skip_deduplication: true,
					..Default::default()
				},
				DocumentNode {
					manual_composition: Some(concrete!(Context)),
					inputs: vec![NodeInput::node(NodeId(0), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_core::memo::memo::IDENTIFIER),
					..Default::default()
				},
				// TODO: Add conversion step
				DocumentNode {
					manual_composition: Some(concrete!(Context)),
					inputs: vec![
						NodeInput::scope("editor-metadata"),
						NodeInput::scope("application-io"),
						NodeInput::network(graphene_core::Type::Fn(Box::new(concrete!(Context)), Box::new(generic!(T))), 0),
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
	let nodes = vec![inner_network, render_node];

	NodeNetwork {
		// exports: vec![NodeInput::value(TaggedValue::RenderOutput(RenderOutput::default()), true)],
		exports: vec![NodeInput::node(NodeId(1), 0)],
		nodes: nodes.into_iter().enumerate().map(|(id, node)| (NodeId(id as u64), node)).collect(),
		scope_injections: [
			("font-cache".to_string(), TaggedValue::FontCache(font_cache)),
			("editor-metadata".to_string(), TaggedValue::EditorMetadata(editor_metadata)),
			("application-io".to_string(), TaggedValue::ApplicationIo(Arc::new(ApplicationIoValue(Some(application_io))))),
		]
		.into_iter()
		.collect(),
		// TODO(TrueDoctor): check if it makes sense to set `generated` to `true`
		generated: false,
	}
}
