use super::*;
use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use glam::DVec2;
use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::NodeNetwork;
use graph_craft::proto::GraphErrors;
use graphene_std::text::FontCache;
use graphene_std::wasm_application_io::WasmApplicationIo;
use interpreted_executor::dynamic_executor::DynamicExecutor;
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

	/// Mapping of the fully-qualified node paths to their preprocessor substitutions.
	substitutions: HashMap<ProtoNodeIdentifier, DocumentNode>,
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
	// ThumbnailRenderRequest(HashSet<CompiledProtonodeInput>),
	// Request the data from a list of node inputs. For example, used by vector modify to get the data at the input of every Path node.
	// Can also be used by the spreadsheet/introspection system
	IntrospectionRequest(HashSet<SNI>),
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
	fn send_introspection_response(&self, response: IntrospectionResponse) {
		self.0.send(NodeGraphUpdate::IntrospectionResponse(response)).expect("Failed to send introspection response")
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
		}
	}

	pub async fn run(&mut self) {
		if self.application_io.is_none() {
			// #[cfg(not(test))]
			self.application_io = Some(Arc::new(WasmApplicationIo::new().await));
			// #[cfg(test)]
			// self.application_io = Some(Arc::new(WasmApplicationIo::new_offscreen().await));
		}

		// TODO: This deduplication of messages may cause issues
		let mut compilation = None;
		let mut evaluation = None;
		let mut introspection = None;
		for request in self.receiver.try_iter() {
			match request {
				GraphRuntimeRequest::CompilationRequest(_) => compilation = Some(request),
				GraphRuntimeRequest::EvaluationRequest(_) => evaluation = Some(request),
				GraphRuntimeRequest::IntrospectionRequest(_) => introspection = Some(request),
			}
		}
		let requests = [compilation, evaluation, introspection].into_iter().flatten();

		for request in requests {
			match request {
				GraphRuntimeRequest::CompilationRequest(CompilationRequest { network, font_cache, editor_metadata }) => {
					// Insert the monitor node to manage the inspection
					// self.inspect_state = inspect_node.map(|inspect| InspectState::monitor_inspect_node(&mut network, inspect));

					self.node_graph_errors.clear();
					let result = self.update_network(network, font_cache, editor_metadata).await;
					self.sender.send_compilation_response(CompilationResponse {
						result,
						node_graph_errors: self.node_graph_errors.clone(),
					});
				}
				// Inputs to monitor is sent from the editor, and represents a list of input connectors to track the data through
				// During the execution. If the value is None, then the node was not evaluated, which can occur due to caching
				GraphRuntimeRequest::EvaluationRequest(EvaluationRequest {
					evaluation_id,
					context,
					node_to_evaluate,
				}) => {
					// for (protonode_input, introspect_mode) in &inputs_to_monitor {
					// 	self.executor.set_introspect(*protonode_input, *introspect_mode)
					// }
					let result = self.executor.evaluate_from_node(context, node_to_evaluate).await;

					self.sender.send_evaluation_response(EvaluationResponse { evaluation_id, result });
				}
				// GraphRuntimeRequest::ThumbnailRenderRequest(_) => {}
				GraphRuntimeRequest::IntrospectionRequest(nodes) => {
					let mut introspected_nodes = Vec::new();
					for protonode in nodes {
						let introspected_data = match self.executor.introspect(protonode, true) {
							Ok(introspected_data) => introspected_data,
							Err(e) => {
								log::error!("Could not introspect protonode: {:?}, error: {:?}", protonode, e);
								continue;
							}
						};
						introspected_nodes.push((protonode, introspected_data));
					}

					self.sender.send_introspection_response(IntrospectionResponse(introspected_nodes));
				}
			}
		}
	}

	async fn update_network(&mut self, mut graph: NodeNetwork, font_cache: Arc<FontCache>, editor_metadata: EditorMetadata) -> Result<CompilationMetadata, String> {
		preprocessor::expand_network(&mut graph, &self.substitutions);

		// Creates a network where the node paths to the document network are prefixed with NodeId(0)
		let mut scoped_network = wrap_network_in_scope(graph, font_cache, editor_metadata, self.application_io.as_ref().unwrap().clone());

		// We assume only one output
		assert_eq!(scoped_network.exports.len(), 1, "Graph with multiple outputs not yet handled");

		// Modifies the NodeNetwork so the tagged values are removed and the document nodes with protonode implementations have their protonode ids set
		// Needs to return a mapping of absolute input connectors to protonode callers, types for protonodes, and callers for protonodes, add/remove delta for resolved types
		let (proto_network, original_locations) = match scoped_network.flatten() {
			Ok(result) => result,
			Err(e) => {
				log::error!("Error compiling network: {e:?}");
				return Err(e);
			}
		};

		let result = match self.executor.update(proto_network).await {
			Ok((types_to_add, types_to_remove)) => {
				// Used to remove thumbnails from the mapping of SNI to rendered SVG strings on the frontend, which occurs when the SNI is removed
				// When native frontend rendering is possible, the strings can just be stored in the network interface for each protonode with the rest of the type metadata
				Ok(CompilationMetadata {
					original_locations,
					types_to_add,
					types_to_remove,
				})
			}
			Err(e) => {
				self.node_graph_errors.clone_from(&e);
				Err(format!("{e:?}"))
			}
		};
		// log::debug!("result: {:?}", result);
		result
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
