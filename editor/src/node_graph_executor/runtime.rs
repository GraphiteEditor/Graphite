use super::*;
use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use glam::{DAffine2, DVec2, UVec2};
use graph_craft::application_io::{PlatformApplicationIo, PlatformEditorApi};
use graph_craft::document::value::{RenderOutput, RenderOutputType, TaggedValue};
use graph_craft::document::{NodeId, NodeNetwork};
use graph_craft::graphene_compiler::Compiler;
use graph_craft::proto::GraphErrors;
use graph_craft::{ProtoNodeIdentifier, concrete};
use graphene_std::application_io::{ApplicationIo, ExportFormat, ImageTexture, NodeGraphUpdateMessage, NodeGraphUpdateSender, RenderConfig};
use graphene_std::bounds::{BoundingBox, RenderBoundingBox};
use graphene_std::memo::IORecord;
use graphene_std::ops::Convert;
#[cfg(all(target_family = "wasm", feature = "gpu", feature = "wasm"))]
use graphene_std::platform_application_io::canvas_utils::{Canvas, CanvasSurface, CanvasSurfaceHandle};
use graphene_std::raster_types::Raster;
use graphene_std::renderer::{Render, RenderParams, RenderSvgSegmentList, SvgRender, SvgSegment};
use graphene_std::table::Table;
use graphene_std::text::FontCache;
use graphene_std::transform::RenderQuality;
use graphene_std::vector::Vector;
use graphene_std::vector::style::RenderMode;
use graphene_std::{Artboard, Context, Graphic};
use interpreted_executor::dynamic_executor::{DynamicExecutor, IntrospectError, ResolvedDocumentNodeTypesDelta};
use interpreted_executor::util::wrap_network_in_scope;
use spin::Mutex;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

/// Persistent data between graph executions. It's updated via message passing from the editor thread with [`GraphRuntimeRequest`]`.
/// Some of these fields are put into a [`PlatformEditorApi`] which is passed to the final compiled graph network upon each execution.
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

	editor_api: Arc<PlatformEditorApi>,
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

	/// Cached surface for Wasm viewport rendering (reused across frames)
	#[cfg(all(target_family = "wasm", feature = "gpu", feature = "wasm"))]
	wasm_canvas_cache: CanvasSurfaceHandle,
	/// Currently displayed texture, the runtime keeps a reference to it to avoid the texture getting destroyed while it is still in use.
	#[cfg(all(target_family = "wasm", feature = "gpu", feature = "wasm"))]
	current_viewport_texture: Option<ImageTexture>,
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
	/// Full path from the root network to the node that should be temporarily inspected during execution.
	/// The last element is the inspect target; preceding elements identify the nested subnetwork it lives in,
	/// so the runtime can splice its monitor node alongside the target instead of only at the top level.
	pub(super) node_to_inspect: Vec<NodeId>,
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExportConfig {
	pub name: String,
	pub file_type: FileType,
	pub scale_factor: f64,
	pub bounds: ExportBounds,
	pub size: UVec2,
	pub artboard_name: Option<String>,
	pub artboard_count: usize,
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

	fn send_eyedropper_preview(&self, raster: Raster<CPU>) {
		self.0.send(NodeGraphUpdate::EyedropperPreview(raster)).expect("Failed to send response")
	}
}

impl NodeGraphUpdateSender for InternalNodeGraphUpdateSender {
	fn send(&self, message: NodeGraphUpdateMessage) {
		self.0.send(NodeGraphUpdate::NodeGraphUpdateMessage(message)).expect("Failed to send response")
	}
}

// TODO: Replace with `core::cell::LazyCell` (<https://doc.rust-lang.org/core/cell/struct.LazyCell.html>) or similar
pub static NODE_RUNTIME: once_cell::sync::Lazy<Mutex<Option<NodeRuntime>>> = once_cell::sync::Lazy::new(|| Mutex::new(None));

impl NodeRuntime {
	pub fn new(receiver: Receiver<GraphRuntimeRequest>, sender: Sender<NodeGraphUpdate>) -> Self {
		Self {
			executor: DynamicExecutor::default(),
			receiver,
			sender: InternalNodeGraphUpdateSender(sender.clone()),
			editor_preferences: EditorPreferences::default(),
			old_graph: None,
			update_thumbnails: true,

			editor_api: PlatformEditorApi {
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
			#[cfg(all(target_family = "wasm", feature = "gpu"))]
			wasm_canvas_cache: CanvasSurfaceHandle::new(),
			#[cfg(all(target_family = "wasm", feature = "gpu"))]
			current_viewport_texture: None,
		}
	}

	pub async fn run(&mut self) -> Option<ImageTexture> {
		if self.editor_api.application_io.is_none() {
			self.editor_api = PlatformEditorApi {
				#[cfg(all(not(test), target_family = "wasm"))]
				application_io: Some(PlatformApplicationIo::new().await.into()),
				#[cfg(any(test, not(target_family = "wasm")))]
				application_io: Some(PlatformApplicationIo::new().await.into()),
				font_cache: self.editor_api.font_cache.clone(),
				node_graph_message_sender: Box::new(self.sender.clone()),
				editor_preferences: Box::new(self.editor_preferences.clone()),
			}
			.into();
		}

		let mut font = None;
		let mut preferences = None;
		let mut graph = None;
		let mut eyedropper = None;
		let mut execution = None;
		for request in self.receiver.try_iter() {
			match request {
				GraphRuntimeRequest::GraphUpdate(_) => graph = Some(request),
				GraphRuntimeRequest::ExecutionRequest(ref execution_request) => {
					if execution_request.render_config.for_eyedropper {
						eyedropper = Some(request);

						continue;
					}

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

		// Eydropper should use the same time and pointer to not invalidate the cache
		if let Some(GraphRuntimeRequest::ExecutionRequest(eyedropper)) = &mut eyedropper
			&& let Some(GraphRuntimeRequest::ExecutionRequest(execution)) = &execution
		{
			eyedropper.render_config.time = execution.render_config.time;
			eyedropper.render_config.pointer = execution.render_config.pointer;
		}

		let requests = [font, preferences, graph, eyedropper, execution].into_iter().flatten();

		for request in requests {
			match request {
				GraphRuntimeRequest::FontCacheUpdate(font_cache) => {
					self.editor_api = PlatformEditorApi {
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
					self.editor_api = PlatformEditorApi {
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
					self.inspect_state = InspectState::monitor_inspect_node(&mut network, &node_to_inspect);

					self.old_graph = Some(network.clone());

					self.node_graph_errors.clear();
					let result = self.update_network(network).await;
					let node_graph_errors = self.node_graph_errors.clone();

					self.update_thumbnails = true;

					self.sender.send_generation_response(CompilationResponse { result, node_graph_errors });
				}
				GraphRuntimeRequest::ExecutionRequest(ExecutionRequest { execution_id, mut render_config, .. }) => {
					// We may want to render via the SVG pipeline even though raster was requested, if SVG Preview render mode is active or WebGPU/Vello is unavailable
					if render_config.export_format == ExportFormat::Raster
						&& (render_config.render_mode == RenderMode::SvgPreview || self.editor_api.application_io.as_ref().unwrap().gpu_executor().is_none())
					{
						render_config.export_format = ExportFormat::Svg;
					}

					let result = self.execute_network(render_config).await;
					let mut responses = VecDeque::new();
					// TODO: Only process monitor nodes if the graph has changed, not when only the Footprint changes
					if !render_config.for_eyedropper {
						self.process_monitor_nodes(&mut responses, self.update_thumbnails);
					}
					self.update_thumbnails = false;

					// Resolve the result from the inspection by accessing the monitor node
					let inspect_result = self.inspect_state.as_ref().and_then(|state| state.access(&self.executor));

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

							let raster_cpu = Raster::new_gpu(image_texture.as_ref().clone()).convert(Footprint::BOUNDLESS, executor).await;

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
							data: RenderOutputType::Texture(image_texture),
							metadata: _,
						})) if render_config.for_eyedropper => {
							let executor = self
								.editor_api
								.application_io
								.as_ref()
								.unwrap()
								.gpu_executor()
								.expect("GPU executor should be available when we receive a texture");

							let raster_cpu = Raster::new_gpu(image_texture.as_ref().clone()).convert(Footprint::BOUNDLESS, executor).await;

							self.sender.send_eyedropper_preview(raster_cpu);
							continue;
						}
						// Eyedropper render that didn't produce a texture (e.g., SVG fallback when GPU is unavailable); discard it
						_ if render_config.for_eyedropper => {
							continue;
						}
						#[cfg(all(target_family = "wasm", feature = "gpu"))]
						Ok(TaggedValue::RenderOutput(RenderOutput {
							data: RenderOutputType::Texture(image_texture),
							metadata,
						})) if !render_config.for_export => {
							self.current_viewport_texture = Some(image_texture.clone());

							let app_io = self.editor_api.application_io.as_ref().unwrap();
							let executor = app_io.gpu_executor().expect("GPU executor should be available when we receive a texture");

							self.wasm_canvas_cache.present(&image_texture, executor);

							let logical_resolution = render_config.viewport.resolution.as_dvec2() / render_config.scale;
							(
								Ok(TaggedValue::RenderOutput(RenderOutput {
									data: RenderOutputType::CanvasFrame {
										canvas_id: self.wasm_canvas_cache.id(),
										resolution: logical_resolution,
									},
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

	async fn update_network(&mut self, mut graph: NodeNetwork) -> Result<ResolvedDocumentNodeTypesDelta, (ResolvedDocumentNodeTypesDelta, String)> {
		preprocessor::expand_network(&mut graph, &self.substitutions);

		let scoped_network = wrap_network_in_scope(graph, self.editor_api.clone());

		// We assume only one output
		assert_eq!(scoped_network.exports.len(), 1, "Graph with multiple outputs not yet handled");

		let c = Compiler {};
		let proto_network = match c.compile_single(scoped_network) {
			Ok(network) => network,
			Err(e) => return Err((ResolvedDocumentNodeTypesDelta::default(), e)),
		};
		self.monitor_nodes = proto_network
			.nodes
			.iter()
			.filter(|(_, node)| node.identifier == graphene_std::memo::monitor::IDENTIFIER)
			.map(|(_, node)| node.original_location.path.clone().unwrap_or_default())
			.collect::<Vec<_>>();

		assert_ne!(proto_network.nodes.len(), 0, "No proto nodes exist?");
		self.executor.update(proto_network).await.map_err(|(types, e)| {
			self.node_graph_errors.clone_from(&e);
			(types, format!("{e:?}"))
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
			if self
				.inspect_state
				.as_ref()
				.is_some_and(|inspect_state| monitor_node_path.last().copied() == Some(inspect_state.monitor_node))
			{
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
					let bounds = io.output.thumbnail_bounding_box(DAffine2::IDENTITY, true);
					Self::render_thumbnail(&mut self.thumbnail_renders, parent_network_node_id, &io.output, bounds, responses)
				}
			}
			// Artboard thumbnail bounds come from the clipping rectangles, not the content union, since the renderer
			// clips content to those rectangles so anything outside isn't visible
			else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, Table<Artboard>>>() {
				if update_thumbnails {
					let bounds = artboard_clip_bounds(&io.output);
					Self::render_thumbnail(&mut self.thumbnail_renders, parent_network_node_id, &io.output, bounds, responses)
				}
			}
			// Vector table: vector modifications
			else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, Table<Vector>>>() {
				// Insert the vector modify
				self.vector_modify.insert(parent_network_node_id, io.output.element(0).cloned().unwrap_or_default());
			}
			// Other
			else {
				log::warn!("Failed to downcast monitor node output {parent_network_node_id:?}");
			}
		}
	}

	/// If this is `Graphic` data, regenerate click targets and thumbnails for the layers in the graph, modifying the state and updating the UI.
	fn render_thumbnail(
		thumbnail_renders: &mut HashMap<NodeId, Vec<SvgSegment>>,
		parent_network_node_id: NodeId,
		graphic: &impl Render,
		bounds: RenderBoundingBox,
		responses: &mut VecDeque<FrontendMessage>,
	) {
		// Skip thumbnails if the layer is too complex (for performance)
		if graphic.render_complexity() > 1000 {
			let old = thumbnail_renders.insert(parent_network_node_id, Vec::new());
			if old.is_none_or(|v| !v.is_empty()) {
				responses.push_back(FrontendMessage::UpdateNodeThumbnail {
					id: parent_network_node_id,
					value: "<svg viewBox=\"0 0 10 10\" data-tooltip-description=\"Dense thumbnail omitted for performance.\"><line x1=\"0\" y1=\"10\" x2=\"10\" y2=\"0\" stroke=\"red\" /></svg>"
						.to_string(),
				});
			}
			return;
		}

		// Fall back to a 1×1 rectangle if no caller offered finite bounds, then aspect-correct to the panel's 3:2 ratio
		let raw_bounds = match bounds {
			RenderBoundingBox::Rectangle(bounds) if (bounds[1] - bounds[0]) != DVec2::ZERO => bounds,
			_ => [DVec2::ZERO, DVec2::ONE],
		};
		let bounds = expand_to_thumbnail_aspect(raw_bounds);
		let new_thumbnail_svg = {
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

			render.svg
		};

		// Update frontend thumbnail
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

/// Returns the union of the artboards' clipping rectangles, used as the thumbnail bounds for an artboard layer so the
/// framing matches what's actually visible after clipping rather than the unclipped content extents.
fn artboard_clip_bounds(artboards: &Table<Artboard>) -> RenderBoundingBox {
	let mut combined: Option<[DVec2; 2]> = None;
	for index in 0..artboards.len() {
		let location: DVec2 = artboards.attribute_cloned_or_default(graphene_std::ATTR_LOCATION, index);
		let dimensions: DVec2 = artboards.attribute_cloned_or_default(graphene_std::ATTR_DIMENSIONS, index);
		let bounds = [location, location + dimensions];
		combined = Some(match combined {
			Some(existing) => [existing[0].min(bounds[0]), existing[1].max(bounds[1])],
			None => bounds,
		});
	}
	match combined {
		Some(bounds) => RenderBoundingBox::Rectangle(bounds),
		None => RenderBoundingBox::None,
	}
}

/// Expands an AABB outward (centered) to match the Layers panel thumbnail's 3:2 aspect ratio, padding the smaller axis
/// so the input's extent is always preserved.
fn expand_to_thumbnail_aspect(bounds: [DVec2; 2]) -> [DVec2; 2] {
	const THUMBNAIL_ASPECT_RATIO: f64 = 1.5;

	let size = bounds[1] - bounds[0];
	let center = (bounds[0] + bounds[1]) / 2.;
	let (width, height) = if size.x >= size.y * THUMBNAIL_ASPECT_RATIO {
		(size.x, size.x / THUMBNAIL_ASPECT_RATIO)
	} else {
		(size.y * THUMBNAIL_ASPECT_RATIO, size.y)
	};
	let half = DVec2::new(width, height) / 2.;
	[center - half, center + half]
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
pub async fn replace_application_io(application_io: PlatformApplicationIo) {
	let mut node_runtime = NODE_RUNTIME.lock();
	if let Some(node_runtime) = &mut *node_runtime {
		node_runtime.editor_api = PlatformEditorApi {
			font_cache: node_runtime.editor_api.font_cache.clone(),
			application_io: Some(application_io.into()),
			node_graph_message_sender: Box::new(node_runtime.sender.clone()),
			editor_preferences: Box::new(node_runtime.editor_preferences.clone()),
		}
		.into();
	}
}

/// Which node is inspected and which monitor node is used (if any) for the current execution
#[derive(Debug, Clone)]
struct InspectState {
	inspect_node: NodeId,
	monitor_node: NodeId,
	/// Path of the subnetwork the monitor was inserted into (i.e., the parent of `inspect_node`).
	/// Used to construct the full node path when introspecting the monitor's value.
	monitor_parent_path: Vec<NodeId>,
}
/// The resulting value from the temporary inspected during execution
#[derive(Clone, Debug, Default)]
pub struct InspectResult {
	introspected_data: Option<Arc<dyn std::any::Any + Send + Sync + 'static>>,
	/// Full path from the root network to the inspected node, with the node itself as the last element.
	/// The parent slice (`split_last().1`) is the network the node lives in, which downstream consumers
	/// (e.g. the Data panel) need when looking the node up via `network_interface.is_layer(...)` etc.
	pub inspect_node_path: Vec<NodeId>,
}

impl InspectResult {
	pub fn take_data(&mut self) -> Option<Arc<dyn std::any::Any + Send + Sync + 'static>> {
		self.introspected_data.clone()
	}
}

// This is very ugly but is required to be inside a message
impl PartialEq for InspectResult {
	fn eq(&self, other: &Self) -> bool {
		self.inspect_node_path == other.inspect_node_path
	}
}

impl InspectState {
	/// Insert the monitor node alongside the inspect node identified by `inspect_path` (full path from root, last element is the target).
	/// Returns `None` if the path is empty or doesn't resolve to a node inside a reachable subnetwork.
	pub fn monitor_inspect_node(network: &mut NodeNetwork, inspect_path: &[NodeId]) -> Option<Self> {
		let (inspect_node, parent_path) = inspect_path.split_last()?;
		let inspect_node = *inspect_node;
		let target_network = navigate_to_network_mut(network, parent_path)?;
		let monitor_id = NodeId::new();

		// It is necessary to replace the inputs before inserting the monitor node to avoid changing the input of the new monitor node
		for input in target_network.nodes.values_mut().flat_map(|node| node.inputs.iter_mut()).chain(&mut target_network.exports) {
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
		target_network.nodes.insert(monitor_id, monitor_node);

		Some(Self {
			inspect_node,
			monitor_node: monitor_id,
			monitor_parent_path: parent_path.to_vec(),
		})
	}
	/// Resolve the result from the inspection by accessing the monitor node
	fn access(&self, executor: &DynamicExecutor) -> Option<InspectResult> {
		// The executor's source map indexes by full path from root, so prepend the subnetwork path to the monitor ID.
		let mut monitor_path = self.monitor_parent_path.clone();
		monitor_path.push(self.monitor_node);
		let introspected_data = executor.introspect(&monitor_path).inspect_err(|e| warn!("Failed to introspect monitor node {e}")).ok();
		// TODO: Consider displaying the error instead of ignoring it

		let mut inspect_node_path = self.monitor_parent_path.clone();
		inspect_node_path.push(self.inspect_node);
		Some(InspectResult { inspect_node_path, introspected_data })
	}
}

/// Walks `network` down through `path`, returning a mutable reference to the nested `NodeNetwork`
/// at the end. Each path element must name a `DocumentNode` whose implementation is `Network(...)`.
/// Returns `None` if any step is missing or doesn't refer to a subnetwork.
fn navigate_to_network_mut<'a>(network: &'a mut NodeNetwork, path: &[NodeId]) -> Option<&'a mut NodeNetwork> {
	let mut current = network;
	for node_id in path {
		let node = current.nodes.get_mut(node_id)?;
		current = match &mut node.implementation {
			DocumentNodeImplementation::Network(nested) => nested,
			_ => return None,
		};
	}
	Some(current)
}
