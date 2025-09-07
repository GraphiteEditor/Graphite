use std::sync::{Arc, mpsc::Sender};

use glam::UVec2;
use graph_craft::{document::NodeNetwork, graphene_compiler::Compiler};
use graphene_std::node_graph_overlay::{
	types::NodeGraphTransform,
	ui_context::{UIContextImpl, UIRuntimeResponse},
};

use crate::dynamic_executor::DynamicExecutor;

pub struct NodeGraphUIRuntime {
	pub executor: DynamicExecutor,
	pub compiler: Compiler,
	// Used within the node graph to return responses during evaluation
	// Also used to return compilation responses, but not for the UI overlay since the types are not needed
	pub response_sender: Sender<UIRuntimeResponse>,
}

impl NodeGraphUIRuntime {
	pub async fn compile(&mut self, compilation_request: CompilationRequest) {
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
