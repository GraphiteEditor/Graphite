use super::*;
use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use glam::{DAffine2, DVec2};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeNetwork};
use graph_craft::graphene_compiler::Compiler;
use graph_craft::proto::GraphErrors;
use graph_craft::wasm_application_io::EditorPreferences;
use graph_craft::{ProtoNodeIdentifier, concrete};
use graphene_std::Context;
use graphene_std::application_io::{NodeGraphUpdateMessage, NodeGraphUpdateSender, RenderConfig};
use graphene_std::instances::Instance;
use graphene_std::memo::IORecord;
use graphene_std::renderer::{GraphicElementRendered, RenderParams, SvgRender};
use graphene_std::renderer::{RenderSvgSegmentList, SvgSegment};
use graphene_std::text::FontCache;
use graphene_std::uuid::{CompiledProtonodeInput, NodeId};
use graphene_std::vector::style::ViewMode;
use graphene_std::vector::{VectorData, VectorDataTable};
use graphene_std::wasm_application_io::{WasmApplicationIo, WasmEditorApi};
use interpreted_executor::dynamic_executor::{DynamicExecutor, IntrospectError, ResolvedDocumentNodeTypesDelta};
use interpreted_executor::util::wrap_network_in_scope;
use once_cell::sync::Lazy;
use spin::Mutex;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

/// Persistent data between graph evaluations. It's updated via message passing from the editor thread with [`GraphRuntimeRequest`]`.
/// [`PortfolioMessage::CompileActiveDocument`] and [`PortfolioMessage::RenderActiveDocument`] are the two main entry points
/// Some of these fields are inserted into the network at compile time using the scope system
/// Once the implementation is finished, this will live in a separate thread. Right now it's part of the main JS thread, but its own separate JS stack frame independent from the editor.
pub struct NodeRuntime {
	#[cfg(test)]
	pub(super) executor: DynamicExecutor,
	#[cfg(not(test))]
	executor: DynamicExecutor,
	receiver: Receiver<GraphRuntimeRequest>,
	sender: NodeGraphRuntimeSender,

	application_io: Option<Arc<WasmApplicationIo>>,

	node_graph_errors: GraphErrors,

	/// Which node is inspected and which monitor node is used (if any) for the current execution
	inspect_state: Option<InspectState>,

	/// Mapping of the fully-qualified node paths to their preprocessor substitutions.
	substitutions: HashMap<ProtoNodeIdentifier, DocumentNode>,

	/// Stored in order to check for changes before sending to the frontend.
	thumbnail_render_tagged_values: HashMap<CompiledProtonodeInput, TaggedValue>,
}

/// Messages passed from the editor thread to the node runtime thread.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum GraphRuntimeRequest {
	CompilationRequest(CompilationRequest),
	// Makes a request to evaluate the network and stores data for the list of output connectors
	// Should only monitor data for nodes which need their thumbnails.
	EvaluationRequest(EvaluationRequest),
	// Renders thumbnails for the data from the last execution
	// If the upstream node stores data for the context override, then another evaluation must be performed at the input
	// This is performed separately from execution requests, since thumbnails for animation should be updated once every 50ms or so.
	ThumbnailRenderRequest(HashSet<CompiledProtonodeInput>),
	// Request the data from a list of node inputs. For example, used by vector modify to get the data at the input of every Path node.
	// Can also be used by the spreadsheet/introspection system
	IntrospectionRequest(HashSet<(CompiledProtonodeInput, IntrospectMode)>),
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExportConfig {
	pub file_name: String,
	pub file_type: FileType,
	pub scale_factor: f64,
	pub bounds: ExportBounds,
	pub transparent_background: bool,
	pub size: DVec2,
}

#[derive(Clone)]
struct NodeGraphRuntimeSender(Sender<NodeGraphUpdate>);

impl NodeGraphRuntimeSender {
	fn send_compilation_response(&self, response: CompilationResponse) {
		self.0.send(NodeGraphUpdate::CompilationResponse(response)).expect("Failed to send compilation response")
	}
	fn send_evaluation_response(&self, response: EvaluationResponse) {
		self.0.send(NodeGraphUpdate::EvaluationResponse(response)).expect("Failed to send evaluation response")
	}
}

pub static NODE_RUNTIME: Lazy<Mutex<Option<NodeRuntime>>> = Lazy::new(|| Mutex::new(None));

impl NodeRuntime {
	pub fn new(receiver: Receiver<GraphRuntimeRequest>, sender: Sender<NodeGraphUpdate>) -> Self {
		Self {
			executor: DynamicExecutor::default(),
			receiver,
			sender: NodeGraphRuntimeSender(sender.clone()),

			application_io: None,

			node_graph_errors: Vec::new(),

			substitutions: preprocessor::generate_node_substitutions(),
			thumbnail_render_tagged_values: HashSet::new(),
			inspect_state: None,
		}
	}

	pub async fn run(&mut self) {
		if self.application_io.is_none() {
			#[cfg(not(test))]
			self.application_io = Some(Arc::new(WasmApplicationIo::new().await));
			#[cfg(test)]
			self.application_io = Some(Arc::new(WasmApplicationIo::new_offscreen().await));
		}

		// TODO: This deduplication of messages will probably cause more issues than it solved
		// let mut graph = None;
		// let mut execution = None;
		// let mut thumbnails = None;
		// let mut introspection = None;
		// for request in self.receiver.try_iter() {
		// 	match request {
		// 		GraphRuntimeRequest::CompilationRequest(_) => graph = Some(request),
		// 		GraphRuntimeRequest::EvaluationRequest(_) => execution = Some(request),
		// 		GraphRuntimeRequest::ThumbnailRenderResponse(_) => thumbnails = Some(request),
		// 		GraphRuntimeRequest::IntrospectionResponse(_) => introspection = Some(request),
		// 	}
		// }
		// let requests = [font, preferences, graph, execution].into_iter().flatten();

		for request in self.receiver.try_iter() {
			match request {
				GraphRuntimeRequest::CompilationRequest(CompilationRequest {
					mut network,
					font_cache,
					editor_metadata,
				}) => {
					// Insert the monitor node to manage the inspection
					// self.inspect_state = inspect_node.map(|inspect| InspectState::monitor_inspect_node(&mut network, inspect));

					self.node_graph_errors.clear();
					let result = self.update_network(network).await;
					self.sender.send_compilation_response(CompilationResponse {
						result,
						node_graph_errors: self.node_graph_errors.clone(),
					});
				}
				GraphRuntimeRequest::EvaluationRequest(EvaluationRequest {
					evaluation_id,
					context,
					inputs_to_monitor,
					// custom_node_to_evaluate
				}) => {
					for (protonode_input, introspect_mode) in inputs_to_monitor {
						self.executor.set_introspect(protonode_input, introspect_mode)
					}
					let transform = context.render_config.viewport.transform;

					let result = self.execute_network(render_config).await;

					let introspected_inputs = Vec::new();
					for (protonode_input, mode) in inputs_to_introspect {
						let Ok(introspected_data) = self.executor.introspect(protonode_input, mode) else {
							log::error!("Could not introspect node from input: {:?}", protonode_input);
							continue;
						};
						introspected_inputs.push((protonode_input, mode, introspected_data));
					}

					self.sender.send_evaluation_response(EvaluationResponse {
						evaluation_id,
						result,
						transform,
						introspected_inputs,
					});
				}
				GraphRuntimeRequest::ThumbnailRenderRequest(input_to_render) => {
					let mut thumbnail_response = ThumbnailRenderResponse::default();
					for input in input_to_render {}
					self.sender.send_thumbnail_render_response(thumbnail_response);
				}
				GraphRuntimeRequest::IntrospectionRequest(inputs_to_introspect) => {
					self.sender.send_introspection_response(introspection_response);
				}
			}
		}
	}

	async fn update_network(&mut self, mut graph: NodeNetwork) -> Result<CompilationMetadata, String> {
		preprocessor::expand_network(&mut graph, &self.substitutions);

		// Creates a network where the node paths to the document network are prefixed with NodeId(0)
		let scoped_network = wrap_network_in_scope(graph, self.editor_api.clone());

		// We assume only one output
		assert_eq!(scoped_network.exports.len(), 1, "Graph with multiple outputs not yet handled");

		// Modifies the NodeNetwork so the tagged values are removed and the document nodes with protonode implementations have their protonode ids set
		// Needs to return a mapping of absolute input connectors to protonode callers, types for protonodes, and callers for protonodes, add/remove delta for resolved types
		let (proto_network, protonode_callers_for_value, protonode_callers_for_node) = match scoped_network.flatten() {
			Ok(network) => network,
			Err(e) => {
				log::error!("Error compiling network: {e:?}");
				return;
			}
		};

		assert_ne!(proto_network.len(), 0, "No proto nodes exist?");
		let result = match self.executor.update(proto_network).await {
			Ok((types_to_add, types_to_remove)) => {
				// Used to remove thumbnails from the mapping of SNI to rendered SVG strings on the frontend, which occurs when the SNI is removed
				// When native frontend rendering is possible, the strings can just be stored in the network interface for each protonode with the rest of the type metadata
				Ok(CompilationMetadata {
					protonode_callers_for_value,
					protonode_callers_for_node,
					types_to_add,
					types_to_remove,
				})
			}
			Err(e) => {
				self.node_graph_errors.clone_from(&e);
				Err(format!("{e:?}"))
			}
		};
	}

	async fn execute_network(&mut self, render_config: RenderConfig) -> Result<TaggedValue, String> {
		use graph_craft::graphene_compiler::Executor;

		let result = match self.executor.input_type() {
			Some(t) if t == concrete!(RenderConfig) => (&self.executor).execute(render_config).await.map_err(|e| e.to_string()),
			Some(t) if t == concrete!(()) => (&self.executor).execute(()).await.map_err(|e| e.to_string()),
			Some(t) => Err(format!("Invalid input type {t:?}")),
			_ => Err(format!("No input type:\n{:?}", self.node_graph_errors)),
		};
		let result = match result {
			Ok(value) => value,
			Err(e) => return Err(e),
		};

		Ok(result)
	}
}

pub async fn run_node_graph() -> bool {
	let Some(mut runtime) = NODE_RUNTIME.try_lock() else { return false };
	if let Some(ref mut runtime) = runtime.as_mut() {
		runtime.run().await;
	}
	true
}

pub async fn replace_node_runtime(runtime: NodeRuntime) -> Option<NodeRuntime> {
	let mut node_runtime = NODE_RUNTIME.lock();
	node_runtime.replace(runtime)
}
