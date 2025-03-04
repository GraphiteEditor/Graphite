use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::prelude::*;

use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeNetwork};
use graph_craft::graphene_compiler::Compiler;
use graph_craft::proto::GraphErrors;
use graph_craft::wasm_application_io::EditorPreferences;
use graphene_core::application_io::{NodeGraphUpdateMessage, NodeGraphUpdateSender, RenderConfig};
use graphene_core::memo::IORecord;
use graphene_core::renderer::{GraphicElementRendered, RenderParams, SvgRender};
use graphene_core::renderer::{RenderSvgSegmentList, SvgSegment};
use graphene_core::text::FontCache;
use graphene_core::transform::Footprint;
use graphene_core::vector::style::ViewMode;
use graphene_std::vector::{VectorData, VectorDataTable};
use graphene_std::wasm_application_io::{WasmApplicationIo, WasmEditorApi};
use interpreted_executor::dynamic_executor::{DynamicExecutor, IntrospectError, ResolvedDocumentNodeTypesDelta};
use interpreted_executor::util::wrap_network_in_scope;

use super::*;
use glam::{DAffine2, DVec2, UVec2};
use once_cell::sync::Lazy;
use spin::Mutex;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

/// Persistent data between graph executions. It's updated via message passing from the editor thread with [`NodeRuntimeMessage`]`.
/// Some of these fields are put into a [`WasmEditorApi`] which is passed to the final compiled graph network upon each execution.
/// Once the implementation is finished, this will live in a separate thread. Right now it's part of the main JS thread, but its own separate JS stack frame independent from the editor.
pub struct NodeRuntime {
	#[cfg(test)]
	pub(super) executor: DynamicExecutor,
	#[cfg(not(test))]
	executor: DynamicExecutor,
	receiver: Receiver<NodeRuntimeMessage>,
	sender: InternalNodeGraphUpdateSender,
	editor_preferences: EditorPreferences,
	old_graph: Option<NodeNetwork>,
	update_thumbnails: bool,

	editor_api: Arc<WasmEditorApi>,
	node_graph_errors: GraphErrors,
	monitor_nodes: Vec<Vec<NodeId>>,

	/// Which node is inspected and which monitor node is used (if any) for the current execution
	inspect_state: Option<InspectState>,

	// TODO: Remove, it doesn't need to be persisted anymore
	/// The current renders of the thumbnails for layer nodes.
	thumbnail_renders: HashMap<NodeId, Vec<SvgSegment>>,
	vector_modify: HashMap<NodeId, VectorData>,
}

/// Messages passed from the editor thread to the node runtime thread.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum NodeRuntimeMessage {
	GraphUpdate(GraphUpdate),
	ExecutionRequest(ExecutionRequest),
	FontCacheUpdate(FontCache),
	EditorPreferencesUpdate(EditorPreferences),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GraphUpdate {
	pub(super) network: NodeNetwork,
	/// The node that should be temporary inspected during execution
	pub(super) inspect_node: Option<NodeId>,
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
struct InternalNodeGraphUpdateSender(Sender<NodeGraphUpdate>);

impl InternalNodeGraphUpdateSender {
	fn send_generation_response(&self, response: CompilationResponse) {
		self.0.send(NodeGraphUpdate::CompilationResponse(response)).expect("Failed to send response")
	}

	fn send_execution_response(&self, response: ExecutionResponse) {
		self.0.send(NodeGraphUpdate::ExecutionResponse(response)).expect("Failed to send response")
	}
}

impl NodeGraphUpdateSender for InternalNodeGraphUpdateSender {
	fn send(&self, message: NodeGraphUpdateMessage) {
		self.0.send(NodeGraphUpdate::NodeGraphUpdateMessage(message)).expect("Failed to send response")
	}
}

pub static NODE_RUNTIME: Lazy<Mutex<Option<NodeRuntime>>> = Lazy::new(|| Mutex::new(None));

impl NodeRuntime {
	pub fn new(receiver: Receiver<NodeRuntimeMessage>, sender: Sender<NodeGraphUpdate>) -> Self {
		Self {
			executor: DynamicExecutor::default(),
			receiver,
			sender: InternalNodeGraphUpdateSender(sender.clone()),
			editor_preferences: EditorPreferences::default(),
			old_graph: None,
			update_thumbnails: true,

			editor_api: WasmEditorApi {
				font_cache: FontCache::default(),
				editor_preferences: Box::new(EditorPreferences::default()),
				node_graph_message_sender: Box::new(InternalNodeGraphUpdateSender(sender)),

				application_io: None,
			}
			.into(),

			node_graph_errors: Vec::new(),
			monitor_nodes: Vec::new(),

			thumbnail_renders: Default::default(),
			vector_modify: Default::default(),
			inspect_state: None,
		}
	}

	pub async fn run(&mut self) {
		if self.editor_api.application_io.is_none() {
			self.editor_api = WasmEditorApi {
				application_io: Some(WasmApplicationIo::new().await.into()),
				font_cache: self.editor_api.font_cache.clone(),
				node_graph_message_sender: Box::new(self.sender.clone()),
				editor_preferences: Box::new(self.editor_preferences.clone()),
			}
			.into();
		}

		let mut font = None;
		let mut preferences = None;
		let mut graph = None;
		let mut execution = None;
		for request in self.receiver.try_iter() {
			match request {
				NodeRuntimeMessage::GraphUpdate(_) => graph = Some(request),
				NodeRuntimeMessage::ExecutionRequest(_) => execution = Some(request),
				NodeRuntimeMessage::FontCacheUpdate(_) => font = Some(request),
				NodeRuntimeMessage::EditorPreferencesUpdate(_) => preferences = Some(request),
			}
		}
		let requests = [font, preferences, graph, execution].into_iter().flatten();

		for request in requests {
			match request {
				NodeRuntimeMessage::FontCacheUpdate(font_cache) => {
					self.editor_api = WasmEditorApi {
						font_cache,
						application_io: self.editor_api.application_io.clone(),
						node_graph_message_sender: Box::new(self.sender.clone()),
						editor_preferences: Box::new(self.editor_preferences.clone()),
					}
					.into();
					if let Some(graph) = self.old_graph.clone() {
						// We ignore this result as compilation errors should have been reported in an earlier iteration
						let _ = self.update_network(graph).await;
					}
				}
				NodeRuntimeMessage::EditorPreferencesUpdate(preferences) => {
					self.editor_preferences = preferences.clone();
					self.editor_api = WasmEditorApi {
						font_cache: self.editor_api.font_cache.clone(),
						application_io: self.editor_api.application_io.clone(),
						node_graph_message_sender: Box::new(self.sender.clone()),
						editor_preferences: Box::new(preferences),
					}
					.into();
					if let Some(graph) = self.old_graph.clone() {
						// We ignore this result as compilation errors should have been reported in an earlier iteration
						let _ = self.update_network(graph).await;
					}
				}
				NodeRuntimeMessage::GraphUpdate(GraphUpdate { mut network, inspect_node }) => {
					// Insert the monitor node to manage the inspection
					self.inspect_state = inspect_node.map(|inspect| InspectState::monitor_inspect_node(&mut network, inspect));

					self.old_graph = Some(network.clone());
					self.node_graph_errors.clear();
					let result = self.update_network(network).await;
					self.update_thumbnails = true;
					self.sender.send_generation_response(CompilationResponse {
						result,
						node_graph_errors: self.node_graph_errors.clone(),
					});
				}
				NodeRuntimeMessage::ExecutionRequest(ExecutionRequest { execution_id, render_config, .. }) => {
					let transform = render_config.viewport.transform;

					let result = self.execute_network(render_config).await;
					let mut responses = VecDeque::new();
					// TODO: Only process monitor nodes if the graph has changed, not when only the Footprint changes
					self.process_monitor_nodes(&mut responses, self.update_thumbnails);
					self.update_thumbnails = false;

					// Resolve the result from the inspection by accessing the monitor node
					let inspect_result = self.inspect_state.and_then(|state| state.access(&self.executor));

					self.sender.send_execution_response(ExecutionResponse {
						execution_id,
						result,
						responses,
						transform,
						vector_modify: self.vector_modify.clone(),
						inspect_result,
					});
				}
			}
		}
	}

	async fn update_network(&mut self, graph: NodeNetwork) -> Result<ResolvedDocumentNodeTypesDelta, String> {
		let scoped_network = wrap_network_in_scope(graph, self.editor_api.clone());

		// We assume only one output
		assert_eq!(scoped_network.exports.len(), 1, "Graph with multiple outputs not yet handled");
		let c = Compiler {};
		let proto_network = match c.compile_single(scoped_network) {
			Ok(network) => network,
			Err(e) => return Err(e),
		};
		self.monitor_nodes = proto_network
			.nodes
			.iter()
			.filter(|(_, node)| node.identifier == "graphene_core::memo::MonitorNode".into())
			.map(|(_, node)| node.original_location.path.clone().unwrap_or_default())
			.collect::<Vec<_>>();

		assert_ne!(proto_network.nodes.len(), 0, "No proto nodes exist?");
		self.executor.update(proto_network).await.map_err(|e| {
			self.node_graph_errors.clone_from(&e);
			format!("{e:?}")
		})
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

	/// Updates state data
	pub fn process_monitor_nodes(&mut self, responses: &mut VecDeque<FrontendMessage>, update_thumbnails: bool) {
		// TODO: Consider optimizing this since it's currently O(m*n^2), with a sort it could be made O(m * n*log(n))
		self.thumbnail_renders.retain(|id, _| self.monitor_nodes.iter().any(|monitor_node_path| monitor_node_path.contains(id)));

		for monitor_node_path in &self.monitor_nodes {
			// Skip the inspect monitor node
			if self.inspect_state.is_some_and(|inspect_state| monitor_node_path.last().copied() == Some(inspect_state.monitor_node)) {
				continue;
			}
			// The monitor nodes are located within a document node, and are thus children in that network, so this gets the parent document node's ID
			let Some(parent_network_node_id) = monitor_node_path.len().checked_sub(2).and_then(|index| monitor_node_path.get(index)).copied() else {
				warn!("Monitor node has invalid node id");

				continue;
			};

			// Extract the monitor node's stored `GraphicElement` data.
			let Ok(introspected_data) = self.executor.introspect(monitor_node_path) else {
				// TODO: Fix the root of the issue causing the spam of this warning (this at least temporarily disables it in release builds)
				#[cfg(debug_assertions)]
				warn!("Failed to introspect monitor node {}", self.executor.introspect(monitor_node_path).unwrap_err());

				continue;
			};

			if let Some(io) = introspected_data.downcast_ref::<IORecord<Footprint, graphene_core::GraphicElement>>() {
				Self::process_graphic_element(&mut self.thumbnail_renders, parent_network_node_id, &io.output, responses, update_thumbnails)
			} else if let Some(io) = introspected_data.downcast_ref::<IORecord<(), graphene_core::GraphicElement>>() {
				Self::process_graphic_element(&mut self.thumbnail_renders, parent_network_node_id, &io.output, responses, update_thumbnails)
			} else if let Some(io) = introspected_data.downcast_ref::<IORecord<Footprint, graphene_core::Artboard>>() {
				Self::process_graphic_element(&mut self.thumbnail_renders, parent_network_node_id, &io.output, responses, update_thumbnails)
			} else if let Some(io) = introspected_data.downcast_ref::<IORecord<(), graphene_core::Artboard>>() {
				Self::process_graphic_element(&mut self.thumbnail_renders, parent_network_node_id, &io.output, responses, update_thumbnails)
			}
			// Insert the vector modify if we are dealing with vector data
			else if let Some(record) = introspected_data.downcast_ref::<IORecord<Footprint, VectorDataTable>>() {
				self.vector_modify.insert(parent_network_node_id, record.output.one_instance().instance.clone());
			} else if let Some(record) = introspected_data.downcast_ref::<IORecord<(), VectorDataTable>>() {
				self.vector_modify.insert(parent_network_node_id, record.output.one_instance().instance.clone());
			}
		}
	}

	// If this is `GraphicElement` data:
	// Regenerate click targets and thumbnails for the layers in the graph, modifying the state and updating the UI.
	fn process_graphic_element(
		thumbnail_renders: &mut HashMap<NodeId, Vec<SvgSegment>>,
		parent_network_node_id: NodeId,
		graphic_element: &impl GraphicElementRendered,
		responses: &mut VecDeque<FrontendMessage>,
		update_thumbnails: bool,
	) {
		// RENDER THUMBNAIL

		if !update_thumbnails {
			return;
		}

		let bounds = graphic_element.bounding_box(DAffine2::IDENTITY);

		// Render the thumbnail from a `GraphicElement` into an SVG string
		let render_params = RenderParams::new(ViewMode::Normal, bounds, true, false, false);
		let mut render = SvgRender::new();
		graphic_element.render_svg(&mut render, &render_params);

		// And give the SVG a viewbox and outer <svg>...</svg> wrapper tag
		let [min, max] = bounds.unwrap_or_default();
		render.format_svg(min, max);

		// UPDATE FRONTEND THUMBNAIL

		let new_thumbnail_svg = render.svg;
		let old_thumbnail_svg = thumbnail_renders.entry(parent_network_node_id).or_default();

		if old_thumbnail_svg != &new_thumbnail_svg {
			responses.push_back(FrontendMessage::UpdateNodeThumbnail {
				id: parent_network_node_id,
				value: new_thumbnail_svg.to_svg_string(),
			});
			*old_thumbnail_svg = new_thumbnail_svg;
		}
	}
}

pub async fn introspect_node(path: &[NodeId]) -> Result<Arc<dyn std::any::Any + Send + Sync + 'static>, IntrospectError> {
	let runtime = NODE_RUNTIME.lock();
	if let Some(ref mut runtime) = runtime.as_ref() {
		return runtime.executor.introspect(path);
	}
	Err(IntrospectError::RuntimeNotReady)
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

/// Which node is inspected and which monitor node is used (if any) for the current execution
#[derive(Debug, Clone, Copy)]
struct InspectState {
	inspect_node: NodeId,
	monitor_node: NodeId,
}
/// The resulting value from the temporary inspected during execution
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "decouple-execution", derive(serde::Serialize, serde::Deserialize))]
pub struct InspectResult {
	#[cfg(not(feature = "decouple-execution"))]
	introspected_data: Option<Arc<dyn std::any::Any + Send + Sync + 'static>>,
	#[cfg(feature = "decouple-execution")]
	introspected_data: Option<TaggedValue>,
	pub inspect_node: NodeId,
}

impl InspectResult {
	pub fn take_data(&mut self) -> Option<Arc<dyn std::any::Any + Send + Sync + 'static>> {
		#[cfg(not(feature = "decouple-execution"))]
		return self.introspected_data.clone();

		#[cfg(feature = "decouple-execution")]
		return self.introspected_data.take().map(|value| value.to_any());
	}
}

// This is very ugly but is required to be inside a message
impl PartialEq for InspectResult {
	fn eq(&self, other: &Self) -> bool {
		self.inspect_node == other.inspect_node
	}
}

impl InspectState {
	/// Insert the monitor node to manage the inspection
	pub fn monitor_inspect_node(network: &mut NodeNetwork, inspect_node: NodeId) -> Self {
		let monitor_id = NodeId::new();

		// It is necessary to replace the inputs before inserting the monitor node to avoid changing the input of the new monitor node
		for input in network.nodes.values_mut().flat_map(|node| node.inputs.iter_mut()).chain(&mut network.exports) {
			let NodeInput::Node { node_id, output_index, .. } = input else { continue };
			// We only care about the primary output of our inspect node
			if *output_index != 0 || *node_id != inspect_node {
				continue;
			}

			*node_id = monitor_id;
		}

		let monitor_node = DocumentNode {
			inputs: vec![NodeInput::node(inspect_node, 0)], // Connect to the primary output of the inspect node
			implementation: DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode"),
			manual_composition: Some(graph_craft::generic!(T)),
			skip_deduplication: true,
			..Default::default()
		};
		network.nodes.insert(monitor_id, monitor_node);

		Self {
			inspect_node,
			monitor_node: monitor_id,
		}
	}
	/// Resolve the result from the inspection by accessing the monitor node
	fn access(&self, executor: &DynamicExecutor) -> Option<InspectResult> {
		let introspected_data = executor.introspect(&[self.monitor_node]).inspect_err(|e| warn!("Failed to introspect monitor node {e}")).ok();
		// TODO: Consider displaying the error instead of ignoring it
		#[cfg(feature = "decouple-execution")]
		let introspected_data = introspected_data.as_ref().and_then(|data| TaggedValue::try_from_std_any_ref(data).ok());

		Some(InspectResult {
			inspect_node: self.inspect_node,
			introspected_data,
		})
	}
}
