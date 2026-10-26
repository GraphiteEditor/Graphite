use super::*;
use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::task::{Spawn, Spawner, Task};
use glam::{DAffine2, DVec2, UVec2};
use graph_craft::application_io::resource::ResourceRegistry;
use graph_craft::application_io::{PlatformApplicationIo, PlatformEditorApi};
use graph_craft::concrete;
use graph_craft::document::value::{RenderOutput, RenderOutputType, TaggedValue};
use graph_craft::document::{NodeId, NodeNetwork};
use graph_craft::graphene_compiler::{CompiledNetwork, Compiler};
use graph_craft::proto::GraphErrors;
use graphene_std::application_io::{ApplicationIo, ExportFormat, RenderConfig, Texture};
use graphene_std::bounds::RenderBoundingBox;
use graphene_std::list::{Item, List};
use graphene_std::memo::IORecord;
use graphene_std::ops::ConvertAsync;
#[cfg(all(target_family = "wasm", feature = "gpu", feature = "wasm"))]
use graphene_std::platform_application_io::canvas_utils::{Canvas, CanvasSurface, CanvasSurfaceHandle};
use graphene_std::raster_types::Raster;
use graphene_std::renderer::{Render, RenderParams, RenderSvgSegmentList, SvgRender, SvgSegment};
use graphene_std::runtime::AsyncRuntime;
use graphene_std::transform::RenderQuality;
use graphene_std::vector::style::RenderMode;
use graphene_std::vector::{Vector, VectorGeometry, paths_from_list_row_attributes};
use graphene_std::{Artboard, Context, Graphic, NodeError, Poll};
use interpreted_executor::dynamic_executor::{DynamicExecutor, ResolvedDocumentNodeTypesDelta};
use interpreted_executor::util::wrap_network_in_scope;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
	sender: Sender<NodeGraphUpdate>,
	editor_wake: Wake,
	dirty: Arc<AtomicBool>,
	pending: Option<ExecutionRequest>,
	pending_export: Option<ExecutionRequest>,
	editor_preferences: EditorPreferences,
	old_graph: Option<NodeNetwork>,
	update_thumbnails: bool,
	async_runtime: Option<AsyncRuntime>,

	editor_api: Arc<PlatformEditorApi>,
	resources: ResourceRegistry,
	node_graph_errors: GraphErrors,
	node_graph_warnings: GraphErrors,
	monitor_nodes: Vec<Vec<NodeId>>,

	/// Which node is inspected and which monitor node is used (if any) for the current execution.
	inspect_state: Option<InspectState>,

	preprocessor: preprocessor::Preprocessor,

	// TODO: Remove, it doesn't need to be persisted anymore
	/// The current renders of the thumbnails for layer nodes.
	thumbnail_renders: HashMap<NodeId, Vec<SvgSegment>>,
	vector_modify: HashMap<NodeId, VectorGeometry>,

	/// Cached surface for Wasm viewport rendering (reused across frames)
	#[cfg(all(target_family = "wasm", feature = "gpu", feature = "wasm"))]
	wasm_canvas_cache: CanvasSurfaceHandle,
	/// Currently displayed texture, the runtime keeps a reference to it to avoid the texture getting destroyed while it is still in use.
	#[cfg(all(target_family = "wasm", feature = "gpu", feature = "wasm"))]
	current_viewport_texture: Option<Texture>,
}

/// Messages passed from the editor thread to the node runtime thread.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum GraphRuntimeRequest {
	GraphUpdate(GraphUpdate),
	ExecutionRequest(ExecutionRequest),
	EditorPreferencesUpdate(EditorPreferences),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GraphUpdate {
	pub(super) network: NodeNetwork,
	pub(super) resources: ResourceRegistry,
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

impl NodeRuntime {
	pub fn new(receiver: Receiver<GraphRuntimeRequest>, sender: Sender<NodeGraphUpdate>, editor_wake: Wake) -> Self {
		Self {
			executor: DynamicExecutor::default(),
			receiver,
			sender,
			editor_wake,
			dirty: Default::default(),
			pending: None,
			pending_export: None,
			editor_preferences: EditorPreferences::default(),
			old_graph: None,
			resources: ResourceRegistry::default(),
			update_thumbnails: true,
			async_runtime: None,

			editor_api: PlatformEditorApi {
				editor_preferences: Box::new(EditorPreferences::default()),

				#[cfg(not(test))]
				application_io: None,

				#[cfg(test)]
				application_io: Some(PlatformApplicationIo::default().into()),
			}
			.into(),

			node_graph_errors: Vec::new(),
			node_graph_warnings: Vec::new(),
			monitor_nodes: Vec::new(),

			preprocessor: preprocessor::Preprocessor::new(),

			thumbnail_renders: Default::default(),
			vector_modify: Default::default(),
			inspect_state: None,
			#[cfg(all(target_family = "wasm", feature = "gpu"))]
			wasm_canvas_cache: CanvasSurfaceHandle::new(),
			#[cfg(all(target_family = "wasm", feature = "gpu"))]
			current_viewport_texture: None,
		}
	}

	pub async fn run(&mut self) -> Option<Texture> {
		let texture = self.handle_requests().await;
		(self.editor_wake)();
		texture
	}

	async fn handle_requests(&mut self) -> Option<Texture> {
		let mut preferences = None;
		let mut graph = None;
		let mut eyedropper = None;
		let mut execution = None;

		for request in self.pending_export.is_none().then_some(&self.receiver).into_iter().flat_map(Receiver::try_iter) {
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
				GraphRuntimeRequest::EditorPreferencesUpdate(_) => preferences = Some(request),
			}
		}

		if self.dirty.swap(false, Ordering::Acquire) && execution.is_none() {
			execution = self.pending_export.or(self.pending).map(GraphRuntimeRequest::ExecutionRequest);
		}

		// Eydropper should use the same time and pointer to not invalidate the cache
		if let Some(GraphRuntimeRequest::ExecutionRequest(eyedropper)) = &mut eyedropper
			&& let Some(GraphRuntimeRequest::ExecutionRequest(execution)) = &execution
		{
			eyedropper.render_config.time = execution.render_config.time;
			eyedropper.render_config.pointer = execution.render_config.pointer;
		}

		let requests = [preferences, graph, eyedropper, execution].into_iter().flatten();

		for request in requests {
			match request {
				GraphRuntimeRequest::EditorPreferencesUpdate(preferences) => {
					self.editor_preferences = preferences.clone();
					self.editor_api = PlatformEditorApi {
						application_io: self.editor_api.application_io.clone(),
						editor_preferences: Box::new(preferences),
					}
					.into();
					if let Some(graph) = self.old_graph.clone() {
						// We ignore this result as compilation errors should have been reported in an earlier iteration
						let _ = self.update_network(graph);
					}
				}
				GraphRuntimeRequest::GraphUpdate(GraphUpdate {
					mut network,
					resources,
					node_to_inspect,
				}) => {
					// Insert the monitor node to manage the inspection
					self.inspect_state = InspectState::monitor_inspect_node(&mut network, &node_to_inspect);

					self.old_graph = Some(network.clone());
					self.resources = resources;

					self.node_graph_errors.clear();
					self.node_graph_warnings.clear();
					let result = self.update_network(network);
					let node_graph_errors = self.node_graph_errors.clone();
					let node_graph_warnings = self.node_graph_warnings.clone();

					self.update_thumbnails = true;

					self.sender
						.send(NodeGraphUpdate::CompilationResponse(CompilationResponse {
							result,
							node_graph_errors,
							node_graph_warnings,
						}))
						.expect("Failed to send response");
				}
				GraphRuntimeRequest::ExecutionRequest(request @ ExecutionRequest { execution_id, mut render_config, .. }) => {
					if !render_config.for_export && !render_config.for_eyedropper {
						self.pending = Some(request);
					}
					if render_config.for_export {
						self.pending_export = None;
					}

					// Render via the SVG pipeline even though raster was requested if SVG Preview render mode is active or there's no GPU, as in headless tests
					if render_config.export_format == ExportFormat::Raster
						&& (render_config.render_mode == RenderMode::SvgPreview || self.editor_api.application_io.as_ref().unwrap().gpu_executor().is_none())
					{
						render_config.export_format = ExportFormat::Svg;
					}

					let mut result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.execute_network(render_config))) {
						Ok(Ok(Poll::Pending)) => ExecutionResult::Pending,
						Ok(Ok(Poll::Partial(_))) if render_config.for_export => ExecutionResult::Pending,
						Ok(Ok(Poll::Final(value) | Poll::Partial(value))) => ExecutionResult::Value(value),
						Ok(Ok(Poll::Fallback(boxed))) => {
							let (value, error) = *boxed;
							ExecutionResult::Fallback(value, self.resolve_errors(&error))
						}
						Ok(Ok(Poll::Error(error))) => ExecutionResult::Error(self.resolve_errors(&error)),
						Ok(Err(error)) => ExecutionResult::Error(Box::new([(Arc::clone(NodeError::new(error).error()), Box::default())])),
						Err(payload) => {
							let response = ExecutionResponse {
								execution_id,
								result: ExecutionResult::Panic(payload),
								responses: VecDeque::new(),
								dropped_thumbnails: Vec::new(),
								vector_modify: HashMap::new(),
								inspect_result: None,
							};
							self.sender.send(NodeGraphUpdate::ExecutionResponse(Box::new(response))).expect("Failed to send response");
							return None;
						}
					};

					match (render_config.for_export, &result) {
						(true, ExecutionResult::Pending) => {
							self.pending_export = Some(request);
							return None;
						}
						(true, _) => self.dirty.store(true, Ordering::Release),
						_ => {}
					}

					let mut responses = VecDeque::new();
					// TODO: Only process monitor nodes if the graph has changed, not when only the Footprint changes
					let dropped_thumbnails = if !render_config.for_eyedropper {
						self.process_monitor_nodes(&mut responses, self.update_thumbnails)
					} else {
						Vec::new()
					};
					self.update_thumbnails = false;

					// Resolve the result from the inspection by accessing the monitor node
					let inspect_result = self.inspect_state.as_ref().and_then(|state| state.access(&self.executor));

					let output = match &mut result {
						ExecutionResult::Value(value) | ExecutionResult::Fallback(value, _) => Some(value),
						ExecutionResult::Pending | ExecutionResult::Error(_) | ExecutionResult::Panic(_) => None,
					};
					let texture = if let Some(TaggedValue::RenderOutput(RenderOutput { data, .. })) = output
						&& let RenderOutputType::Texture(texture) = data
					{
						let texture = texture.clone();
						if render_config.for_export || render_config.for_eyedropper {
							let executor = self
								.editor_api
								.application_io
								.as_ref()
								.unwrap()
								.gpu_executor()
								.expect("GPU executor should be available when we receive a texture");

							let raster_cpu = Raster::new_gpu(texture).convert(Footprint::BOUNDLESS, executor.clone()).await;

							if render_config.for_eyedropper {
								self.sender.send(NodeGraphUpdate::EyedropperPreview(raster_cpu)).expect("Failed to send response");
								continue;
							}

							let (buffer, width, height) = raster_cpu.to_flat_u8();
							*data = RenderOutputType::Buffer { data: buffer, width, height };
							None
						} else {
							#[cfg(all(target_family = "wasm", feature = "gpu"))]
							{
								self.current_viewport_texture = Some(texture.clone());

								let app_io = self.editor_api.application_io.as_ref().unwrap();
								let executor = app_io.gpu_executor().expect("GPU executor should be available when we receive a texture");

								self.wasm_canvas_cache.present(&texture, executor);

								let logical_resolution = render_config.viewport.resolution.as_dvec2() / render_config.scale;
								*data = RenderOutputType::CanvasFrame {
									canvas_id: self.wasm_canvas_cache.id(),
									resolution: logical_resolution,
								};
								None
							}
							#[cfg(not(all(target_family = "wasm", feature = "gpu")))]
							{
								Some(texture)
							}
						}
					} else {
						None
					};
					// Eyedropper render that didn't produce a texture (e.g., SVG fallback when GPU is unavailable); discard it
					if render_config.for_eyedropper {
						continue;
					}

					let response = ExecutionResponse {
						execution_id,
						result,
						responses,
						dropped_thumbnails,
						vector_modify: self.vector_modify.clone(),
						inspect_result,
					};
					self.sender.send(NodeGraphUpdate::ExecutionResponse(Box::new(response))).expect("Failed to send response");
					return texture;
				}
			}
		}
		None
	}

	fn update_network(&mut self, graph: NodeNetwork) -> Result<ResolvedDocumentNodeTypesDelta, (ResolvedDocumentNodeTypesDelta, String)> {
		let mut scoped_network = wrap_network_in_scope(graph, self.editor_api.clone(), self.async_runtime().handle());

		if let Err(e) = self.preprocessor.preprocess(&mut scoped_network, &|resource_id| self.resources.hash(&resource_id)) {
			return Err((ResolvedDocumentNodeTypesDelta::default(), e.to_string()));
		}

		// We assume only one output
		assert_eq!(scoped_network.exports.len(), 1, "Graph with multiple outputs not yet handled");

		let c = Compiler {};
		let CompiledNetwork { network: proto_network, warnings } = match c.compile_single(scoped_network) {
			Ok(compiled_network) => compiled_network,
			Err(e) => return Err((ResolvedDocumentNodeTypesDelta::default(), e)),
		};
		self.node_graph_warnings = warnings;
		self.monitor_nodes = proto_network
			.nodes
			.iter()
			.filter(|(_, node)| node.identifier == graphene_std::memo::monitor::IDENTIFIER)
			.map(|(_, node)| node.original_location.path.clone().unwrap_or_default())
			.collect::<Vec<_>>();

		assert_ne!(proto_network.nodes.len(), 0, "No proto nodes exist?");
		let result = self.executor.update(proto_network).map_err(|(types, e)| {
			self.node_graph_errors.clone_from(&e);
			(types, format!("{e:?}"))
		});
		self.async_runtime().retain_sources(self.executor.sources());
		result
	}

	fn execute_network(&mut self, render_config: RenderConfig) -> Result<Poll<TaggedValue>, String> {
		use graph_craft::graphene_compiler::Executor;

		match self.executor.input_type() {
			Some(t) if t == concrete!(Context) => (&self.executor)
				.execute(render_config.into_context_with_generations(self.async_runtime().snapshot()))
				.map_err(|e| e.to_string()),
			Some(t) if t == concrete!(()) => (&self.executor).execute(()).map_err(|e| e.to_string()),
			Some(t) => Err(format!("Invalid input type {t:?}")),
			_ => Err(format!("No input type:\n{:?}", self.node_graph_errors)),
		}
	}

	fn resolve_errors(&self, error: &NodeError) -> EvaluationError {
		let source_map = self.executor.tree().source_map();
		let mut errors = Vec::new();
		let mut pending = vec![(Some(self.executor.output()), error)];
		while let Some((node, error)) = pending.pop() {
			let origin = node.and_then(|node| self.executor.find_origin(node, error));
			let start = errors.len();
			if let Some(origin) = origin {
				errors.extend(source_map.iter().filter(|(_, (node, _))| *node == origin).map(|(path, _)| (Arc::clone(error.error()), path.clone())));
				errors[start..].sort_unstable_by(|(_, a), (_, b)| a.cmp(b));
			}
			if errors.len() == start {
				errors.push((Arc::clone(error.error()), Box::default()));
			}
			pending.extend(error.upstream().iter().rev().map(|upstream| (origin, upstream)));
		}
		errors.into_boxed_slice()
	}

	/// Updates state data, giving back the layers whose thumbnails were dropped because they're no longer monitored.
	fn process_monitor_nodes(&mut self, responses: &mut VecDeque<FrontendMessage>, update_thumbnails: bool) -> Vec<NodeId> {
		// TODO: Consider optimizing this since it's currently O(m*n^2), with a sort it could be made O(m * n*log(n))
		let mut dropped_thumbnails = Vec::new();
		self.thumbnail_renders.retain(|id, _| {
			let monitored = self.monitor_nodes.iter().any(|monitor_node_path| monitor_node_path.contains(id));
			if !monitored {
				dropped_thumbnails.push(*id);
			}
			monitored
		});

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

			// Graphic list: thumbnail (text-aware bounds, since the `BoundingBox` trait can't lay out `Graphic::TextList` content)
			if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, List<Graphic>>>() {
				if update_thumbnails {
					let bounds = graphene_std::renderer::graphic_list_bounding_box(&io.output, DAffine2::IDENTITY);
					Self::render_thumbnail(&mut self.thumbnail_renders, parent_network_node_id, &io.output, bounds, responses)
				}
			}
			// Artboard thumbnail bounds come from the clipping rectangles, not the content union, since the renderer
			// clips content to those rectangles so anything outside isn't visible
			else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, List<Artboard>>>() {
				if update_thumbnails {
					let bounds = artboard_clip_bounds(&io.output);
					Self::render_thumbnail(&mut self.thumbnail_renders, parent_network_node_id, &io.output, bounds, responses)
				}
			}
			// Vector list: vector modifications
			else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, List<Vector>>>() {
				// Insert the vector modify
				let geometry = VectorGeometry::new(
					io.output.element(0).cloned().unwrap_or_default(),
					paths_from_list_row_attributes(&io.output, 0).cloned().unwrap_or_default(),
				);
				self.vector_modify.insert(parent_network_node_id, geometry);
			}
			// String list: thumbnail (bounds need text layout, which the `BoundingBox` trait can't do for a bare `String`)
			else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, List<String>>>() {
				if update_thumbnails {
					let bounds = graphene_std::renderer::text_list_bounding_box(&io.output, DAffine2::IDENTITY);
					Self::render_thumbnail(&mut self.thumbnail_renders, parent_network_node_id, &io.output, bounds, responses)
				}
			}
			// Rank-0 wires record single items; each arm mirrors its list counterpart through a singleton raise
			else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, Item<Graphic>>>() {
				if update_thumbnails {
					let singleton = List::new_from_item(io.output.clone());
					let bounds = graphene_std::renderer::graphic_list_bounding_box(&singleton, DAffine2::IDENTITY);
					Self::render_thumbnail(&mut self.thumbnail_renders, parent_network_node_id, &singleton, bounds, responses)
				}
			} else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, Item<Artboard>>>() {
				if update_thumbnails {
					let singleton = List::new_from_item(io.output.clone());
					let bounds = artboard_clip_bounds(&singleton);
					Self::render_thumbnail(&mut self.thumbnail_renders, parent_network_node_id, &singleton, bounds, responses)
				}
			} else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, Item<Vector>>>() {
				self.vector_modify.insert(parent_network_node_id, VectorGeometry::from_item(&io.output));
			} else if let Some(io) = introspected_data.downcast_ref::<IORecord<Context, Item<String>>>() {
				if update_thumbnails {
					let singleton = List::new_from_item(io.output.clone());
					let bounds = graphene_std::renderer::text_list_bounding_box(&singleton, DAffine2::IDENTITY);
					Self::render_thumbnail(&mut self.thumbnail_renders, parent_network_node_id, &singleton, bounds, responses)
				}
			}
			// Other
			else {
				log::warn!("Failed to downcast monitor node output {parent_network_node_id:?}");
			}
		}

		dropped_thumbnails
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
fn artboard_clip_bounds(artboards: &List<Artboard>) -> RenderBoundingBox {
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

impl NodeRuntime {
	pub(crate) fn replace_application_io(&mut self, application_io: PlatformApplicationIo) {
		self.editor_api = PlatformEditorApi {
			application_io: Some(application_io.into()),
			editor_preferences: Box::new(self.editor_preferences.clone()),
		}
		.into();
	}

	pub(crate) fn replace_task_executor(&mut self, spawner: Spawner, wake: Wake) {
		let spawn = NodeTaskExecutor {
			spawner,
			dirty: self.dirty.clone(),
			wake,
		};
		self.async_runtime = Some(AsyncRuntime::new(Arc::new(spawn)));
	}

	fn async_runtime(&self) -> &AsyncRuntime {
		self.async_runtime.as_ref().expect("the task executor is installed before the first graph update")
	}
}

struct NodeTaskExecutor {
	spawner: Spawner,
	dirty: Arc<AtomicBool>,
	wake: Wake,
}

impl Spawn for NodeTaskExecutor {
	fn spawn(&self, task: Task) {
		let dirty = self.dirty.clone();
		let wake = self.wake.clone();
		self.spawner.spawn(Box::pin(async move {
			task.await;
			dirty.store(true, Ordering::Release);
			wake();
		}));
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
	/// Returns `None` if the path is empty, doesn't resolve to a node inside a reachable subnetwork, or the target has no
	/// flatten-safe primary output to monitor (e.g. an empty merged subnetwork), which would otherwise leave the monitor's
	/// input dangling once the subnetwork is flattened away.
	pub fn monitor_inspect_node(network: &mut NodeNetwork, inspect_path: &[NodeId]) -> Option<Self> {
		let (inspect_node, parent_path) = inspect_path.split_last()?;
		let inspect_node = *inspect_node;
		let target_network = navigate_to_network_mut(network, parent_path)?;

		// A subnetwork's primary output only survives flattening if its first export is a node
		let monitorable = match &target_network.nodes.get(&inspect_node)?.implementation {
			DocumentNodeImplementation::Network(inner) => matches!(inner.exports.first(), Some(NodeInput::Node { .. })),
			_ => true,
		};
		if !monitorable {
			return None;
		}

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
