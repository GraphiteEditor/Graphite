use super::*;
use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::network_interface::RootNode;
use crate::messages::portfolio::history::utility_types::DocumentCapture;
use crate::task::{Spawn, Spawner, Task};
use glam::UVec2;
use graph_craft::application_io::resource::ResourceRegistry;
use graph_craft::application_io::{PlatformApplicationIo, PlatformEditorApi};
use graph_craft::document::value::{RenderOutput, RenderOutputType, TaggedValue};
use graph_craft::document::{NodeId, NodeNetwork};
use graph_craft::graphene_compiler::{CompileError, CompiledNetwork, Compiler};
use graph_craft::proto::{GraphErrors, ProtoNetwork};
use graphene_hash::CacheHash;
use graphene_std::application_io::{ApplicationIo, ExportFormat, Texture};
use graphene_std::ops::ConvertAsync;
#[cfg(all(target_family = "wasm", feature = "wasm"))]
use graphene_std::platform_application_io::canvas_utils::{Canvas, CanvasSurface, CanvasSurfaceHandle};
use graphene_std::raster_types::Raster;
use graphene_std::runtime::AsyncRuntime;
use graphene_std::vector::style::RenderMode;
use graphene_std::{Context, NodeError, Poll};
use interpreted_executor::dynamic_executor::{DynamicExecutor, ResolvedDocumentNodeTypesDelta};
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
	receiver: Receiver<GraphRuntimeRequest>,
	sender: Sender<NodeGraphUpdate>,
	editor_wake: Wake,
	dirty: Arc<AtomicBool>,
	pending: Option<ExecutionRequest>,
	pending_export: Option<ExecutionRequest>,
	pending_eyedropper: Option<ExecutionRequest>,
	pending_thumbnails: Option<DocumentId>,
	/// The step the History panel wants the document captured for, once what is pending has run.
	pending_capture: Option<(DocumentId, document_graph_storage::TimeStamp, Option<[DVec2; 2]>)>,
	/// A past state the History panel wants captured, once nothing else is waiting to run.
	pending_past: Option<PastCapture>,
	/// The network the executor last compiled successfully, with previews applied.
	old_graph: Option<NodeNetwork>,
	/// What `old_graph` compiled to, which a past state is compiled alongside.
	compiled: Option<ProtoNetwork>,
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
	viewport_artwork: Option<Texture>,
	/// The overlays to render once the viewport has artwork.
	pending_overlays: Option<OverlayContext>,
	overlays_canvas: VelloCanvas,
	/// The latest rendered overlays.
	overlays_texture: Option<Texture>,
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
	CaptureDocument {
		document_id: DocumentId,
		step: document_graph_storage::TimeStamp,
		/// The area to render close up as well.
		detail: Option<[DVec2; 2]>,
	},
	CapturePast(PastCapture),
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

/// A past state of a document to render like [`GraphRuntimeRequest::CaptureDocument`] renders the current one.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PastCapture {
	pub(super) document_id: DocumentId,
	pub(super) step: document_graph_storage::TimeStamp,
	pub(super) network: NodeNetwork,
	/// Full path from the root network to every layer of the past state.
	pub(super) layers: Vec<Vec<NodeId>>,
	/// The area to render close up as well.
	pub(super) detail: Option<[DVec2; 2]>,
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
			pending_eyedropper: None,
			pending_thumbnails: None,
			pending_capture: None,
			pending_past: None,
			old_graph: None,
			compiled: None,
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
			pending_overlays: None,
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
				GraphRuntimeRequest::GraphUpdate(update) => {
					graph = Some(update);
					// The graph may be another document's, or show a later step than the one asked for
					if let Some((document_id, step, _)) = self.pending_capture.take() {
						self.sender
							.send(NodeGraphUpdate::DocumentCapture { document_id, step, capture: None })
							.expect("Failed to send response");
					}
				}
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
				GraphRuntimeRequest::CaptureDocument { document_id, step, detail } => self.pending_capture = Some((document_id, step, detail)),
				GraphRuntimeRequest::CapturePast(past) => self.pending_past = Some(past),
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
				GraphRuntimeRequest::RenderOverlays(context) => self.pending_overlays = Some(context),
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
		if result.is_ok() {
			self.thumbnails.update_sources(&layers, &network, &thumbnails::compiled_sources(&self.executor));
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
			if self.pending_overlays.is_some() {
				texture = self.composite_viewport_frame();
			}

			if let Some((document_id, step, detail)) = self.pending_capture.take() {
				let capture = match &self.old_graph {
					Some(network) => {
						self.thumbnails
							.capture_document(network, &self.executor, &thumbnails::compiled_sources(&self.executor), &self.editor_api, detail)
							.await
					}
					None => None,
				};
				let capture = capture.map(|mut capture| {
					capture.layer_bounds = Some(self.thumbnails.layer_bounds(&self.executor));
					Box::new(capture)
				});
				self.sender.send(NodeGraphUpdate::DocumentCapture { document_id, step, capture }).expect("Failed to send response");
			}

			if viewport.is_none()
				&& let Some(past) = self.pending_past.take()
			{
				let capture = self.capture_past(&past).await.map(Box::new);
				let (document_id, step) = (past.document_id, past.step);
				self.sender.send(NodeGraphUpdate::DocumentCapture { document_id, step, capture }).expect("Failed to send response");
			}
		}

		(self.editor_wake)();
		texture
	}

	async fn run_viewport(&mut self, request: ExecutionRequest) -> Option<Texture> {
		self.thumbnails.set_scale(request.render_config.scale);

		let mut result = self.evaluate(request)?;

		let texture = if let Some(data) = result.render_output()
			&& let RenderOutputType::Texture(texture) = data
		{
			self.viewport_artwork = Some(texture.clone());
			let frame = self.composite_viewport_frame();

			#[cfg(all(target_family = "wasm", feature = "wasm"))]
			{
				*data = RenderOutputType::CanvasFrame {
					canvas_id: self.wasm_canvas_cache.id(),
					resolution: request.render_config.viewport.resolution.as_dvec2() / request.render_config.scale,
				};
			}

			frame
		} else {
			None
		};

		self.respond(request.execution_id, result).await;
		texture
	}

	/// Draws the latest overlays over the viewport's artwork, presenting the frame on web.
	fn composite_viewport_frame(&mut self) -> Option<Texture> {
		let artwork = self.viewport_artwork.clone()?;
		let Some(executor) = self.editor_api.application_io.as_ref().and_then(|application_io| application_io.gpu_executor()) else {
			log::error!("The GPU executor should be available once the viewport has artwork");
			return None;
		};

		if let Some(overlays) = self.pending_overlays.take() {
			self.overlays_texture = executor
				.render_vello_scene_on_canvas(&mut self.overlays_canvas, overlays.take_scene())
				.inspect_err(|error| log::error!("Failed to render the overlays: {error:?}"))
				.ok();
		}

		let frame = match &self.overlays_texture {
			Some(overlays) => {
				let (width, height) = (artwork.width(), artwork.height());
				let max_texture_size = executor.context().device.limits().max_texture_dimension_2d;
				let overlays_to_artwork = kurbo::Affine::scale_non_uniform(width as f64 / overlays.width() as f64, height as f64 / overlays.height() as f64);

				let mut scene = VelloSceneBuilder::new(width as u16, height as u16, kurbo::Affine::IDENTITY, max_texture_size);
				scene.draw_texture(artwork.clone(), ImageSampler::new(), kurbo::Affine::IDENTITY);
				scene.draw_texture(overlays.clone(), ImageSampler::new(), overlays_to_artwork);

				executor
					.render_vello_scene_on_canvas(&mut self.overlays_canvas, scene)
					.inspect_err(|error| log::error!("Failed to draw the overlays over the artwork: {error:?}"))
					.unwrap_or(artwork)
			}
			None => artwork,
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

		if let Some(data) = result.render_output()
			&& let RenderOutputType::Texture(texture) = data
		{
			let (buffer, width, height) = self.read_back(texture.clone()).await.to_flat_u8();
			*data = RenderOutputType::Buffer { data: buffer, width, height };
		}
		self.respond(request.execution_id, result).await;
	}

	async fn run_eyedropper(&mut self, mut request: ExecutionRequest) {
		// Eyedropper should use the same time and pointer to not invalidate the cache
		if let Some(viewport) = self.pending {
			request.render_config.time = viewport.render_config.time;
			request.render_config.pointer = viewport.render_config.pointer;
		}

		if let Some(mut result) = self.evaluate(request)
			&& let Some(RenderOutputType::Texture(texture)) = result.render_output()
		{
			let raster = self.read_back(texture.clone()).await;
			self.sender.send(NodeGraphUpdate::EyedropperPreview(raster)).expect("Failed to send response");
		}
	}

	fn evaluate(&mut self, request: ExecutionRequest) -> Option<ExecutionResult> {
		let ExecutionRequest { execution_id, mut render_config } = request;

		// Render via the SVG pipeline even though raster was requested if SVG Preview render mode is active or there's no GPU
		if render_config.export_format == ExportFormat::Raster && (render_config.render_mode == RenderMode::SvgPreview || self.editor_api.application_io.as_ref().unwrap().gpu_executor().is_none()) {
			render_config.export_format = ExportFormat::Svg;
		}

		let context = render_config.into_context_with_generations(self.async_runtime().snapshot());
		let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.executor.tree().eval_tagged_value(self.executor.output(), &context))) {
			Ok(Ok(Poll::Pending)) => ExecutionResult::Pending,
			Ok(Ok(Poll::Partial(_))) if render_config.for_export => ExecutionResult::Pending,
			Ok(Ok(Poll::Final(value) | Poll::Partial(value))) => ExecutionResult::Value(value),
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

	async fn respond(&mut self, execution_id: u64, result: ExecutionResult) {
		let mut responses = VecDeque::new();
		let dropped_thumbnails = self.update_thumbnails(&mut responses).await;
		let observations = self.observer.evaluate(&self.executor, self.old_graph.as_ref());

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

	/// Compiles a document network to the proto network the executor runs.
	fn compile(&mut self, graph: NodeNetwork) -> Result<CompiledNetwork, CompileError> {
		let mut scoped_network = wrap_network_in_scope(graph, self.editor_api.clone(), self.async_runtime().handle());

		if let Err(e) = self.preprocessor.preprocess(&mut scoped_network, &|resource_id| self.resources.hash(&resource_id)) {
			return Err(CompileError::Message(e.to_string()));
		}

		// We assume only one output
		assert_eq!(scoped_network.exports.len(), 1, "Graph with multiple outputs not yet handled");

		Compiler {}.compile_single(scoped_network)
	}

	fn update_network(&mut self, graph: NodeNetwork) -> Result<ResolvedDocumentNodeTypesDelta, (ResolvedDocumentNodeTypesDelta, String)> {
		let CompiledNetwork { network: proto_network, warnings } = match self.compile(graph) {
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
		let compiled = proto_network.clone();
		let result = self.executor.update(proto_network).map_err(|(types, e)| {
			self.node_graph_errors.clone_from(&e);
			(types, format!("{e:?}"))
		});
		self.compiled = result.is_ok().then_some(compiled);
		self.async_runtime().update_sources(self.executor.sources());
		result
	}

	/// Renders a past state of the document, compiled alongside the current graph as a second output. A proto node's ID
	/// hashes everything upstream of it, so what the two states share is one node, which keeps its cache. The current
	/// nodes go in last, so the executor still names them for their document paths, and the past state's own nodes are
	/// freed once a later graph update leaves them out.
	async fn capture_past(&mut self, past: &PastCapture) -> Option<DocumentCapture> {
		let current = self.compiled.clone()?;
		let CompiledNetwork { network: past_network, .. } = self
			.compile(past.network.clone())
			.inspect_err(|error| log::warn!("The History panel's past state didn't compile: {error:?}"))
			.ok()?;

		let past_output = past_network.output;
		let past_sources: HashMap<Vec<NodeId>, NodeId> = past_network.nodes.iter().filter_map(|(id, node)| Some((node.original_location.path.clone()?, *id))).collect();
		let current_ids: HashSet<NodeId> = current.nodes.iter().map(|(id, _)| *id).collect();
		let mut merged = current.clone();
		merged.nodes = past_network.nodes.into_iter().filter(|(id, _)| !current_ids.contains(id)).chain(current.nodes).collect();
		if let Err((_, error)) = self.executor.update(merged) {
			log::warn!("The History panel's past state didn't compile alongside the document: {error:?}");
			return None;
		}
		self.async_runtime().update_sources(self.executor.sources());

		// Thumbnail content evaluates from the context it last ran in, so the past state runs once as the viewport does
		let mut render_config = self.pending?.render_config;
		if render_config.export_format == ExportFormat::Raster && self.editor_api.application_io.as_ref().unwrap().gpu_executor().is_none() {
			render_config.export_format = ExportFormat::Svg;
		}
		let context = render_config.into_context_with_generations(self.async_runtime().snapshot());
		let evaluated = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.executor.tree().eval_tagged_value(past_output, &context)));
		if !matches!(evaluated, Ok(Ok(_))) {
			return None;
		}

		let source_of = |path: &[NodeId]| past_sources.get(path).copied();
		let mut thumbnails = Thumbnails::new();
		thumbnails.set_scale(self.thumbnails.scale());
		thumbnails.update_sources(&past.layers, &past.network, &source_of);
		let mut capture = thumbnails.capture_document(&past.network, &self.executor, &source_of, &self.editor_api, past.detail).await?;
		capture.layer_bounds = Some(thumbnails.layer_bounds(&self.executor));
		Some(capture)
	}

	fn resolve_errors(&self, error: &NodeError) -> EvaluationErrors {
		let source_map = self.executor.tree().source_map();
		let paths = |node: NodeId| -> NodePaths {
			let mut paths: Vec<_> = source_map.iter().filter(|(_, (id, _))| *id == node).map(|(path, _)| path.clone()).collect();
			paths.sort_unstable();
			paths.into()
		};
		let mut errors = Vec::new();
		let output = self.executor.output();
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
	fn render_output(&mut self) -> Option<&mut RenderOutputType> {
		let (Self::Value(value) | Self::Fallback(value, _)) = self else { return None };
		let TaggedValue::RenderOutput(RenderOutput { data, .. }) = value else { return None };
		Some(data)
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
	use crate::test_utils::EditorTestUtils;
	use glam::DVec2;
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
		let portfolio = editor.portfolio_message_handler_mut();
		let document_id = portfolio.active_document_id.unwrap();
		let (executor, documents) = (&mut portfolio.executor, &mut portfolio.documents);
		let document = documents.get_mut(&document_id).unwrap();
		executor
			.submit_node_graph_evaluation(document, document_id, UVec2::ONE, 1., Default::default(), Vec::new(), true, DVec2::ZERO)
			.unwrap();
	}

	/// Sends a network update and a request to export it, as the editor does for an export.
	fn send_export(editor: &mut EditorTestUtils) {
		let portfolio = editor.portfolio_message_handler_mut();
		let document_id = portfolio.active_document_id.unwrap();
		let (executor, documents) = (&mut portfolio.executor, &mut portfolio.documents);
		let document = documents.get_mut(&document_id).unwrap();
		let export_config = ExportConfig {
			name: "export".to_string(),
			file_type: FileType::Svg,
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

	/// Fills the document's layer with a trace of noise, which an export has to wait on over several evaluations.
	fn vectorize_layer(editor: &mut EditorTestUtils) {
		use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
		use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, OutputConnector};

		let layer = editor.active_document().metadata().all_layers().next().unwrap();
		let mut create = |node_type| {
			let node_id = NodeId::new();
			editor.dispatch(NodeGraphMessage::CreateNodeFromContextMenu {
				node_id: Some(node_id),
				node_type,
				xy: None,
				add_transaction: true,
			});
			node_id
		};
		let noise = create(DefinitionIdentifier::ProtoNode(graphene_std::raster_nodes::std_nodes::noise_pattern::IDENTIFIER));
		let vectorize = create(DefinitionIdentifier::ProtoNode(graphene_std::vectorize_nodes::vectorize::IDENTIFIER));
		editor.dispatch(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::primary_output(noise),
			input_connector: InputConnector::node_at_index(vectorize, 0),
		});
		editor.dispatch(NodeGraphMessage::CreateWire {
			output_connector: OutputConnector::primary_output(vectorize),
			input_connector: InputConnector::node_at_index(layer.to_node(), 1),
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

	#[tokio::test]
	async fn an_export_goes_ahead_of_what_was_sent_after_it() {
		let mut editor = editor().await;
		let last = last_request(&editor);
		send_export(&mut editor);
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
		editor.draw_rect(0., 0., 512., 512.).await;
		vectorize_layer(&mut editor);

		send_export(&mut editor);
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
