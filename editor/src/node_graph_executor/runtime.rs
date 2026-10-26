use super::*;
use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::portfolio::document::utility_types::network_interface::RootNode;
use crate::task::{Scheduling, Spawn, Spawner, Task};
use glam::{DVec2, UVec2};
use graph_craft::application_io::resource::ResourceRegistry;
use graph_craft::application_io::{PlatformApplicationIo, PlatformEditorApi};
use graph_craft::concrete;
use graph_craft::document::value::{RenderOutput, RenderOutputType, TaggedValue};
use graph_craft::document::{NodeId, NodeNetwork};
use graph_craft::graphene_compiler::{CompileError, CompiledNetwork, Compiler};
use graph_craft::proto::GraphErrors;
use graphene_hash::CacheHash;
use graphene_std::application_io::{ApplicationIo, ExportFormat, RenderConfig, Texture};
use graphene_std::list::{Item, List};
use graphene_std::memo::IORecord;
use graphene_std::ops::ConvertAsync;
#[cfg(all(target_family = "wasm", feature = "gpu", feature = "wasm"))]
use graphene_std::platform_application_io::canvas_utils::{Canvas, CanvasSurface, CanvasSurfaceHandle};
use graphene_std::raster_types::Raster;
use graphene_std::runtime::{AsyncRuntime, Clock};
use graphene_std::vector::style::RenderMode;
use graphene_std::vector::{Vector, VectorGeometry, paths_from_list_row_attributes};
use graphene_std::{Context, ContextFeatures, ContextOverrides, NodeError, OwnedContextImpl, Poll};
use interpreted_executor::dynamic_executor::{DynamicExecutor, ResolvedDocumentNodeTypesDelta};
use interpreted_executor::util::wrap_network_in_scope;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;
use thumbnails::Thumbnails;

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
	/// A re-evaluation waiting for the next frame, or for the end of an export.
	rerun: bool,
	pending: Option<ExecutionRequest>,
	pending_export: Option<ExecutionRequest>,
	/// The network the executor last compiled successfully, with previews applied.
	old_graph: Option<NodeNetwork>,
	async_runtime: Option<AsyncRuntime>,
	scheduling: Scheduling,
	frame_start: Duration,

	editor_api: Arc<PlatformEditorApi>,
	resources: ResourceRegistry,
	node_graph_errors: GraphErrors,
	node_graph_warnings: GraphErrors,
	monitor_nodes: Vec<(Vec<NodeId>, NodeId)>,
	thumbnails: Thumbnails,

	/// The node the Data panel inspects.
	inspect_state: Option<InspectState>,

	preprocessor: preprocessor::Preprocessor,

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
	/// The node each network renders in place of its export, paired with that network's path.
	pub(super) previewed: Vec<(Vec<NodeId>, RootNode)>,
	/// Full path from the root network to every layer, which are the nodes with thumbnails.
	pub(super) layers: Vec<Vec<NodeId>>,
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
			rerun: false,
			pending: None,
			pending_export: None,
			old_graph: None,
			resources: ResourceRegistry::default(),
			async_runtime: None,
			scheduling: Scheduling::default(),
			frame_start: Duration::ZERO,

			editor_api: PlatformEditorApi {
				#[cfg(not(test))]
				application_io: None,

				#[cfg(test)]
				application_io: Some(PlatformApplicationIo::default().into()),
			}
			.into(),

			node_graph_errors: Vec::new(),
			node_graph_warnings: Vec::new(),
			monitor_nodes: Vec::new(),
			thumbnails: Thumbnails::new(),

			preprocessor: preprocessor::Preprocessor::new(),

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
			self.thumbnails.set_in_view(layers);
			if execution.is_none() && graph.is_none() {
				let mut responses = VecDeque::new();
				self.thumbnails.render_stale(&self.executor, &self.editor_api, &mut responses).await;
				if !responses.is_empty() {
					self.sender.send(NodeGraphUpdate::Thumbnails { responses, document_id }).expect("Failed to send response");
				}
			}
		}

		let rerun = std::mem::take(&mut self.rerun) | self.async_runtime().take_bumped();
		if rerun && execution.is_none() {
			if self.frame_has_budget() {
				execution = self.pending_export.or(self.pending).map(GraphRuntimeRequest::ExecutionRequest);
			} else {
				self.rerun = true;
			}
		}

		// Eydropper should use the same time and pointer to not invalidate the cache
		if let Some(GraphRuntimeRequest::ExecutionRequest(eyedropper)) = &mut eyedropper
			&& let Some(GraphRuntimeRequest::ExecutionRequest(execution)) = &execution
		{
			eyedropper.render_config.time = execution.render_config.time;
			eyedropper.render_config.pointer = execution.render_config.pointer;
		}

		let requests = [graph, eyedropper, execution].into_iter().flatten();

		for request in requests {
			match request {
				GraphRuntimeRequest::ThumbnailsInView { .. } | GraphRuntimeRequest::Inspect(_) | GraphRuntimeRequest::InspectContextOverrides(_) => {}
				GraphRuntimeRequest::GraphUpdate(GraphUpdate {
					mut network,
					resources,
					node_to_inspect,
					previewed,
					layers,
				}) => {
					apply_previews(&mut network, &previewed);

					match &mut self.inspect_state {
						Some(inspect_state) if inspect_state.inspect_node_path == node_to_inspect => inspect_state.last_evaluation = None,
						_ => self.inspect_state = InspectState::new(node_to_inspect),
					}

					self.resources = resources;

					self.node_graph_errors.clear();
					self.node_graph_warnings.clear();
					let result = self.update_network(network.clone());
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
				GraphRuntimeRequest::ExecutionRequest(request @ ExecutionRequest { execution_id, mut render_config, .. }) => {
					if !render_config.for_export && !render_config.for_eyedropper {
						self.thumbnails.set_scale(render_config.scale);
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
						Ok(Err(error)) => ExecutionResult::Error(EvaluationErrors(Box::new([(Arc::clone(NodeError::new(error).error()), NodeErrorTrace::default())]))),
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
						(true, _) => self.rerun = true,
						_ => {}
					}

					let mut responses = VecDeque::new();
					let dropped_thumbnails = if !render_config.for_eyedropper {
						self.process_monitor_nodes(&mut responses).await
					} else {
						Vec::new()
					};

					let inspect_result = self.inspect_state.as_mut().map(|state| state.access(&self.executor, self.old_graph.as_ref()));

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
			let inspect_result = inspect_state.access(&self.executor, self.old_graph.as_ref());
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
			Err(CompileError::Graph(errors)) => {
				let message = format!("{errors:?}");
				self.node_graph_errors = errors;
				return Err((ResolvedDocumentNodeTypesDelta::default(), message));
			}
			Err(CompileError::Message(message)) => return Err((ResolvedDocumentNodeTypesDelta::default(), message)),
		};
		self.node_graph_warnings = warnings;
		self.monitor_nodes = proto_network
			.nodes
			.iter()
			.filter(|(_, node)| node.identifier == graphene_std::memo::monitor::IDENTIFIER)
			.map(|(id, node)| (node.original_location.path.clone().unwrap_or_default(), *id))
			.collect();

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

	/// Updates state data, giving back the layers whose thumbnails were dropped because they no longer have content to show.
	async fn process_monitor_nodes(&mut self, responses: &mut VecDeque<FrontendMessage>) -> Vec<NodeId> {
		let dropped_thumbnails = self.thumbnails.drop_sourceless();

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

		self.thumbnails.render_stale(&self.executor, &self.editor_api, responses).await;

		dropped_thumbnails
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

impl NodeRuntime {
	pub(crate) fn replace_application_io(&mut self, application_io: PlatformApplicationIo) {
		self.editor_api = PlatformEditorApi {
			application_io: Some(application_io.into()),
		}
		.into();
	}

	pub(crate) fn replace_task_executor(&mut self, spawner: Spawner, wake: Wake, clock: Arc<dyn Clock>) {
		self.async_runtime = Some(AsyncRuntime::new(Arc::new(NodeTaskExecutor { spawner, wake }), clock));
		self.begin_frame();
	}

	pub fn set_scheduling(&mut self, scheduling: Scheduling) {
		self.scheduling = scheduling;
	}

	/// Called by the host once the previous output was presented.
	pub fn begin_frame(&mut self) {
		self.frame_start = self.async_runtime().clock().now();
	}

	/// How long a deferred re-evaluation waits for the next frame.
	pub fn until_next_frame(&self) -> Option<Duration> {
		let budget = self.frame_budget()?;
		let elapsed = self.frame_elapsed().as_nanos() % budget.as_nanos();
		self.rerun.then(|| budget - Duration::from_nanos(elapsed as u64))
	}

	fn frame_elapsed(&self) -> Duration {
		self.async_runtime().clock().now().saturating_sub(self.frame_start)
	}

	fn frame_budget(&self) -> Option<Duration> {
		self.scheduling.frame_budget.filter(|budget| !budget.is_zero())
	}

	fn frame_has_budget(&self) -> bool {
		self.frame_budget().is_none_or(|budget| self.frame_elapsed() < budget)
	}

	pub(crate) fn async_runtime(&self) -> &AsyncRuntime {
		self.async_runtime.as_ref().expect("the task executor is installed before the first graph update")
	}
}

struct NodeTaskExecutor {
	spawner: Spawner,
	wake: Wake,
}

impl Spawn for NodeTaskExecutor {
	fn spawn(&self, task: Task) {
		let wake = self.wake.clone();
		self.spawner.spawn(Box::pin(async move {
			task.await;
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

	fn access(&mut self, executor: &DynamicExecutor, network: Option<&NodeNetwork>) -> InspectResult {
		let node = network.and_then(|network| executor.tree().output_node(network, &self.inspect_node_path));
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::test_utils::{EditorTestUtils, FakeClock};
	use graph_craft::document::DocumentNode;
	use graphene_std::SourceId;
	use graphene_std::runtime::Runtime;

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

	const SOURCE: SourceId = SourceId(1);

	async fn editor(clock: &Arc<FakeClock>) -> EditorTestUtils {
		let mut editor = EditorTestUtils::create_with(Scheduling::default(), clock.clone());
		editor.new_document().await;
		// SVG Preview renders through the SVG pipeline, which sends each evaluation's artwork to the frontend where it can be seen
		editor.handle_message(DocumentMessage::SetRenderMode { render_mode: RenderMode::SvgPreview }).await;
		editor.draw_rect(0., 0., 100., 100.).await;
		editor.runtime.async_runtime().retain_sources(&[SOURCE]);
		editor.runtime.begin_frame();
		editor
	}

	fn finish_task(editor: &EditorTestUtils) {
		editor.runtime.async_runtime().spawn(SOURCE, Box::pin(async {}));
	}

	async fn evaluates(editor: &mut EditorTestUtils) -> bool {
		editor.runtime.run().await;
		let messages = editor.editor.handle_message(Message::Wake);
		messages.iter().any(|message| matches!(message, FrontendMessage::UpdateDocumentArtwork { .. }))
	}

	#[tokio::test]
	async fn reevaluations_past_the_frame_budget_wait_for_the_next_frame() {
		let clock = Arc::new(FakeClock::default());
		let mut editor = editor(&clock).await;

		finish_task(&editor);
		finish_task(&editor);
		assert!(evaluates(&mut editor).await);
		assert!(!evaluates(&mut editor).await, "two finished tasks re-evaluated twice");

		clock.advance(Duration::from_millis(20));
		finish_task(&editor);
		assert!(!evaluates(&mut editor).await);
		assert_eq!(editor.runtime.until_next_frame(), Some(Duration::from_millis(12)));

		editor.runtime.begin_frame();
		assert!(evaluates(&mut editor).await);
		assert_eq!(editor.runtime.until_next_frame(), None);
	}

	#[tokio::test]
	async fn explicit_requests_ignore_the_frame_budget() {
		let clock = Arc::new(FakeClock::default());
		let mut editor = editor(&clock).await;

		clock.advance(Duration::from_millis(100));
		finish_task(&editor);
		let portfolio = &mut editor.editor.dispatcher.message_handlers.portfolio_message_handler;
		let document_id = portfolio.active_document_id.unwrap();
		let document = portfolio.documents.get_mut(&document_id).unwrap();
		portfolio
			.executor
			.submit_current_node_graph_evaluation(document, document_id, UVec2::ONE, 1., Default::default(), DVec2::ZERO)
			.unwrap();

		assert!(evaluates(&mut editor).await);
		assert!(!evaluates(&mut editor).await, "the finished task re-evaluated after the explicit request");
	}
}
