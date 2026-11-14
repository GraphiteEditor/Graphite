use graph_craft::application_io::PlatformEditorApi;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, DocumentNodeMetadata, NodeInput, NodeNetwork};
use graph_craft::generic;
use graph_craft::{concrete, item};
use graphene_std::Context;
use graphene_std::ContextFeatures;
use graphene_std::RuntimeHandle;
use graphene_std::uuid::NodeId;
use std::sync::Arc;

pub fn wrap_network_in_scope(mut network: NodeNetwork, editor_api: Arc<PlatformEditorApi>, runtime: RuntimeHandle) -> NodeNetwork {
	let definitions = std::mem::take(&mut network.definitions);

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
			exports: vec![NodeInput::node(NodeId(6), 0)],
			nodes: [
				DocumentNode {
					call_argument: concrete!(Context),
					inputs: vec![NodeInput::import(core_types::Type::Fn(Box::new(concrete!(Context)), Box::new(generic!(T))), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_std::render_node::render_intermediate::IDENTIFIER),
					compile_metadata: Some(graphene_std::NodeCompileMetadata {
						extract: ContextFeatures::VARARGS,
						inject: ContextFeatures::INDEX,
						..Default::default()
					}),
					..Default::default()
				},
				DocumentNode {
					call_argument: concrete!(Context),
					inputs: vec![
						NodeInput::scope(graphene_std::platform_application_io::try_wgpu_executor::IDENTIFIER),
						NodeInput::node(NodeId(0), 0),
						NodeInput::scope("runtime"),
						NodeInput::Reflection(DocumentNodeMetadata::SourceId),
					],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_std::render_node::download_gpu_rasters::IDENTIFIER),
					compile_metadata: Some(graphene_std::NodeCompileMetadata {
						extract: ContextFeatures::VARARGS,
						inject: ContextFeatures::empty(),
						..Default::default()
					}),
					..Default::default()
				},
				DocumentNode {
					call_argument: concrete!(Context),
					inputs: vec![NodeInput::scope(graphene_std::platform_application_io::wgpu_executor::IDENTIFIER), NodeInput::node(NodeId(1), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_std::render_node::render::IDENTIFIER),
					compile_metadata: Some(graphene_std::NodeCompileMetadata {
						extract: ContextFeatures::FOOTPRINT | ContextFeatures::VARARGS,
						inject: ContextFeatures::empty(),
						..Default::default()
					}),
					..Default::default()
				},
				DocumentNode {
					call_argument: concrete!(Context),
					inputs: vec![NodeInput::scope(graphene_std::platform_application_io::wgpu_executor::IDENTIFIER), NodeInput::node(NodeId(2), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_std::render_cache::render_output_cache::IDENTIFIER),
					compile_metadata: Some(graphene_std::NodeCompileMetadata {
						extract: ContextFeatures::FOOTPRINT | ContextFeatures::VARARGS,
						inject: ContextFeatures::VARARGS,
						..Default::default()
					}),
					..Default::default()
				},
				DocumentNode {
					call_argument: concrete!(Context),
					inputs: vec![NodeInput::scope(graphene_std::render_pixel_preview::pixel_preview_pipeline::IDENTIFIER), NodeInput::node(NodeId(3), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_std::render_pixel_preview::render_pixel_preview::IDENTIFIER),
					compile_metadata: Some(graphene_std::NodeCompileMetadata {
						extract: ContextFeatures::FOOTPRINT | ContextFeatures::VARARGS,
						inject: ContextFeatures::FOOTPRINT | ContextFeatures::VARARGS,
						..Default::default()
					}),
					..Default::default()
				},
				DocumentNode {
					call_argument: concrete!(Context),
					inputs: vec![
						NodeInput::scope(graphene_std::render_background::composite_background_pipeline::IDENTIFIER),
						NodeInput::node(NodeId(4), 0),
					],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_std::render_background::render_background::IDENTIFIER),
					compile_metadata: Some(graphene_std::NodeCompileMetadata {
						extract: ContextFeatures::FOOTPRINT | ContextFeatures::VARARGS,
						inject: ContextFeatures::empty(),
						..Default::default()
					}),
					..Default::default()
				},
				DocumentNode {
					call_argument: concrete!(Context),
					inputs: vec![NodeInput::node(NodeId(5), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_std::render_node::create_context::IDENTIFIER),
					compile_metadata: Some(graphene_std::NodeCompileMetadata {
						// INDEX is never read; it forces a nullification and cache pair onto the `data` parameter's wire so the render cache keys on a stable context
						extract: ContextFeatures::VARARGS | ContextFeatures::INDEX,
						inject: ContextFeatures::REAL_TIME | ContextFeatures::ANIMATION_TIME | ContextFeatures::POINTER_POSITION | ContextFeatures::FOOTPRINT | ContextFeatures::VARARGS,
						..Default::default()
					}),
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
			implementation: DocumentNodeImplementation::ProtoNode(graphene_std::ops::passthrough::IDENTIFIER),
			inputs: vec![NodeInput::value(TaggedValue::EditorApi(editor_api), false)],
			..Default::default()
		},
		DocumentNode {
			implementation: DocumentNodeImplementation::ProtoNode(graphene_std::ops::passthrough::IDENTIFIER),
			inputs: vec![NodeInput::value(TaggedValue::Runtime(runtime), false)],
			..Default::default()
		},
	];
	let scope_injections = vec![
		("editor-api".to_string(), (NodeId(2), item!(Arc<PlatformEditorApi>))),
		("runtime".to_string(), (NodeId(3), item!(RuntimeHandle))),
	];

	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(1), 0)],
		nodes: nodes.into_iter().enumerate().map(|(id, node)| (NodeId(id as u64), node)).collect(),
		scope_injections: scope_injections.into_iter().collect(),
		definitions,
		generated: true,
	}
}
