use super::*;
use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::portfolio::document::overlays::utility_types::{OverlayContext, RenderedLayers};
use crate::messages::portfolio::document::utility_types::network_interface::RootNode;
use crate::task::{Spawn, Spawner, Task};
use graph_craft::application_io::resource::ResourceRegistry;
use graph_craft::application_io::{PlatformApplicationIo, PlatformEditorApi};
use graph_craft::document::value::{RenderOutput, RenderOutputType, TaggedValue};
use graph_craft::document::{NodeId, NodeNetwork};
use graph_craft::graphene_compiler::{CompileError, CompiledNetwork, Compiler};
use graph_craft::proto::GraphErrors;
use graphene_hash::CacheHash;
use graphene_std::application_io::{ApplicationIo, ExportFormat, Texture};
use graphene_std::ops::ConvertAsync;
#[cfg(all(target_family = "wasm", feature = "wasm"))]
use graphene_std::platform_application_io::canvas_utils::{Canvas, CanvasSurface, CanvasSurfaceHandle};
use graphene_std::raster_types::Raster;
use graphene_std::runtime::AsyncRuntime;
use graphene_std::vector::style::RenderMode;
use graphene_std::{Context, NodeError, Poll};
use interpreted_executor::dynamic_executor::{DynamicExecutor, OutputHandle, ResolvedDocumentNodeTypesDelta};
use interpreted_executor::layer_paths::write_layer_paths;
use interpreted_executor::util::wrap_network_in_scope;
use observer::Observer;
pub use observer::{InspectResult, InspectedContext, Observation, ObservedConnector, ObservedValue};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use thumbnails::Thumbnails;
use vello_gpu::peniko::ImageSampler;
use wgpu_executor::{VelloCanvas, VelloSceneBuilder};

#[cfg(test)]
mod layer_path_tests;
mod observer;
mod thumbnails;

/// Persistent data between graph executions. It's updated via message passing from the editor thread with [`GraphRuntimeRequest`]`.
/// Some of these fields are put into a [`PlatformEditorApi`] which is passed to the final compiled graph network upon each execution.
/// Once the implementation is finished, this will live in a separate thread. Right now it's part of the main JS thread, but its own separate JS stack frame independent from the editor.
pub struct NodeRuntime {
	#[cfg(test)]
	pub(super) executor: DynamicExecutor,
	#[cfg(not(test))]
	executor: DynamicExecutor,
	output: OutputHandle,
	receiver: Receiver<GraphRuntimeRequest>,
	sender: Sender<NodeGraphUpdate>,
	editor_wake: Wake,
	dirty: Arc<AtomicBool>,
	pending: Option<ExecutionRequest>,
	pending_export: Option<ExecutionRequest>,
	pending_eyedropper: Option<ExecutionRequest>,
	pending_thumbnails: Option<DocumentId>,
	/// The network the executor last compiled successfully, with previews applied.
	old_graph: Option<NodeNetwork>,
	/// Whether the latest network failed to compile, leaving the executor with an older one that mustn't be shown in its place.
	compile_failed: bool,
	async_runtime: Option<AsyncRuntime>,

	editor_api: Arc<PlatformEditorApi>,
	resources: ResourceRegistry,
	node_graph_errors: GraphErrors,
	node_graph_warnings: GraphErrors,
	thumbnails: Thumbnails,
	observer: Observer<ObservationKey>,
	/// Whether an observation changed since its results were last sent.
	observations_changed: bool,

	preprocessor: preprocessor::Preprocessor,

	/// Cached surface for Wasm viewport rendering (reused across frames)
	#[cfg(all(target_family = "wasm", feature = "wasm"))]
	wasm_canvas_cache: CanvasSurfaceHandle,
	/// The viewport's latest artwork.
	viewport_artwork: Option<ViewportArtwork>,
	/// The layers from the render of the viewport's latest artwork.
	rendered_layers: RenderedLayers,
	/// The latest overlays, rendered again over each new artwork.
	overlays: Option<OverlayContext>,
	/// Whether the overlays or the artwork changed since the overlays were last rendered.
	overlays_stale: bool,
	overlays_canvas: VelloCanvas,
	/// The latest rendered overlays.
	overlays_texture: Option<Texture>,
}

/// What the viewport shows beneath its overlays.
enum ViewportArtwork {
	/// Rendered by Vello, with the overlays drawn over it.
	Texture(Texture),
	/// An SVG the frontend draws, with the overlays shown alone above it.
	Svg,
}

/// Messages passed from the editor thread to the node runtime thread.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum GraphRuntimeRequest {
	GraphUpdate(GraphUpdate),
	ExecutionRequest(ExecutionRequest),
	ThumbnailsInView {
		layers: HashSet<NodeId>,
		document_id: DocumentId,
	},
	/// Starts, changes, or with `None` stops one observation without a graph update.
	Observe {
		key: ObservationKey,
		observation: Option<Observation>,
	},
	ClipboardSvg {
		id: u64,
		layers: Vec<NodeId>,
	},
	RenderOverlays(OverlayContext),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GraphUpdate {
	pub(super) network: NodeNetwork,
	pub(super) resources: ResourceRegistry,
	/// The node each network renders in place of its export, paired with that network's path.
	pub(super) previewed: Vec<(Vec<NodeId>, RootNode)>,
	/// Full path from the root network to every layer, which are the nodes with thumbnails.
	pub(super) layers: Vec<Vec<NodeId>>,
	/// The connectors whose values are sent back after each evaluation, replacing those observed before.
	pub(super) observations: HashMap<ObservationKey, Observation>,
}

/// Replaces each network's primary export with its previewed node.
pub(crate) fn apply_previews(network: &mut NodeNetwork, previewed: &[(Vec<NodeId>, RootNode)]) {
	for (network_path, previewed) in previewed {
		let Some(nested) = network.nested_network_mut(network_path) else { continue };
		let Some(export) = nested.exports.first_mut() else { continue };
		*export = NodeInput::node(previewed.node_id, previewed.output_index);
	}
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExportConfig {
	pub name: String,
	pub file_type: FileType,
	pub scale_factor: f64,
	pub bounds: ExportBounds,
	pub artboard_name: Option<String>,
	pub artboard_count: usize,
}

impl NodeRuntime {
	pub fn new(receiver: Receiver<GraphRuntimeRequest>, sender: Sender<NodeGraphUpdate>, editor_wake: Wake) -> Self {
		Self {
			executor: DynamicExecutor::default(),
			output: OutputHandle::default(),
			receiver,
			sender,
			editor_wake,
			dirty: Default::default(),
			pending: None,
			pending_export: None,
			pending_eyedropper: None,
			pending_thumbnails: None,
			old_graph: None,
			compile_failed: false,
			resources: ResourceRegistry::default(),
			async_runtime: None,

			editor_api: PlatformEditorApi {
				#[cfg(not(test))]
				application_io: None,

				#[cfg(test)]
				application_io: Some(PlatformApplicationIo::default().into()),
			}
			.into(),

			node_graph_errors: Vec::new(),
			node_graph_warnings: Vec::new(),
			thumbnails: Thumbnails::new(),
			observer: Observer::default(),
			observations_changed: false,

			preprocessor: preprocessor::Preprocessor::new(),

			#[cfg(all(target_family = "wasm", feature = "wasm"))]
			wasm_canvas_cache: CanvasSurfaceHandle::new(),
			viewport_artwork: None,
			rendered_layers: RenderedLayers::default(),
			overlays: None,
			overlays_stale: false,
			overlays_canvas: VelloCanvas::default(),
			overlays_texture: None,
		}
	}

	pub fn update(&mut self) {
		if self.pending_export.is_some() {
			return;
		}

		let mut graph = None;
		for request in self.receiver.try_iter() {
			match request {
				GraphRuntimeRequest::GraphUpdate(update) => graph = Some(update),
				GraphRuntimeRequest::ExecutionRequest(request) if request.render_config.for_eyedropper => self.pending_eyedropper = Some(request),
				GraphRuntimeRequest::ExecutionRequest(request) if request.render_config.for_export => {
					self.pending_export = Some(request);
					self.dirty.store(true, Ordering::Release);
					break;
				}
				GraphRuntimeRequest::ExecutionRequest(request) => {
					self.pending = Some(request);
					self.dirty.store(true, Ordering::Release);
				}
				GraphRuntimeRequest::ThumbnailsInView { layers, document_id } => {
					self.thumbnails.set_in_view(layers);
					self.pending_thumbnails = Some(document_id);
				}
				// A graph update applied after this loop would otherwise replace this with what it observed before
				GraphRuntimeRequest::Observe { key, observation } => match (&mut graph, observation) {
					(Some(update), Some(observation)) => _ = update.observations.insert(key, observation),
					(Some(update), None) => _ = update.observations.remove(&key),
					(None, observation) => {
						self.observer.set(key, observation);
						self.observations_changed = true;
					}
				},
				GraphRuntimeRequest::ClipboardSvg { id, layers } => {
					let svg = self.old_graph.as_ref().and_then(|network| thumbnails::layers_svg(&layers, network, &self.executor));
					self.sender.send(NodeGraphUpdate::ClipboardSvg { id, svg }).expect("Failed to send response");
				}
				GraphRuntimeRequest::RenderOverlays(context) => {
					self.overlays = Some(context);
					self.overlays_stale = true;
				}
			}
		}

		let Some(GraphUpdate {
			mut network,
			resources,
			previewed,
			layers,
			observations,
		}) = graph
		else {
			return;
		};

		apply_previews(&mut network, &previewed);
		write_layer_paths(&mut network, &layers);
		self.observer.replace_all(observations);
		self.resources = resources;

		self.node_graph_errors.clear();
		self.node_graph_warnings.clear();
		let result = self.update_network(network.clone());
		self.compile_failed = result.is_err();
		if result.is_ok() {
			self.thumbnails.update_sources(&layers, &network, &self.executor);
			self.old_graph = Some(network);
		}
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

	pub async fn run(&mut self) -> Option<Texture> {
		let dirty = self.dirty.swap(false, Ordering::Acquire);
		let mut texture = None;

		if let Some(export) = self.pending_export {
			if dirty {
				self.run_export(export).await;
			}
		} else {
			let viewport = if dirty { self.pending } else { None };

			if let Some(document_id) = self.pending_thumbnails.take()
				&& viewport.is_none()
				&& !self.compile_failed
			{
				let mut responses = VecDeque::new();
				self.thumbnails.render_stale(&self.executor, &self.editor_api, &mut responses).await;
				if !responses.is_empty() {
					self.sender.send(NodeGraphUpdate::Thumbnails { responses, document_id }).expect("Failed to send response");
				}
			}

			if std::mem::take(&mut self.observations_changed) && viewport.is_none() {
				let observations = self.observer.evaluate(&self.executor, self.old_graph.as_ref());
				self.sender.send(NodeGraphUpdate::Observations(observations)).expect("Failed to send response");
			}

			if let Some(eyedropper) = self.pending_eyedropper.take() {
				self.run_eyedropper(eyedropper).await;
			}

			if let Some(viewport) = viewport {
				texture = self.run_viewport(viewport).await;
			}

			// Overlays that changed without new artwork are drawn over the artwork already shown
			if self.overlays_stale {
				texture = self.composite_viewport_frame();
			}
		}

		(self.editor_wake)();
		texture
	}

	async fn run_viewport(&mut self, request: ExecutionRequest) -> Option<Texture> {
		self.thumbnails.set_scale(request.render_config.scale);

		let mut result = self.evaluate(request)?;
		let observations = self.observer.evaluate(&self.executor, self.old_graph.as_ref());

		#[cfg(all(target_family = "wasm", feature = "wasm"))]
		let resolution = request.render_config.viewport.resolution.as_dvec2() / request.render_config.scale;

		let texture = if let Some(RenderOutput { data, metadata }) = result.render_output()
			&& let RenderOutputType::Texture(texture) = data
		{
			self.viewport_artwork = Some(ViewportArtwork::Texture(texture.clone()));
			let path_contents = path_contents(&observations);
			self.rendered_layers = RenderedLayers::new(metadata, &path_contents, self.old_graph.as_ref());
			self.overlays_stale = true;
			let frame = self.composite_viewport_frame();

			#[cfg(all(target_family = "wasm", feature = "wasm"))]
			{
				*data = RenderOutputType::CanvasFrame {
					canvas_id: self.wasm_canvas_cache.id(),
					resolution,
				};
			}

			frame
		} else if request.render_config.export_format == ExportFormat::Raster
			&& let Some(RenderOutput {
				data: RenderOutputType::Svg { .. },
				metadata,
			}) = result.render_output()
		{
			// The viewport was rendered as SVG in place of the requested raster, so the frontend draws it beneath the overlays
			self.viewport_artwork = Some(ViewportArtwork::Svg);
			let path_contents = path_contents(&observations);
			self.rendered_layers = RenderedLayers::new(metadata, &path_contents, self.old_graph.as_ref());
			self.overlays_stale = true;
			let frame = self.composite_viewport_frame();

			#[cfg(all(target_family = "wasm", feature = "wasm"))]
			if let Some(RenderOutput {
				data: RenderOutputType::Svg { svg, .. },
				..
			}) = result.render_output()
			{
				let end = svg.rfind("</svg>").unwrap_or(svg.len());
				svg.insert_str(end, &viewport_canvas_placeholder(self.wasm_canvas_cache.id(), resolution));
			}

			frame
		} else {
			None
		};

		self.respond(request.execution_id, result, observations).await;
		texture
	}

	/// Draws the latest overlays over the viewport's artwork, or alone above SVG artwork, presenting the frame on web.
	fn composite_viewport_frame(&mut self) -> Option<Texture> {
		let artwork = self.viewport_artwork.as_ref()?;
		let executor = self.editor_api.application_io.as_ref().and_then(|application_io| application_io.gpu_executor())?;

		if std::mem::take(&mut self.overlays_stale)
			&& let Some(overlays) = &self.overlays
		{
			self.overlays_texture = executor
				.render_vello_scene_on_canvas(&mut self.overlays_canvas, overlays.render(&self.rendered_layers))
				.inspect_err(|error| log::error!("Failed to render the overlays: {error:?}"))
				.ok();
		}

		let frame = match (artwork, &self.overlays_texture) {
			(ViewportArtwork::Texture(artwork), Some(overlays)) => {
				let (width, height) = (artwork.width(), artwork.height());
				let max_texture_size = executor.context().device.limits().max_texture_dimension_2d;
				let overlays_to_artwork = kurbo::Affine::scale_non_uniform(width as f64 / overlays.width() as f64, height as f64 / overlays.height() as f64);

				let mut scene = VelloSceneBuilder::new(width as u16, height as u16, kurbo::Affine::IDENTITY, max_texture_size);
				scene.draw_texture(artwork.clone(), ImageSampler::new(), kurbo::Affine::IDENTITY);
				scene.draw_texture(overlays.clone(), ImageSampler::new(), overlays_to_artwork);

				executor
					.render_vello_scene_on_canvas(&mut self.overlays_canvas, scene)
					.inspect_err(|error| log::error!("Failed to draw the overlays over the artwork: {error:?}"))
					.unwrap_or_else(|_| artwork.clone())
			}
			(ViewportArtwork::Texture(artwork), None) => artwork.clone(),
			(ViewportArtwork::Svg, Some(overlays)) => overlays.clone(),
			(ViewportArtwork::Svg, None) => return None,
		};

		#[cfg(all(target_family = "wasm", feature = "wasm"))]
		self.wasm_canvas_cache.present(&frame, executor);

		Some(frame)
	}

	async fn run_export(&mut self, request: ExecutionRequest) {
		let result = self.evaluate(request);
		if matches!(result, Some(ExecutionResult::Pending)) {
			return;
		}

		self.pending_export = None;
		let Some(mut result) = result else { return };
		self.dirty.store(true, Ordering::Release);

		if let Some(RenderOutput { data, .. }) = result.render_output()
			&& let RenderOutputType::Texture(texture) = data
		{
			let (buffer, width, height) = self.read_back(texture.clone()).await.to_flat_u8();
			*data = RenderOutputType::Buffer { data: buffer, width, height };
		}
		let observations = self.observer.evaluate(&self.executor, self.old_graph.as_ref());
		self.respond(request.execution_id, result, observations).await;
	}

	async fn run_eyedropper(&mut self, mut request: ExecutionRequest) {
		// Eyedropper should use the same time and pointer to not invalidate the cache
		if let Some(viewport) = self.pending {
			request.render_config.time = viewport.render_config.time;
			request.render_config.pointer = viewport.render_config.pointer;
		}

		if let Some(mut result) = self.evaluate(request)
			&& let Some(RenderOutput {
				data: RenderOutputType::Texture(texture),
				..
			}) = result.render_output()
		{
			let raster = self.read_back(texture.clone()).await;
			self.sender.send(NodeGraphUpdate::EyedropperPreview(raster)).expect("Failed to send response");
		}
	}

	fn evaluate(&mut self, request: ExecutionRequest) -> Option<ExecutionResult> {
		if self.compile_failed {
			return Some(ExecutionResult::Uncompiled);
		}

		let ExecutionRequest { execution_id, mut render_config } = request;

		// Render via the SVG pipeline even though raster was requested if SVG Preview render mode is active
		if render_config.export_format == ExportFormat::Raster && render_config.render_mode == RenderMode::SvgPreview {
			render_config.export_format = ExportFormat::Svg;
		}

		let context = render_config.into_context_with_generations(self.async_runtime().snapshot());
		let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.executor.tree().eval_tagged_value(self.output.node(), &context))) {
			Ok(Ok(Poll::Pending)) => ExecutionResult::Pending,
			Ok(Ok(Poll::Partial(_))) if render_config.for_export => ExecutionResult::Pending,
			Ok(Ok(Poll::Partial(value))) => ExecutionResult::Partial(value),
			Ok(Ok(Poll::Final(value))) => ExecutionResult::Value(value),
			Ok(Ok(Poll::Fallback(boxed))) => {
				let (value, error) = *boxed;
				ExecutionResult::Fallback(value, self.resolve_errors(&error))
			}
			Ok(Ok(Poll::Error(error))) => ExecutionResult::Error(self.resolve_errors(&error)),
			Ok(Err(error)) => ExecutionResult::Error(EvaluationErrors(Box::new([(Arc::clone(NodeError::new(error).error()), NodeErrorTrace::default())]))),
			Err(payload) => {
				let response = ExecutionResponse {
					execution_id,
					result: ExecutionResult::Panic(payload),
					responses: VecDeque::new(),
					dropped_thumbnails: Vec::new(),
					observations: HashMap::new(),
				};
				self.sender.send(NodeGraphUpdate::ExecutionResponse(Box::new(response))).expect("Failed to send response");
				return None;
			}
		};
		Some(result)
	}

	async fn respond(&mut self, execution_id: u64, result: ExecutionResult, observations: HashMap<ObservationKey, InspectResult>) {
		let mut responses = VecDeque::new();

		// While the graph fails to compile, thumbnails stay stale to render once it compiles, not from the last network that did
		let dropped_thumbnails = if self.compile_failed { Vec::new() } else { self.update_thumbnails(&mut responses).await };

		let response = ExecutionResponse {
			execution_id,
			result,
			responses,
			dropped_thumbnails,
			observations,
		};
		self.sender.send(NodeGraphUpdate::ExecutionResponse(Box::new(response))).expect("Failed to send response");
	}

	/// Reads one observation from the last evaluation, apart from the observations the runtime keeps.
	#[cfg(test)]
	pub(crate) fn observe(&self, observation: Observation) -> Option<ObservedValue> {
		let mut observer = Observer::default();
		observer.set((), Some(observation));
		observer.evaluate(&self.executor, self.old_graph.as_ref()).remove(&())?.data
	}

	async fn read_back(&self, texture: Texture) -> Raster<CPU> {
		let application_io = self.editor_api.application_io.as_ref().unwrap();
		let executor = application_io.gpu_executor().expect("GPU executor should be available when we receive a texture");
		Raster::new_gpu(texture).convert(Footprint::BOUNDLESS, executor.clone()).await
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
			Err(CompileError::Graph(errors)) => {
				let message = format!("{errors:?}");
				self.node_graph_errors = errors;
				return Err((ResolvedDocumentNodeTypesDelta::default(), message));
			}
			Err(CompileError::Message(message)) => return Err((ResolvedDocumentNodeTypesDelta::default(), message)),
		};
		self.node_graph_warnings = warnings;

		assert_ne!(proto_network.nodes.len(), 0, "No proto nodes exist?");
		let updated = self.executor.update(proto_network).map(|(output, types)| {
			self.output = output;
			types
		});
		let result = updated.map_err(|(types, e)| {
			self.node_graph_errors.clone_from(&e);
			(types, format!("{e:?}"))
		});
		self.async_runtime().update_sources(self.executor.sources());
		result
	}

	fn resolve_errors(&self, error: &NodeError) -> EvaluationErrors {
		let source_map = self.executor.tree().source_map();
		let paths = |node: NodeId| -> NodePaths {
			let mut paths: Vec<_> = source_map.iter().filter(|(_, (id, _))| *id == node).map(|(path, _)| path.clone()).collect();
			paths.sort_unstable();
			paths.into()
		};
		let mut errors = Vec::new();
		let output = self.output.node();
		let mut pending = vec![(vec![paths(output)], output, error)];
		while let Some((mut trace, node, error)) = pending.pop() {
			let passed = self.executor.trace_error(node, error);
			let origin = passed.last().copied().unwrap_or(node);
			trace.extend(passed.into_iter().map(&paths));
			pending.extend(error.upstream().iter().rev().map(|upstream| (trace.clone(), origin, upstream)));
			errors.push((Arc::clone(error.error()), trace.into()));
		}
		EvaluationErrors(errors.into())
	}

	/// Renders the stale thumbnails, giving back the layers whose thumbnails were dropped because they no longer have content to show.
	async fn update_thumbnails(&mut self, responses: &mut VecDeque<FrontendMessage>) -> Vec<NodeId> {
		let dropped_thumbnails = self.thumbnails.drop_sourceless();
		self.thumbnails.render_stale(&self.executor, &self.editor_api, responses).await;

		dropped_thumbnails
	}
}

impl ExecutionResult {
	fn render_output(&mut self) -> Option<&mut RenderOutput> {
		let (Self::Value(value) | Self::Partial(value) | Self::Fallback(value, _)) = self else {
			return None;
		};
		let TaggedValue::RenderOutput(output) = value else { return None };
		Some(output)
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

impl NodeRuntime {
	pub(crate) fn replace_application_io(&mut self, application_io: PlatformApplicationIo) {
		self.editor_api = PlatformEditorApi {
			application_io: Some(application_io.into()),
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::messages::frontend::utility_types::RenderStatus;
	use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
	use crate::test_utils::EditorTestUtils;
	use glam::{DVec2, UVec2};
	use graph_craft::document::DocumentNode;
	use graphene_std::SourceId;
	use graphene_std::runtime::Runtime;

	const SOURCE: SourceId = SourceId(1);

	async fn editor() -> EditorTestUtils {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		// We use the svg pipeline for these tests since we don't read back the result for editor tests anyway
		editor.handle_message(DocumentMessage::SetRenderMode { render_mode: RenderMode::SvgPreview }).await;
		editor.draw_rect(0., 0., 100., 100.).await;
		editor.runtime.async_runtime().update_sources(&[SOURCE]);
		editor
	}

	/// Sends a network update and a request to evaluate it, as the editor does for a change to the graph.
	fn send_update(editor: &mut EditorTestUtils) {
		send_update_at(editor, UVec2::ONE);
	}

	/// Sends a network update and a request to evaluate it at a viewport of `resolution` pixels.
	fn send_update_at(editor: &mut EditorTestUtils, resolution: UVec2) {
		let portfolio = editor.portfolio_message_handler_mut();
		let document_id = portfolio.active_document_id.unwrap();
		let (executor, documents) = (&mut portfolio.executor, &mut portfolio.documents);
		let document = documents.get_mut(&document_id).unwrap();
		executor
			.submit_node_graph_evaluation(document, document_id, resolution, 1., Default::default(), Vec::new(), true, DVec2::ZERO)
			.unwrap();
	}

	/// Sends a network update and a request to export it, as the editor does for an export.
	fn send_export(editor: &mut EditorTestUtils, file_type: FileType) {
		let portfolio = editor.portfolio_message_handler_mut();
		let document_id = portfolio.active_document_id.unwrap();
		let (executor, documents) = (&mut portfolio.executor, &mut portfolio.documents);
		let document = documents.get_mut(&document_id).unwrap();
		let export_config = ExportConfig {
			name: "export".to_string(),
			file_type,
			scale_factor: 1.,
			bounds: ExportBounds::AllArtwork,
			..Default::default()
		};
		executor.submit_document_export(document, document_id, export_config).unwrap();
	}

	/// Whether the runtime evaluated the graph since this was last called.
	fn evaluated(editor: &mut EditorTestUtils) -> bool {
		editor.dispatch(Message::Wake).iter().any(|message| matches!(message, FrontendMessage::UpdateDocumentArtwork { .. }))
	}

	/// Whether the runtime finished an export since this was last called.
	fn exported(editor: &mut EditorTestUtils) -> bool {
		editor.dispatch(Message::Wake).iter().any(|message| matches!(message, FrontendMessage::TriggerSaveFile { .. }))
	}

	/// Adds a layer tracing an image of noise, which an export has to wait on over several evaluations.
	async fn add_traced_noise_layer(editor: &mut EditorTestUtils) {
		use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
		use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, OutputConnector};
		use graphene_std::raster::Image;
		use graphene_std::raster::color::Color;

		// Pseudo-random grays from an xorshift sequence, so the trace finds many small regions
		let mut noise = Image::new(64, 64, Color::BLACK);
		let mut state = 0x2545_f491_u32;
		for pixel in &mut noise.data {
			state ^= state << 13;
			state ^= state >> 17;
			state ^= state << 5;
			let gray = (state % 256) as f32 / 255.;
			*pixel = Color::from_rgbaf32_unchecked(gray, gray, gray, 1.);
		}
		editor.create_raster_image(noise, None).await;

		// Trace the image between the layer and the nodes feeding it
		let layer = editor.active_document().metadata().all_layers().next().unwrap();
		let content = InputConnector::node_at_index(layer.to_node(), 1);
		let image = editor
			.active_document()
			.network_interface
			.upstream_output_connector(&content, &[])
			.expect("the image layer has content");

		let vectorize = NodeId::new();
		editor.dispatch(NodeGraphMessage::CreateNodeFromContextMenu {
			node_id: Some(vectorize),
			node_type: DefinitionIdentifier::ProtoNode(graphene_std::vectorize_nodes::vectorize::IDENTIFIER),
			xy: None,
			add_transaction: true,
		});
		editor.dispatch(NodeGraphMessage::CreateWire {
			output_connector: image,
			input_connector: InputConnector::node_at_index(vectorize, 0),
		});
		editor.dispatch(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::primary_output(vectorize),
			input_connector: content,
		});
	}

	/// The ID of the viewport request the runtime evaluates, which only changes once it has read a newer one.
	fn last_request(editor: &EditorTestUtils) -> Option<u64> {
		editor.runtime.pending.map(|request| request.execution_id)
	}

	#[tokio::test]
	async fn only_update_reads_the_channel() {
		let mut editor = editor().await;
		send_update(&mut editor);

		editor.runtime.run().await;
		assert!(!evaluated(&mut editor), "run read the request");

		editor.runtime.update();
		editor.runtime.run().await;
		assert!(evaluated(&mut editor));
	}

	/// Asks for a render of the unchanged graph at this device pixel ratio, as a pan or zoom does.
	fn send_render(editor: &mut EditorTestUtils, scale: f64) {
		let portfolio = editor.portfolio_message_handler_mut();
		let document_id = portfolio.active_document_id.unwrap();
		let (executor, documents) = (&mut portfolio.executor, &mut portfolio.documents);
		let document = documents.get_mut(&document_id).unwrap();
		let message = executor
			.submit_current_node_graph_evaluation(document, document_id, UVec2::ONE, scale, Default::default(), DVec2::ZERO)
			.unwrap();
		editor.dispatch(message);
	}

	/// Wires the rectangle into a 'Read Attribute' node's name, which must be a fixed value, so the graph fails to compile.
	/// Gives back the layer's content input and the value that restores it.
	fn break_compile(editor: &mut EditorTestUtils) -> (InputConnector, NodeInput) {
		use crate::messages::portfolio::document::node_graph::document_node_definitions::{DefinitionIdentifier, resolve_proto_node_type};
		use graphene_std::graphic::read_attribute_number;
		use graphene_std::vector::generator_nodes::rectangle;

		let rectangle_reference = DefinitionIdentifier::ProtoNode(rectangle::IDENTIFIER);
		let interface = &editor.active_document().network_interface;
		let rectangle_node = interface
			.document_network()
			.nodes
			.keys()
			.copied()
			.find(|node_id| interface.reference(node_id, &[]) == Some(rectangle_reference.clone()))
			.unwrap();
		let layer_content = InputConnector::node_at_index(editor.active_document().metadata().all_layers().next().unwrap().to_node(), 1);
		let original_content = editor.active_document().network_interface.input_from_connector(&layer_content, &[]).unwrap().clone();

		let read_node = NodeId::new();
		let template = resolve_proto_node_type(read_attribute_number::IDENTIFIER).unwrap().default_node_template();
		let network_interface = &mut editor.active_document_mut().network_interface;
		network_interface.insert_node(read_node, template, &[]);
		network_interface.set_input(&InputConnector::node(read_node, read_attribute_number::NameInput), NodeInput::node(rectangle_node, 0), &[]);
		network_interface.set_input(&layer_content, NodeInput::node(read_node, 0), &[]);

		(layer_content, original_content)
	}

	#[tokio::test]
	async fn a_graph_that_fails_to_compile_keeps_its_error_up_until_it_compiles() {
		use crate::messages::frontend::utility_types::RenderStatus;

		/// Lets the runtime handle what was sent and gives back whether the editor drew artwork and whether it shows the error.
		async fn drawn_and_errored(editor: &mut EditorTestUtils) -> (bool, bool) {
			editor.runtime.update();
			editor.runtime.run().await;
			let messages = editor.dispatch(Message::Wake);
			let drawn = messages.iter().any(|message| matches!(message, FrontendMessage::UpdateDocumentArtwork { .. }));
			let errored = messages.contains(&FrontendMessage::UpdateRenderStatus { status: RenderStatus::Error });
			(drawn, errored)
		}

		let mut editor = editor().await;
		send_update(&mut editor);
		assert_eq!(drawn_and_errored(&mut editor).await, (true, false));

		let (layer_content, original_content) = break_compile(&mut editor);
		send_update(&mut editor);
		assert_eq!(drawn_and_errored(&mut editor).await, (false, true));

		send_render(&mut editor, 1.);
		assert_eq!(
			drawn_and_errored(&mut editor).await,
			(false, true),
			"the last graph that compiled shouldn't be drawn in place of the error"
		);

		editor.active_document_mut().network_interface.set_input(&layer_content, original_content, &[]);
		send_update(&mut editor);
		assert_eq!(drawn_and_errored(&mut editor).await, (true, false));
	}

	#[tokio::test]
	async fn thumbnails_wait_for_a_graph_that_fails_to_compile() {
		/// Lets the runtime handle what was sent and gives back whether the layer's thumbnail is left to render.
		async fn thumbnail_stale(editor: &mut EditorTestUtils, layer: NodeId) -> bool {
			editor.runtime.update();
			editor.runtime.run().await;
			editor.dispatch(Message::Wake);
			editor.runtime.thumbnails.is_stale(layer)
		}

		let mut editor = editor().await;
		let layer = editor.active_document().metadata().all_layers().next().unwrap().to_node();
		editor.runtime.thumbnails.set_in_view(HashSet::from([layer]));
		send_update(&mut editor);
		assert!(!thumbnail_stale(&mut editor, layer).await);

		// A new device pixel ratio makes the thumbnail stale, but the network to render it from has yet to compile
		let (layer_content, original_content) = break_compile(&mut editor);
		send_update(&mut editor);
		thumbnail_stale(&mut editor, layer).await;
		send_render(&mut editor, 2.);
		assert!(thumbnail_stale(&mut editor, layer).await, "the thumbnail shouldn't render from the last network that compiled");

		editor.active_document_mut().network_interface.set_input(&layer_content, original_content, &[]);
		send_update(&mut editor);
		assert!(!thumbnail_stale(&mut editor, layer).await, "the thumbnail should render once the graph compiles");
	}

	#[tokio::test]
	async fn a_resource_that_loads_recompiles_the_unchanged_network() {
		use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_proto_node_type;
		use crate::messages::portfolio::document::utility_types::network_interface::OutputConnector;
		use crate::test_utils::test_prelude::{Color, Image};
		use graph_craft::application_io::resource::{DataSource, ResourceHash, ResourceId};
		use graphene_std::raster_nodes::std_nodes::image;

		/// Handles the message, lets the runtime compile and evaluate whatever it queued, and gives back whether it drew an image.
		async fn image_drawn_after(editor: &mut EditorTestUtils, message: impl Into<Message>) -> bool {
			editor.dispatch(message);
			editor.runtime.update();
			editor.runtime.run().await;
			let messages = editor.dispatch(Message::Wake);
			messages
				.iter()
				.any(|message| matches!(message, FrontendMessage::UpdateImageData { canvas_ids_in_use, .. } if !canvas_ids_in_use.is_empty()))
		}

		let mut editor = editor().await;
		let document_id = editor.portfolio_message_handler().active_document_id.unwrap();
		editor.dispatch(ViewportMessage::Update {
			x: 0.,
			y: 0.,
			width: 100.,
			height: 100.,
			scale: 1.,
		});

		// The layer shows an 'Image' node whose image is known only by where to load it from
		let resource_id = ResourceId::new();
		let source = DataSource::Url("https://example.com/image.png".parse().unwrap());
		editor.active_document_mut().resources.registry.push_source_back(&resource_id, source.clone());
		let image_node = NodeId::new();
		let input = NodeInput::value(TaggedValue::Resource(resource_id), false);
		let template = resolve_proto_node_type(image::IDENTIFIER).unwrap().node_template_input_override([None, Some(input)]);
		editor.active_document_mut().network_interface.insert_node(image_node, template, &[]);
		let layer = editor.active_document().metadata().all_layers().next().unwrap();
		editor.dispatch(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::primary_output(image_node),
			input_connector: InputConnector::node_at_index(layer.to_node(), 1),
		});

		// Failing to load sends the network while the image has no data
		let failed = ResourceMessage::ResolveFailed { resource_id }.into();
		image_drawn_after(&mut editor, PortfolioMessage::DocumentPassMessage { document_id, message: failed }).await;

		let data: Arc<[u8]> = Image::new(2, 2, Color::BLACK).to_png().into();
		let hash = ResourceHash::from(data.as_ref());
		editor.dispatch(ResourceStorageMessage::Store { data });
		let resolved = ResourceMessage::Resolved { resource_id, source, hash }.into();
		assert!(image_drawn_after(&mut editor, PortfolioMessage::DocumentPassMessage { document_id, message: resolved }).await);
	}

	#[tokio::test]
	async fn a_missing_resource_renders_with_an_error_traced_through_its_node() {
		use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_proto_node_type;
		use crate::messages::portfolio::document::utility_types::network_interface::OutputConnector;
		use graph_craft::application_io::resource::ResourceId;
		use graphene_std::platform_application_io::MissingResourceError;
		use graphene_std::raster_nodes::std_nodes::image;

		let mut editor = editor().await;

		// The layer shows an 'Image' node holding an image the document has no record of
		let image_node = NodeId::new();
		let missing_image = NodeInput::value(TaggedValue::Resource(ResourceId::new()), false);
		let template = resolve_proto_node_type(image::IDENTIFIER).unwrap().node_template_input_override([None, Some(missing_image)]);
		editor.active_document_mut().network_interface.insert_node(image_node, template, &[]);
		let layer = editor.active_document().metadata().all_layers().next().unwrap();
		editor.dispatch(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::primary_output(image_node),
			input_connector: InputConnector::node_at_index(layer.to_node(), 1),
		});

		send_update(&mut editor);
		editor.runtime.update();
		editor.runtime.run().await;
		assert!(evaluated(&mut editor));

		let errors = editor.portfolio_message_handler_mut().executor.take_evaluation_errors().unwrap();
		assert!(
			errors
				.iter()
				.any(|(error, trace)| error.is::<MissingResourceError>() && trace.iter().flatten().any(|path| **path == [image_node])),
			"{errors:?}"
		);
	}

	#[tokio::test]
	async fn a_raster_export_shows_the_artwork_whatever_the_render_mode() {
		use crate::test_utils::test_prelude::{Color, Image};

		for render_mode in [RenderMode::Normal, RenderMode::Outline, RenderMode::SvgPreview] {
			let mut editor = EditorTestUtils::create();
			editor.new_document().await;
			editor.handle_message(DocumentMessage::SetRenderMode { render_mode }).await;
			editor.draw_rect(0., 0., 100., 100.).await;

			send_export(&mut editor, FileType::Png);
			editor.runtime.update();
			editor.runtime.run().await;

			let png = editor.dispatch(Message::Wake).into_iter().find_map(|message| match message {
				FrontendMessage::TriggerSaveFile { content, .. } => Some(content),
				_ => None,
			});
			let Some(png) = png else { panic!("the export in {render_mode:?} should save a PNG") };

			let image = Image::from_encoded(&png).expect("the export should be a valid PNG");
			let center = image.data[(image.height / 2 * image.width + image.width / 2) as usize];
			assert_eq!(center, Color::WHITE, "the export in {render_mode:?} should show the rectangle's fill");
		}
	}

	#[tokio::test]
	async fn an_svg_export_draws_effects_svg_filters_cannot_carry_out_as_images_the_gpu_renders() {
		use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
		use graphene_std::blending_nodes::effects::drop_shadow;
		use graphene_std::raster_nodes::adjustments::black_and_white;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(0., 0., 100., 100.).await;

		// Following a shadow, Black & White can't fold into the paints, and no SVG filter mixes the channels as it does, so the rectangle is drawn as an image the GPU renders of it
		let layer = editor.active_document().metadata().all_layers().next().unwrap();
		for node in [drop_shadow::IDENTIFIER, black_and_white::IDENTIFIER] {
			let node_type = DefinitionIdentifier::ProtoNode(node);
			editor.handle_message(NodeGraphMessage::CreateNodeInLayerWithTransaction { node_type, layer }).await;
		}

		// The export waits for the render node to read the image back into the SVG
		send_export(&mut editor, FileType::Svg);
		let mut saved = None;
		for _ in 0..100 {
			editor.runtime.update();
			editor.runtime.run().await;
			saved = editor.dispatch(Message::Wake).into_iter().find_map(|message| match message {
				FrontendMessage::TriggerSaveFile { content, .. } => Some(content),
				_ => None,
			});
			if saved.is_some() {
				break;
			}
		}
		let Some(saved) = saved else { panic!("the export never finished") };

		let svg = String::from_utf8(saved.into_vec()).expect("the export should be SVG text");
		assert!(svg.contains("data:image/png;base64,"), "the image should be embedded: {svg}");
		assert!(!svg.contains("deferred-image"), "the markup standing in for the image should be replaced: {svg}");
	}

	#[tokio::test]
	async fn exporting_a_graph_that_fails_to_compile_says_it_failed() {
		let mut editor = editor().await;
		break_compile(&mut editor);

		send_export(&mut editor, FileType::Png);
		editor.runtime.update();
		editor.runtime.run().await;

		let messages = editor.dispatch(Message::Wake);
		let shows_dialog = messages.iter().any(|message| matches!(message, FrontendMessage::DisplayDialog { .. }));
		assert!(shows_dialog, "the export should show why it saved nothing");
	}

	#[tokio::test]
	async fn an_export_goes_ahead_of_what_was_sent_after_it() {
		let mut editor = editor().await;
		let last = last_request(&editor);
		send_export(&mut editor, FileType::Svg);
		send_update(&mut editor);

		editor.runtime.update();
		assert!(editor.runtime.pending_export.is_some());
		assert_eq!(last_request(&editor), last, "the request sent after the export was read ahead of it");

		editor.runtime.run().await;
		assert!(exported(&mut editor));

		editor.runtime.update();
		assert_ne!(last_request(&editor), last);
		editor.runtime.run().await;
		assert!(evaluated(&mut editor), "the request sent after the export was lost");
	}

	#[tokio::test]
	async fn a_parked_export_holds_back_everything_sent_after_it() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.handle_message(DocumentMessage::SetRenderMode { render_mode: RenderMode::SvgPreview }).await;
		add_traced_noise_layer(&mut editor).await;

		send_export(&mut editor, FileType::Svg);
		editor.runtime.update();
		editor.runtime.run().await;
		assert!(editor.runtime.pending_export.is_some(), "the export took a single evaluation");

		let last = last_request(&editor);
		send_update(&mut editor);
		let mut runs = 0;
		while editor.runtime.pending_export.is_some() {
			editor.runtime.update();
			assert_eq!(last_request(&editor), last, "a request got ahead of the parked export");
			editor.runtime.run().await;
			runs += 1;
			assert!(runs < 100_000, "the export never finished");
		}
		assert!(exported(&mut editor));

		editor.runtime.update();
		assert_ne!(last_request(&editor), last);
		editor.runtime.run().await;
		assert!(evaluated(&mut editor), "the request sent after the export was lost");
	}

	#[tokio::test]
	async fn a_graph_settles_while_network_updates_keep_arriving() {
		let mut editor = editor().await;
		send_update(&mut editor);
		editor.runtime.update();
		editor.runtime.run().await;
		assert!(evaluated(&mut editor));

		let last = last_request(&editor);
		for _ in 0..3 {
			send_update(&mut editor);
			editor.runtime.async_runtime().spawn(SOURCE, Box::pin(async {}));
			editor.runtime.run().await;
			assert!(evaluated(&mut editor), "the finished task didn't evaluate the graph again");
			assert_eq!(last_request(&editor), last, "run read the request");
		}

		editor.runtime.update();
		assert_ne!(last_request(&editor), last);
		assert!(editor.runtime.receiver.try_recv().is_err(), "update left a request in the channel");
		editor.runtime.run().await;
		assert!(evaluated(&mut editor));
	}

	#[tokio::test]
	async fn several_network_updates_evaluate_once() {
		let mut editor = editor().await;
		for _ in 0..3 {
			send_update(&mut editor);
		}

		editor.runtime.update();
		editor.runtime.run().await;
		assert!(evaluated(&mut editor));

		editor.runtime.run().await;
		assert!(!evaluated(&mut editor), "the requests of the older network updates evaluated again");
	}

	/// A trace renders partial frames until it is done.
	#[tokio::test]
	async fn a_trace_renders_partial_frames() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.handle_message(DocumentMessage::SetRenderMode { render_mode: RenderMode::SvgPreview }).await;
		add_traced_noise_layer(&mut editor).await;

		send_update_at(&mut editor, UVec2::splat(512));
		editor.runtime.update();
		editor.runtime.run().await;
		let messages = editor.dispatch(Message::Wake);
		assert!(
			messages.contains(&FrontendMessage::UpdateRenderStatus { status: RenderStatus::Partial }),
			"the trace's frame isn't partial"
		);
	}

	#[test]
	fn a_preview_replaces_the_export_of_the_graph_being_compiled() {
		let previewed_node = NodeId(1);
		let exported_node = NodeId(2);
		let mut network = NodeNetwork {
			exports: vec![NodeInput::node(exported_node, 0)],
			nodes: [(previewed_node, DocumentNode::default()), (exported_node, DocumentNode::default())].into_iter().collect(),
			..Default::default()
		};

		apply_previews(
			&mut network,
			&[(
				Vec::new(),
				RootNode {
					node_id: previewed_node,
					output_index: 0,
				},
			)],
		);

		assert_eq!(network.exports.first().and_then(|export| export.as_node()), Some(previewed_node));
	}
}
