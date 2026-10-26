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
use graphene_hash::CacheHash;
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
use graphene_std::{Artboard, Context, ContextFeatures, ContextOverrides, Graphic, NodeError, OwnedContextImpl, Poll};
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
	async_runtime: Option<AsyncRuntime>,

	editor_api: Arc<PlatformEditorApi>,
	resources: ResourceRegistry,
	node_graph_errors: GraphErrors,
	node_graph_warnings: GraphErrors,
	monitor_nodes: Vec<(Vec<NodeId>, NodeId)>,
	stale_thumbnails: HashSet<Vec<NodeId>>,
	thumbnails_in_view: HashSet<NodeId>,

	/// The node the Data panel inspects.
	inspect_state: Option<InspectState>,

	preprocessor: preprocessor::Preprocessor,

	// TODO: Remove, it doesn't need to be persisted anymore
	/// The current renders of the thumbnails for layer nodes.
	thumbnail_renders: HashMap<NodeId, Vec<SvgSegment>>,
	vector_modify: HashMap<NodeId, VectorGeometry>,
	vector_evaluations: HashMap<NodeId, (NodeId, u64)>,

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
	ThumbnailsInView {
		layers: HashSet<NodeId>,
		document_id: DocumentId,
	},
	/// The full path from the root network to the node the Data panel inspects, or empty for none.
	Inspect(Vec<NodeId>),
	InspectContextOverrides(ContextOverrides),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GraphUpdate {
	pub(super) network: NodeNetwork,
	pub(super) resources: ResourceRegistry,
	/// Full path from the root network to the node the Data panel inspects.
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
			stale_thumbnails: HashSet::new(),
			thumbnails_in_view: HashSet::new(),

			preprocessor: preprocessor::Preprocessor::new(),

			thumbnail_renders: Default::default(),
			vector_modify: Default::default(),
			vector_evaluations: Default::default(),
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
		let mut thumbnails_in_view = None;
		let mut inspect_again = false;

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
				GraphRuntimeRequest::ThumbnailsInView { layers, document_id } => thumbnails_in_view = Some((layers, document_id)),
				// A graph update applied after this loop would otherwise inspect what was selected before
				GraphRuntimeRequest::Inspect(node_to_inspect) => match &mut graph {
					Some(GraphRuntimeRequest::GraphUpdate(update)) => update.node_to_inspect = node_to_inspect,
					_ => self.inspect_state = InspectState::new(node_to_inspect),
				},
				GraphRuntimeRequest::InspectContextOverrides(overrides) => {
					if let Some(inspect_state) = &mut self.inspect_state {
						inspect_state.overrides = overrides;
						inspect_again = true;
					}
				}
			}
		}

		// Without an execution to render them along with, the thumbnails that came into view are rendered on their own
		if let Some((layers, document_id)) = thumbnails_in_view {
			self.thumbnails_in_view = layers;
			if execution.is_none() && graph.is_none() {
				let mut responses = VecDeque::new();
				self.render_stale_thumbnails(&mut responses);
				if !responses.is_empty() {
					self.sender.send(NodeGraphUpdate::Thumbnails { responses, document_id }).expect("Failed to send response");
				}
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
				GraphRuntimeRequest::ThumbnailsInView { .. } | GraphRuntimeRequest::Inspect(_) | GraphRuntimeRequest::InspectContextOverrides(_) => {}
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
				GraphRuntimeRequest::GraphUpdate(GraphUpdate { network, resources, node_to_inspect }) => {
					match &mut self.inspect_state {
						Some(inspect_state) if inspect_state.inspect_node_path == node_to_inspect => inspect_state.last_evaluation = None,
						_ => self.inspect_state = InspectState::new(node_to_inspect),
					}

					self.old_graph = Some(network.clone());
					self.resources = resources;

					self.node_graph_errors.clear();
					self.node_graph_warnings.clear();
					let result = self.update_network(network);
					let node_graph_errors = self.node_graph_errors.clone();
					let node_graph_warnings = self.node_graph_warnings.clone();

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
					let dropped_thumbnails = if !render_config.for_eyedropper { self.process_monitor_nodes(&mut responses) } else { Vec::new() };

					let inspect_result = self.inspect_state.as_mut().map(|state| state.access(&self.executor));

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

		// Without an execution to send it along with, the inspected node is evaluated again with its new overrides once the graph is up to date
		if inspect_again && let Some(inspect_state) = &mut self.inspect_state {
			let inspect_result = inspect_state.access(&self.executor);
			self.sender.send(NodeGraphUpdate::InspectResult(Box::new(inspect_result))).expect("Failed to send response");
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
		let previous_monitor_ids: HashMap<Vec<NodeId>, NodeId> = std::mem::take(&mut self.monitor_nodes).into_iter().collect();
		self.monitor_nodes = proto_network
			.nodes
			.iter()
			.filter(|(_, node)| node.identifier == graphene_std::memo::monitor::IDENTIFIER)
			.map(|(id, node)| (node.original_location.path.clone().unwrap_or_default(), *id))
			.collect();

		// A proto node's ID hashes everything upstream of it, so a monitor node whose ID stayed the same records the same output
		let changed_monitors = self.monitor_nodes.iter().filter(|(path, id)| previous_monitor_ids.get(path) != Some(id));
		self.stale_thumbnails.extend(changed_monitors.map(|(path, _)| path.clone()));
		let current_monitors: HashSet<&Vec<NodeId>> = self.monitor_nodes.iter().map(|(path, _)| path).collect();
		self.stale_thumbnails.retain(|path| current_monitors.contains(path));

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
	fn process_monitor_nodes(&mut self, responses: &mut VecDeque<FrontendMessage>) -> Vec<NodeId> {
		// TODO: Consider optimizing this since it's currently O(m*n^2), with a sort it could be made O(m * n*log(n))
		let mut dropped_thumbnails = Vec::new();
		self.thumbnail_renders.retain(|id, _| {
			let monitored = self.monitor_nodes.iter().any(|(monitor_node_path, _)| monitor_node_path.contains(id));
			if !monitored {
				dropped_thumbnails.push(*id);
			}
			monitored
		});

		for (monitor_node_path, _) in &self.monitor_nodes {
			let Some(parent_network_node_id) = monitor_parent_node(monitor_node_path) else {
				warn!("Monitor node has invalid node id");
				continue;
			};

			if !records_vector(&self.executor, monitor_node_path) {
				continue;
			}
			let tree = self.executor.tree();
			let Ok(content_node) = tree.input_node(monitor_node_path, 0) else { continue };
			let Ok(Some(context)) = tree.last_context(content_node) else { continue };
			let context_hash = context_hash(&context);
			if self.vector_evaluations.get(&parent_network_node_id) == Some(&(content_node, context_hash)) {
				continue;
			}

			let Some(output) = tree.evaluate(content_node, &context).ok().and_then(evaluated_output) else {
				continue;
			};
			let geometry = if let Some(list) = output.downcast_ref::<List<Vector>>() {
				VectorGeometry::new(list.element(0).cloned().unwrap_or_default(), paths_from_list_row_attributes(list, 0).cloned().unwrap_or_default())
			} else if let Some(item) = output.downcast_ref::<Item<Vector>>() {
				VectorGeometry::from_item(item)
			} else {
				continue;
			};
			self.vector_modify.insert(parent_network_node_id, geometry);
			self.vector_evaluations.insert(parent_network_node_id, (content_node, context_hash));
		}

		self.render_stale_thumbnails(responses);

		dropped_thumbnails
	}

	/// Renders the thumbnails of the layers in view whose content changed since they were last rendered, keeping the others stale until they come into view.
	fn render_stale_thumbnails(&mut self, responses: &mut VecDeque<FrontendMessage>) {
		for monitor_node_path in std::mem::take(&mut self.stale_thumbnails) {
			let Some(parent_network_node_id) = monitor_parent_node(&monitor_node_path) else { continue };

			if records_vector(&self.executor, &monitor_node_path) {
				continue;
			}
			if !self.thumbnails_in_view.contains(&parent_network_node_id) {
				self.stale_thumbnails.insert(monitor_node_path);
				continue;
			}

			match thumbnail_content(&self.executor, &monitor_node_path) {
				// Text-aware bounds, since the `BoundingBox` trait can't lay out `Graphic::TextList` content
				Some(ThumbnailContent::Graphics(graphics)) => {
					let bounds = graphene_std::renderer::graphic_list_bounding_box(&graphics, DAffine2::IDENTITY);
					Self::render_thumbnail(&mut self.thumbnail_renders, parent_network_node_id, &graphics, bounds, responses);
				}
				// Artboard thumbnail bounds come from the clipping rectangles, not the content union, since the renderer
				// clips content to those rectangles so anything outside isn't visible
				Some(ThumbnailContent::Artboards(artboards)) => {
					let bounds = artboard_clip_bounds(&artboards);
					Self::render_thumbnail(&mut self.thumbnail_renders, parent_network_node_id, &artboards, bounds, responses);
				}
				// Bounds need text layout, which the `BoundingBox` trait can't do for a bare `String`
				Some(ThumbnailContent::Text(text)) => {
					let bounds = graphene_std::renderer::text_list_bounding_box(&text, DAffine2::IDENTITY);
					Self::render_thumbnail(&mut self.thumbnail_renders, parent_network_node_id, &text, bounds, responses);
				}
				// Content that isn't ready yet, or wasn't evaluated since the graph changed, is tried again on the next pass
				None => {
					self.stale_thumbnails.insert(monitor_node_path);
				}
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

/// The layer a monitor node belongs to, since monitor nodes sit within the network of the document node they monitor.
fn monitor_parent_node(monitor_node_path: &[NodeId]) -> Option<NodeId> {
	monitor_node_path.len().checked_sub(2).and_then(|index| monitor_node_path.get(index)).copied()
}

fn records_vector(executor: &DynamicExecutor, monitor_node_path: &[NodeId]) -> bool {
	// TODO: Tell this from the monitor node's type once the types the borrow tree records can be compared with concrete ones
	executor
		.introspect(monitor_node_path)
		.is_ok_and(|data| data.is::<IORecord<Context, List<Vector>>>() || data.is::<IORecord<Context, Item<Vector>>>())
}

fn context_features(context: &OwnedContextImpl) -> InspectedContext {
	use graphene_std::{ExtractAnimationTime, ExtractFootprint, ExtractIndex, ExtractPointerPosition, ExtractPosition, ExtractRealTime, ExtractVarArgs};

	InspectedContext {
		features: ContextOverrides {
			footprint: context.try_footprint().copied(),
			real_time: context.try_real_time(),
			animation_time: context.try_animation_time(),
			pointer_position: context.try_pointer_position(),
			index: context.try_index().map(|index| index.collect()),
		},
		positions: context.try_position().map(|positions| positions.collect()).unwrap_or_default(),
		variables: context.varargs_len().unwrap_or_default(),
		..Default::default()
	}
}

fn evaluated_output(poll: Poll<graphene_std::registry::Any>) -> Option<graphene_std::registry::Any> {
	match poll {
		Poll::Final(output) | Poll::Partial(output) => Some(output),
		Poll::Fallback(boxed) => Some(boxed.0),
		Poll::Pending | Poll::Error(_) => None,
	}
}

fn context_hash(context: &Context) -> u64 {
	let mut hasher = std::hash::DefaultHasher::new();
	context.cache_hash(&mut hasher);
	std::hash::Hasher::finish(&hasher)
}

/// The time thumbnails are evaluated at, so they don't depend on the playhead.
const THUMBNAIL_TIME: f64 = 0.;

enum ThumbnailContent {
	Graphics(List<Graphic>),
	Artboards(List<Artboard>),
	Text(List<String>),
}

/// Evaluates the content a monitor node watches again from its last context, normalized so thumbnails don't depend on the view or playhead.
fn thumbnail_content(executor: &DynamicExecutor, monitor_node_path: &[NodeId]) -> Option<ThumbnailContent> {
	let tree = executor.tree();
	let content_node = tree.input_node(monitor_node_path, 0).ok()?;
	let context = tree.last_context(content_node).ok()??;

	// Replacing a feature the content doesn't read would only miss its caches
	let reads = tree.context_features(content_node);
	let footprint = reads.contains(ContextFeatures::FOOTPRINT).then_some(Footprint::BOUNDLESS);
	let time = reads.intersects(ContextFeatures::REAL_TIME | ContextFeatures::ANIMATION_TIME).then_some(THUMBNAIL_TIME);
	let normalized: Context = context.map(|context| Arc::new(context.with_replaced_footprint_and_time(footprint, time)));
	let output = evaluated_output(tree.evaluate(content_node, &normalized).ok()?)?;

	let content = if output.is::<List<Graphic>>() {
		ThumbnailContent::Graphics(*output.downcast().ok()?)
	} else if output.is::<Item<Graphic>>() {
		ThumbnailContent::Graphics(List::new_from_item(*output.downcast().ok()?))
	} else if output.is::<List<Artboard>>() {
		ThumbnailContent::Artboards(*output.downcast().ok()?)
	} else if output.is::<Item<Artboard>>() {
		ThumbnailContent::Artboards(List::new_from_item(*output.downcast().ok()?))
	} else if output.is::<List<String>>() {
		ThumbnailContent::Text(*output.downcast().ok()?)
	} else if output.is::<Item<String>>() {
		ThumbnailContent::Text(List::new_from_item(*output.downcast().ok()?))
	} else {
		log::warn!("Failed to downcast the content of monitor node {monitor_node_path:?} for its thumbnail");
		return None;
	};
	Some(content)
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

/// The node the Data panel inspects, which is evaluated again from the context it was last evaluated with.
struct InspectState {
	inspect_node_path: Vec<NodeId>,
	overrides: ContextOverrides,
	last_evaluation: Option<(NodeId, u64, Option<InspectedData>)>,
}

type InspectedData = Arc<dyn std::any::Any + Send + Sync + 'static>;

/// The resulting value from the temporary inspected during execution
#[derive(Clone, Debug, Default)]
pub struct InspectResult {
	introspected_data: Option<Arc<dyn std::any::Any + Send + Sync + 'static>>,
	/// Full path from the root network to the inspected node, with the node itself as the last element.
	/// The parent slice (`split_last().1`) is the network the node lives in, which downstream consumers
	/// (e.g. the Data panel) need when looking the node up via `network_interface.is_layer(...)` etc.
	pub inspect_node_path: Vec<NodeId>,
	pub context: InspectedContext,
}

/// The features of the context an inspected node was evaluated with, after the overrides replaced some of them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InspectedContext {
	pub features: ContextOverrides,
	/// The positions the context carries, from the outermost level in.
	pub positions: Vec<DVec2>,
	pub variables: usize,
	pub overrides: ContextOverrides,
	pub dependencies: ContextFeatures,
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
	fn new(inspect_node_path: Vec<NodeId>) -> Option<Self> {
		(!inspect_node_path.is_empty()).then_some(Self {
			inspect_node_path,
			overrides: ContextOverrides::default(),
			last_evaluation: None,
		})
	}

	fn access(&mut self, executor: &DynamicExecutor) -> InspectResult {
		let node = executor.tree().output_node(&self.inspect_node_path, executor.output());
		let context = node.and_then(|node| executor.tree().last_context(node).ok().flatten()).map(|context| self.with_overrides(context));

		let introspected_data = node.zip(context.clone()).and_then(|(node, context)| self.evaluate(executor, node, context));
		let features = context.as_ref().and_then(|context| context.as_deref()).map(context_features).unwrap_or_default();
		let context = InspectedContext {
			overrides: self.overrides.clone(),
			dependencies: node.map(|node| executor.tree().context_features(node)).unwrap_or_default(),
			..features
		};

		InspectResult {
			introspected_data,
			inspect_node_path: self.inspect_node_path.clone(),
			context,
		}
	}

	fn with_overrides(&self, context: Context<'static>) -> Context<'static> {
		if self.overrides.is_empty() {
			return context;
		}
		let context = context
			.as_deref()
			.map(|context| context.with_overrides(&self.overrides))
			.unwrap_or_else(|| OwnedContextImpl::empty().with_overrides(&self.overrides));
		Some(Arc::new(context))
	}

	/// Evaluates the node with the context, reusing the last result if neither changed.
	fn evaluate(&mut self, executor: &DynamicExecutor, node: NodeId, context: Context<'static>) -> Option<Arc<dyn std::any::Any + Send + Sync + 'static>> {
		let context_hash = context_hash(&context);
		if let Some((last_node, last_context_hash, data)) = &self.last_evaluation
			&& (*last_node, *last_context_hash) == (node, context_hash)
		{
			return data.clone();
		}

		let output = evaluated_output(executor.tree().evaluate(node, &context).ok()?)?;
		// Output the Data panel can't show is remembered too, so it isn't evaluated again for nothing
		let data = crate::messages::portfolio::document::data_panel::shareable_inspect_output(output);
		self.last_evaluation = Some((node, context_hash, data.clone()));
		data
	}
}
