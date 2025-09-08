use std::sync::{Arc, mpsc::Sender};

use glam::UVec2;
use graph_craft::{
	concrete,
	document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork, value::TaggedValue},
	graphene_compiler::Compiler,
};

use graphene_std::{
	node_graph_overlay::{
		types::NodeGraphTransform,
		ui_context::{UIContextImpl, UIRuntimeResponse},
	},
	text::NewFontCacheWrapper,
	uuid::NodeId,
};

use crate::dynamic_executor::DynamicExecutor;

pub struct NodeGraphUIRuntime {
	pub executor: DynamicExecutor,
	pub compiler: Compiler,
	// Used within the node graph to return responses during evaluation
	// Also used to return compilation responses, but not for the UI overlay since the types are not needed
	pub response_sender: Sender<UIRuntimeResponse>,
	pub font_cache: NewFontCacheWrapper,
}

impl NodeGraphUIRuntime {
	pub async fn compile(&mut self, mut compilation_request: CompilationRequest) {
		let font_cache_id = NodeId::new();
		let font_cache_node = DocumentNode {
			inputs: vec![NodeInput::value(TaggedValue::NewFontCache(self.font_cache.clone()), false)],
			implementation: DocumentNodeImplementation::ProtoNode(graphene_core::ops::identity::IDENTIFIER),
			..Default::default()
		};
		compilation_request.network.nodes.insert(font_cache_id, font_cache_node);
		compilation_request.network.scope_injections.insert("font-cache".to_string(), (font_cache_id, concrete!(())));
		match self.compiler.compile_single(compilation_request.network) {
			Ok(proto_network) => {
				if let Err(e) = self.executor.update(proto_network).await {
					log::error!("update error: {e:?}")
				}
			}
			Err(e) => {
				log::error!("Error compiling node graph ui network: {e:?}");
			}
		};
		let _ = self.response_sender.send(UIRuntimeResponse::RuntimeReady);
	}

	pub async fn evaluate(&mut self, evaluation_request: EvaluationRequest) {
		use graph_craft::graphene_compiler::Executor;

		let ui_context = Arc::new(UIContextImpl {
			transform: evaluation_request.transform,
			resolution: evaluation_request.resolution,
			response_sender: self.response_sender.clone(),
		});
		let _ = (&self.executor).execute(ui_context).await;
		let _ = self.response_sender.send(UIRuntimeResponse::RuntimeReady);
	}
}

/// Represents an update to the render state
/// TODO: Incremental compilation
#[derive(Debug)]
pub struct CompilationRequest {
	pub network: NodeNetwork,
}

// Requests an evaluation. The responses are added to the sender and can be processed when the next frame is requested
pub struct EvaluationRequest {
	pub transform: NodeGraphTransform,
	pub resolution: UVec2,
}
