use super::*;
use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use glam::{DAffine2, DVec2};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeNetwork};
use graph_craft::graphene_compiler::Compiler;
use graph_craft::proto::GraphErrors;
use graph_craft::wasm_application_io::EditorPreferences;
use graph_craft::{ProtoNodeIdentifier, concrete};
use graphene_std::application_io::{ApplicationIo, ExportFormat, ImageTexture, NodeGraphUpdateMessage, NodeGraphUpdateSender, RenderConfig};
use graphene_std::bounds::RenderBoundingBox;
use graphene_std::memo::IORecord;
use graphene_std::ops::Convert;
use graphene_std::raster_types::Raster;
use graphene_std::renderer::{Render, RenderParams, SvgRender};
use graphene_std::renderer::{RenderSvgSegmentList, SvgSegment};
use graphene_std::table::{Table, TableRow};
use graphene_std::text::FontCache;
use graphene_std::transform::RenderQuality;
use graphene_std::vector::Vector;
use graphene_std::wasm_application_io::{RenderOutputType, WasmApplicationIo, WasmEditorApi};
use graphene_std::{Artboard, Context, Graphic};
use interpreted_executor::dynamic_executor::{DynamicExecutor, IntrospectError, ResolvedDocumentNodeTypesDelta};
use interpreted_executor::util::wrap_network_in_scope;
use once_cell::sync::Lazy;
use spin::Mutex;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

/// Persistent data between graph executions. It's updated via message passing from the editor thread with [`GraphRuntimeRequest`]`.
/// Some of these fields are put into a [`WasmEditorApi`] which is passed to the final compiled graph network upon each execution.
/// Once the implementation is finished, this will live in a separate thread. Right now it's part of the main JS thread, but its own separate JS stack frame independent from the editor.
pub struct NodeRuntime {
	#[cfg(test)]
	pub(super) executor: DynamicExecutor,
	#[cfg(not(test))]
	executor: DynamicExecutor,
	receiver: Receiver<GraphRuntimeRequest>,
	sender: InternalNodeGraphUpdateSender,
	editor_preferences: EditorPreferences,
	old_graph: Option<NodeNetwork>,
	update_thumbnails: bool,

	editor_api: Arc<WasmEditorApi>,
	node_graph_errors: GraphErrors,
	monitor_nodes: Vec<Vec<NodeId>>,

	/// Which node is inspected and which monitor node is used (if any) for the current execution.
	inspect_state: Option<InspectState>,

	/// Mapping of the fully-qualified node paths to their preprocessor substitutions.
	substitutions: HashMap<ProtoNodeIdentifier, DocumentNode>,

	// TODO: Remove, it doesn't need to be persisted anymore
	/// The current renders of the thumbnails for layer nodes.
	thumbnail_renders: HashMap<NodeId, Vec<SvgSegment>>,
	vector_modify: HashMap<NodeId, Vector>,
}

/// Messages passed from the editor thread to the node runtime thread.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum GraphRuntimeRequest {
	GraphUpdate(GraphUpdate),
	ExecutionRequest(ExecutionRequest),
	FontCacheUpdate(FontCache),
	EditorPreferencesUpdate(EditorPreferences),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GraphUpdate {
	pub(super) network: NodeNetwork,
	/// The node that should be temporary inspected during execution
	pub(super) node_to_inspect: Option<NodeId>,
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExportConfig {
	pub name: String,
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
	pub fn new(receiver: Receiver<GraphRuntimeRequest>, sender: Sender<NodeGraphUpdate>) -> Self {
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

			substitutions: preprocessor::generate_node_substitutions(),

			thumbnail_renders: Default::default(),
			vector_modify: Default::default(),
			inspect_state: None,
		}
	}

	pub async fn run(&mut self) -> Option<ImageTexture> {
		if self.editor_api.application_io.is_none() {
			self.editor_api = WasmEditorApi {
				#[cfg(all(not(test), target_family = "wasm"))]
				application_io: Some(WasmApplicationIo::new().await.into()),
				#[cfg(any(test, not(target_family = "wasm")))]
				application_io: Some(WasmApplicationIo::new_offscreen().await.into()),
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
				GraphRuntimeRequest::GraphUpdate(_) => graph = Some(request),
				GraphRuntimeRequest::ExecutionRequest(ref execution_request) => {
					let for_export = execution_request.render_config.for_export;
					execution = Some(request);
					// If we get an export request we always execute it immedeatly otherwise it could get deduplicated
					if for_export {
						break;
					}
				}
				GraphRuntimeRequest::FontCacheUpdate(_) => font = Some(request),
				GraphRuntimeRequest::EditorPreferencesUpdate(_) => preferences = Some(request),
			}
		}
		let requests = [font, preferences, graph, execution].into_iter().flatten();

		for request in requests {
			match request {
				GraphRuntimeRequest::FontCacheUpdate(font_cache) => {
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
				GraphRuntimeRequest::EditorPreferencesUpdate(preferences) => {
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
				GraphRuntimeRequest::GraphUpdate(GraphUpdate { mut network, node_to_inspect }) => {
					// Insert the monitor node to manage the inspection
					self.inspect_state = node_to_inspect.map(|inspect| InspectState::monitor_inspect_node(&mut network, inspect));

					self.old_graph = Some(network.clone());

					self.node_graph_errors.clear();
					let result = self.update_network(network).await;
					let node_graph_errors = self.node_graph_errors.clone();

					self.update_thumbnails = true;

					self.sender.send_generation_response(CompilationResponse { result, node_graph_errors });
				}
				GraphRuntimeRequest::ExecutionRequest(ExecutionRequest { execution_id, mut render_config, .. }) => {
					// There are cases where we want to export via the svg pipeline eventhough raster was requested.
					if matches!(render_config.export_format, ExportFormat::Raster) {
						let vello_available = self.editor_api.application_io.as_ref().unwrap().gpu_executor().is_some();
						let use_vello = vello_available && self.editor_api.editor_preferences.use_vello();

						// On web when the user has disabled vello rendering in the preferences or we are exporting.
						// And on all platforms when vello is not supposed to be used.
						if !use_vello || cfg!(target_family = "wasm") && render_config.for_export {
							render_config.export_format = ExportFormat::Svg;
						}
					}

					let result = self.execute_network(render_config).await;
					let mut responses = VecDeque::new();
					// TODO: Only process monitor nodes if the graph has changed, not when only the Footprint changes
					self.process_monitor_nodes(&mut responses, self.update_thumbnails);
					self.update_thumbnails = false;

					// Resolve the result from the inspection by accessing the monitor node
					let inspect_result = self.inspect_state.and_then(|state| state.access(&self.executor));

					let (result, texture) = match result {
						Ok(TaggedValue::RenderOutput(RenderOutput {
							data: RenderOutputType::Texture(image_texture),
							metadata,
						})) if render_config.for_export => {
							let executor = self
								.editor_api
								.application_io
								.as_ref()
								.unwrap()
								.gpu_executor()
								.expect("GPU executor should be available when we receive a texture");

							let raster_cpu = Raster::new_gpu(image_texture.texture).convert(Footprint::BOUNDLESS, executor).await;

							let (data, width, height) = raster_cpu.to_flat_u8();

							(
								Ok(TaggedValue::RenderOutput(RenderOutput {
									data: RenderOutputType::Buffer { data, width, height },
									metadata,
								})),
								None,
							)
						}
						Ok(TaggedValue::RenderOutput(RenderOutput {
							data: RenderOutputType::Texture(texture),
							metadata,
						})) => (
							Ok(TaggedValue::RenderOutput(RenderOutput {
								data: RenderOutputType::Texture(texture.clone()),
								metadata,
							})),
							Some(texture),
						),
						r => (r, None),
					};

					self.sender.send_execution_response(ExecutionResponse {
						execution_id,
						result,
						responses,
						vector_modify: self.vector_modify.clone(),
						inspect_result,
					});
					return texture;
				}
			}
		}
		None
	}

	async fn update_network(&mut self, mut graph: NodeNetwork) -> Result<ResolvedDocumentNodeTypesDelta, String> {
		preprocessor::expand_network(&mut graph, &self.substitutions);

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

		match self.executor.input_type() {
			Some(t) if t == concrete!(RenderConfig) => (&self.executor).execute(render_config).await.map_err(|e| e.to_string()),
			Some(t) if t == concrete!(()) => (&self.executor).execute(()).await.map_err(|e| e.to_string()),
			Some(t) => Err(format!("Invalid input type {t:?}")),
			_ => Err(format!("No input type:\n{:?}", self.node_graph_errors)),
		}
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

			// Extract the monitor node's stored `Graphic` data
			let Ok(introspected_data) = self.executor.introspect(monitor_node_path) else {
				// TODO: Fix the root of the issue causing the spam of this warning (this at least temporarily disables it in release builds)
				#[cfg(debug_assertions)]
				warn!("Failed to introspect monitor node {}", self.executor.introspect(monitor_node_path).unwrap_err());
				continue;
			};

			// Graphic table: thumbnail
			if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, Table<Graphic>>>() {
				if update_thumbnails {
					Self::render_thumbnail(&mut self.thumbnail_renders, parent_network_node_id, &io.output, responses)
				}
			}
			// Artboard table: thumbnail
			else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, Table<Artboard>>>() {
				if update_thumbnails {
					Self::render_thumbnail(&mut self.thumbnail_renders, parent_network_node_id, &io.output, responses)
				}
			}
			// Vector table: vector modifications
			else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, Table<Vector>>>() {
				// Insert the vector modify
				let default = TableRow::default();
				self.vector_modify
					.insert(parent_network_node_id, io.output.iter().next().unwrap_or_else(|| default.as_ref()).element.clone());
			}
			// Other
			else {
				log::warn!("Failed to downcast monitor node output {parent_network_node_id:?}");
			}
		}
	}

	/// If this is `Graphic` data, regenerate click targets and thumbnails for the layers in the graph, modifying the state and updating the UI.
	fn render_thumbnail(thumbnail_renders: &mut HashMap<NodeId, Vec<SvgSegment>>, parent_network_node_id: NodeId, graphic: &impl Render, responses: &mut VecDeque<FrontendMessage>) {
		// Skip thumbnails if the layer is too complex (for performance)
		if graphic.render_complexity() > 1000 {
			let old = thumbnail_renders.insert(parent_network_node_id, Vec::new());
			if old.is_none_or(|v| !v.is_empty()) {
				responses.push_back(FrontendMessage::UpdateNodeThumbnail {
					id: parent_network_node_id,
					value: "<svg viewBox=\"0 0 10 10\"><title>Dense thumbnail omitted for performance</title><line x1=\"0\" y1=\"10\" x2=\"10\" y2=\"0\" stroke=\"red\" /></svg>".to_string(),
				});
			}
			return;
		}

		let bounds = match graphic.bounding_box(DAffine2::IDENTITY, true) {
			RenderBoundingBox::None => return,
			RenderBoundingBox::Infinite => [DVec2::ZERO, DVec2::new(300., 200.)],
			RenderBoundingBox::Rectangle(bounds) => bounds,
		};
		let footprint = Footprint {
			transform: DAffine2::from_translation(DVec2::new(bounds[0].x, bounds[0].y)),
			resolution: UVec2::new((bounds[1].x - bounds[0].x).abs() as u32, (bounds[1].y - bounds[0].y).abs() as u32),
			quality: RenderQuality::Full,
		};

		// Render the thumbnail from a `Graphic` into an SVG string
		let render_params = RenderParams {
			footprint,
			thumbnail: true,
			..Default::default()
		};
		let mut render = SvgRender::new();
		graphic.render_svg(&mut render, &render_params);

		// And give the SVG a viewbox and outer <svg>...</svg> wrapper tag
		render.format_svg(bounds[0], bounds[1]);

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

pub async fn run_node_graph() -> (bool, Option<ImageTexture>) {
	let Some(mut runtime) = NODE_RUNTIME.try_lock() else { return (false, None) };
	if let Some(ref mut runtime) = runtime.as_mut() {
		return (true, runtime.run().await);
	}
	(false, None)
}

pub async fn replace_node_runtime(runtime: NodeRuntime) -> Option<NodeRuntime> {
	let mut node_runtime = NODE_RUNTIME.lock();
	node_runtime.replace(runtime)
}
pub async fn replace_application_io(application_io: WasmApplicationIo) {
	let mut node_runtime = NODE_RUNTIME.lock();
	if let Some(node_runtime) = &mut *node_runtime {
		node_runtime.editor_api = WasmEditorApi {
			font_cache: node_runtime.editor_api.font_cache.clone(),
			application_io: Some(application_io.into()),
			node_graph_message_sender: Box::new(node_runtime.sender.clone()),
			editor_preferences: Box::new(node_runtime.editor_preferences.clone()),
		}
		.into();
	}
}

/// Which node is inspected and which monitor node is used (if any) for the current execution
#[derive(Debug, Clone, Copy)]
struct InspectState {
	inspect_node: NodeId,
	monitor_node: NodeId,
}
/// The resulting value from the temporary inspected during execution
#[derive(Clone, Debug, Default)]
pub struct InspectResult {
	introspected_data: Option<Arc<dyn std::any::Any + Send + Sync + 'static>>,
	pub inspect_node: NodeId,
}

impl InspectResult {
	pub fn take_data(&mut self) -> Option<Arc<dyn std::any::Any + Send + Sync + 'static>> {
		self.introspected_data.clone()
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
			implementation: DocumentNodeImplementation::ProtoNode(graphene_std::memo::monitor::IDENTIFIER),
			call_argument: graph_craft::generic!(T),
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

		Some(InspectResult {
			inspect_node: self.inspect_node,
			introspected_data,
		})
	}
}
