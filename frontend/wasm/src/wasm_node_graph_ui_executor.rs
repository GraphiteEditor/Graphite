use std::sync::{Mutex, mpsc::Receiver};

use editor::{
	application::Editor,
	messages::prelude::{FrontendMessage, Message},
};
use graph_craft::graphene_compiler::Compiler;
use graphene_std::node_graph_overlay::{types::NodeGraphTransform, ui_context::UIRuntimeResponse};
use interpreted_executor::{
	dynamic_executor::DynamicExecutor,
	ui_runtime::{CompilationRequest, EvaluationRequest, NodeGraphUIRuntime},
};
use once_cell::sync::Lazy;

pub static NODE_UI_RUNTIME: Lazy<Mutex<Option<NodeGraphUIRuntime>>> = Lazy::new(|| Mutex::new(None));

// Since the runtime is not locked, it is possible to spawn multiple futures concurrently.
// This is why the runtime_busy flag exists
// This struct should never be locked in a future
pub struct WasmNodeGraphUIExecutor {
	response_receiver: Receiver<UIRuntimeResponse>,
	runtime_busy: bool,
	queued_compilation: Option<CompilationRequest>,
}

impl Default for WasmNodeGraphUIExecutor {
	fn default() -> Self {
		Self::new()
	}
}

impl WasmNodeGraphUIExecutor {
	pub fn new() -> Self {
		let (response_sender, response_receiver) = std::sync::mpsc::channel();
		let runtime = NodeGraphUIRuntime {
			executor: DynamicExecutor::default(),
			compiler: Compiler {},
			response_sender,
		};
		if let Ok(mut node_runtime) = NODE_UI_RUNTIME.lock() {
			node_runtime.replace(runtime);
		} else {
			log::error!("Could not lock runtime when creating new executor");
		};

		WasmNodeGraphUIExecutor {
			response_receiver,
			runtime_busy: false,
			queued_compilation: None,
		}
	}

	pub fn compilation_request(&mut self, compilation_request: CompilationRequest) {
		if !self.runtime_busy {
			self.runtime_busy = true;
			wasm_bindgen_futures::spawn_local(async move {
				let Ok(mut runtime) = NODE_UI_RUNTIME.try_lock() else {
					log::error!("Could not get runtime when evaluating");
					return;
				};
				let Some(runtime) = runtime.as_mut() else {
					log::error!("Could not lock runtime when evaluating");
					return;
				};
				runtime.compile(compilation_request).await;
			})
		} else {
			self.queued_compilation = Some(compilation_request);
		}
	}

	// Evaluates the node graph in a spawned future, and returns responses with the response_sender
	fn evaluation_request(&mut self, editor: &Editor) {
		if let Some(active_document) = editor.dispatcher.message_handlers.portfolio_message_handler.active_document() {
			let Some(network_metadata) = active_document.network_interface.network_metadata(&active_document.breadcrumb_network_path) else {
				return;
			};

			let transform = active_document.navigation_handler.calculate_offset_transform(
				editor.dispatcher.message_handlers.input_preprocessor_message_handler.viewport_bounds.center(),
				&network_metadata.persistent_metadata.navigation_metadata.node_graph_ptz,
			);

			let transform = NodeGraphTransform {
				scale: transform.matrix2.x_axis.x,
				x: transform.translation.x,
				y: transform.translation.y,
			};
			let resolution = editor.dispatcher.message_handlers.input_preprocessor_message_handler.viewport_bounds.size().as_uvec2();
			let evaluation_request = EvaluationRequest { transform, resolution };
			self.runtime_busy = true;

			wasm_bindgen_futures::spawn_local(async move {
				let Ok(mut runtime) = NODE_UI_RUNTIME.try_lock() else {
					log::error!("Could not get runtime when evaluating");
					return;
				};
				let Some(runtime) = runtime.as_mut() else {
					log::error!("Could not lock runtime when evaluating");
					return;
				};
				runtime.evaluate(evaluation_request).await
			})
		}
	}

	// This is run every time a frame is requested to be rendered.
	// It returns back Messages for how to update the frontend/editor click targets
	// It also checks for any queued evaluation/compilation requests and runs them
	pub fn poll_node_graph_ui_evaluation(&mut self, editor: &Editor) -> Vec<Message> {
		let mut responses = Vec::new();
		for runtime_response in self.response_receiver.try_iter() {
			match runtime_response {
				UIRuntimeResponse::RuntimeReady => {
					self.runtime_busy = false;
				}
				UIRuntimeResponse::OverlaySVG(svg_string) => {
					responses.push(FrontendMessage::UpdateNativeNodeGraphSVG { svg_string }.into());
				}
				UIRuntimeResponse::OverlayTexture(_texture) => todo!(),
			}
		}

		if !self.runtime_busy {
			if let Some(compilation_request) = self.queued_compilation.take() {
				self.compilation_request(compilation_request);
			} else {
				self.evaluation_request(editor);
			}
		}

		responses
	}
}
