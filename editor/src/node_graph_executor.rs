use crate::messages::frontend::utility_types::{ExportBounds, FileType, RasterizedImage, RenderStatus};
use crate::messages::portfolio::document::node_graph::utility_types::NodeEvaluationError;
use crate::messages::portfolio::document::utility_types::layers_in_view::LayersInView;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::*;
use glam::{DAffine2, DVec2, UVec2};
use graph_craft::application_io::EditorPreferences;
use graph_craft::document::value::{RenderOutput, RenderOutputType, TaggedValue};
use graph_craft::document::{DocumentNodeImplementation, NodeId, NodeInput, NodeNetwork};
use graph_craft::proto::GraphErrors;
use graphene_std::application_io::{ExportFormat, RenderConfig, TimingInformation};
use graphene_std::bounds::RenderBoundingBox;
use graphene_std::color::SRGBA8;
use graphene_std::list::List;
use graphene_std::raster::{CPU, Raster};
use graphene_std::renderer::{RenderMetadata, graphic_list_bounding_box};
use graphene_std::transform::Footprint;
use graphene_std::vector::{Vector, VectorGeometry, graphic_types, no_paths, paths_from_list_row_attributes};
use graphene_std::{ATTR_TRANSFORM, Graphic};
use interpreted_executor::dynamic_executor::ResolvedDocumentNodeTypesDelta;
use std::any::Any;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

#[cfg(test)]
mod recompile_tests;
mod runtime;
pub use runtime::*;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ExecutionRequest {
	execution_id: u64,
	render_config: RenderConfig,
}

pub struct ExecutionResponse {
	execution_id: u64,
	result: ExecutionResult,
	responses: VecDeque<FrontendMessage>,
	/// The layers whose thumbnails the runtime let go of, which it renders anew if they're monitored again.
	dropped_thumbnails: Vec<NodeId>,
	vector_modify: HashMap<NodeId, VectorGeometry>,
	/// The resulting value from the temporary inspected during execution
	inspect_result: Option<InspectResult>,
}

#[derive(Debug)]
pub enum ExecutionResult {
	Pending,
	Value(TaggedValue),
	Fallback(TaggedValue, EvaluationError),
	Error(EvaluationError),
	Panic(Box<dyn Any + Send>),
}

pub type EvaluationError = Box<[(Arc<dyn std::error::Error + Send + Sync>, Box<[NodeId]>)]>;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CompilationResponse {
	result: Result<ResolvedDocumentNodeTypesDelta, (ResolvedDocumentNodeTypesDelta, String)>,
	node_graph_errors: GraphErrors,
	node_graph_warnings: GraphErrors,
}

pub enum NodeGraphUpdate {
	ExecutionResponse(Box<ExecutionResponse>),
	CompilationResponse(CompilationResponse),
	EyedropperPreview(Raster<CPU>),
	Thumbnails { responses: VecDeque<FrontendMessage>, document_id: DocumentId },
	InspectResult(Box<InspectResult>),
}

#[derive(Debug)]
pub struct NodeGraphExecutor {
	sender: Sender<GraphRuntimeRequest>,
	receiver: Receiver<NodeGraphUpdate>,
	current_execution_id: u64,
	futures: VecDeque<(u64, ExecutionContext)>,
	pending: Option<(u64, ExecutionContext)>,
	node_graph_hash: u64,
	/// The document whose network the graph runtime was last sent.
	node_graph_document: Option<DocumentId>,
	/// Full path from the root document network to the node currently being inspected by the Data panel, or empty if nothing is selected.
	previous_node_to_inspect: Vec<NodeId>,
	// TODO: Eventually remove this document upgrade code
	/// In-progress one-time pre-pass that converts legacy bounding-box-relative gradients to absolute space, if any.
	gradient_migration: Option<GradientMigration>,
	// TODO: Eventually remove this document upgrade code
	/// Documents whose gradient migration pass already ran this session, so entries that failed to measure
	/// (kept pending for a retry on the next open) don't re-run the pass on every render request.
	gradient_migration_attempted: HashSet<DocumentId>,
}

#[derive(Debug, Clone)]
struct ExecutionContext {
	export_config: Option<ExportConfig>,
	document_id: DocumentId,
	/// The view the render was asked for, which the footprints it reports back are measured in.
	document_to_viewport: DAffine2,
	// TODO: Eventually remove this document upgrade code
	/// Set when this execution is a gradient-migration measurement run, carrying the "Fill" node (addressed by its enclosing
	/// network path) and its original relative gradient. The evaluated geometry is read back from the inspect result to size the
	/// gradient; such runs never touch the visible artwork. Carrying the entry keeps a stale re-dispatched response paired with the fill it measured.
	measure_fill: Option<(Vec<NodeId>, NodeId, graphic_types::migrations::legacy::LegacyGradient)>,
}

// TODO: Eventually remove this document upgrade code
/// State for the deferred legacy-gradient migration: a queue of "Fill" nodes (each addressed by its enclosing network path)
/// whose decomposed gradient still needs its transform baked, each paired with its original relative gradient and measured
/// one at a time by redirecting the document export so even hidden/orphaned/nested branches evaluate.
#[derive(Debug, Clone)]
struct GradientMigration {
	document_id: DocumentId,
	remaining: VecDeque<(Vec<NodeId>, NodeId, graphic_types::migrations::legacy::LegacyGradient)>,
	resolution: UVec2,
	scale: f64,
}

impl NodeGraphExecutor {
	pub(crate) fn new(editor_wake: Wake) -> (Self, NodeRuntime) {
		let (request_sender, request_receiver) = std::sync::mpsc::channel();
		let (response_sender, response_receiver) = std::sync::mpsc::channel();
		let node_runtime = NodeRuntime::new(request_receiver, response_sender, editor_wake);

		let node_executor = Self {
			futures: Default::default(),
			sender: request_sender,
			receiver: response_receiver,
			node_graph_hash: 0,
			node_graph_document: None,
			current_execution_id: 0,
			previous_node_to_inspect: Vec::new(),
			gradient_migration: None,
			gradient_migration_attempted: HashSet::new(),
			pending: None,
		};
		(node_executor, node_runtime)
	}

	/// Allows the test runner to see if any more executions have been queued.
	#[cfg(test)]
	pub fn current_execution_id(&self) -> u64 {
		self.current_execution_id
	}

	/// Execute the network by flattening it and creating a borrow stack.
	fn queue_execution(&mut self, render_config: RenderConfig) -> u64 {
		let execution_id = self.current_execution_id;
		self.current_execution_id += 1;
		let request = ExecutionRequest { execution_id, render_config };
		self.sender.send(GraphRuntimeRequest::ExecutionRequest(request)).expect("Failed to send generation request");

		execution_id
	}

	pub fn submit_inspect_context_overrides(&self, overrides: graphene_std::ContextOverrides) {
		self.sender.send(GraphRuntimeRequest::InspectContextOverrides(overrides)).expect("Failed to send the context overrides");
	}

	pub fn submit_thumbnails_in_view(&self, document: &DocumentMessageHandler, document_id: DocumentId) {
		// The runtime renders thumbnails from the network it has, which may still be another document's
		if self.node_graph_document != Some(document_id) {
			return;
		}

		let layers = document.layers_in_view.all_in_view();
		self.sender
			.send(GraphRuntimeRequest::ThumbnailsInView { layers, document_id })
			.expect("Failed to send the thumbnails in view");
	}

	pub fn update_editor_preferences(&self, editor_preferences: EditorPreferences) {
		self.sender
			.send(GraphRuntimeRequest::EditorPreferencesUpdate(editor_preferences))
			.expect("Failed to send editor preferences");
	}

	/// Updates the network to monitor all inputs. Useful for the testing.
	#[cfg(test)]
	pub(crate) fn update_node_graph_instrumented(&mut self, document: &mut DocumentMessageHandler) -> Result<Instrumented, String> {
		// We should always invalidate the cache.
		self.node_graph_hash = crate::application::generate_uuid();
		let mut network = document.network_interface.document_network().clone();
		let instrumented = Instrumented::new(&mut network);

		let resources = document.resources.registry.clone();

		self.sender
			.send(GraphRuntimeRequest::GraphUpdate(GraphUpdate {
				network,
				resources,
				node_to_inspect: Vec::new(),
			}))
			.map_err(|e| e.to_string())?;
		Ok(instrumented)
	}

	/// Update the cached network if necessary.
	fn update_node_graph(&mut self, document: &mut DocumentMessageHandler, document_id: DocumentId, node_to_inspect: Vec<NodeId>, ignore_hash: bool) -> Result<(), String> {
		let network_hash = document.network_interface.network_hash();
		if network_hash != self.node_graph_hash || self.node_graph_document != Some(document_id) || ignore_hash {
			let network = document.network_interface.document_network().clone();
			self.previous_node_to_inspect.clone_from(&node_to_inspect);
			self.node_graph_hash = network_hash;
			self.node_graph_document = Some(document_id);

			let resources = document.resources.registry.clone();

			self.sender
				.send(GraphRuntimeRequest::GraphUpdate(GraphUpdate { network, resources, node_to_inspect }))
				.map_err(|e| e.to_string())?;
		} else if self.previous_node_to_inspect != node_to_inspect {
			self.previous_node_to_inspect.clone_from(&node_to_inspect);
			self.sender.send(GraphRuntimeRequest::Inspect(node_to_inspect)).map_err(|e| e.to_string())?;
		}

		Ok(())
	}

	/// Adds an evaluate request for whatever current network is cached.
	pub(crate) fn submit_current_node_graph_evaluation(
		&mut self,
		document: &mut DocumentMessageHandler,
		document_id: DocumentId,
		viewport_resolution: UVec2,
		viewport_scale: f64,
		time: TimingInformation,
		pointer: DVec2,
	) -> Result<Message, String> {
		let viewport = Footprint {
			transform: document.metadata().document_to_viewport,
			resolution: viewport_resolution,
			..Default::default()
		};
		let render_config = RenderConfig {
			viewport,
			scale: viewport_scale,
			time,
			pointer,
			export_format: graphene_std::application_io::ExportFormat::Raster,
			render_mode: document.render_mode,
			for_export: false,
			for_eyedropper: false,
		};

		// Execute the node graph
		let execution_id = self.queue_execution(render_config);

		self.futures.push_back((
			execution_id,
			ExecutionContext {
				export_config: None,
				document_id,
				document_to_viewport: viewport.transform,
				measure_fill: None,
			},
		));

		Ok(DeferMessage::SetGraphSubmissionIndex { execution_id }.into())
	}

	/// Evaluates a node graph, computing the entire graph
	#[allow(clippy::too_many_arguments)]
	pub fn submit_node_graph_evaluation(
		&mut self,
		document: &mut DocumentMessageHandler,
		document_id: DocumentId,
		viewport_resolution: UVec2,
		viewport_scale: f64,
		time: TimingInformation,
		node_to_inspect: Vec<NodeId>,
		ignore_hash: bool,
		pointer: DVec2,
	) -> Result<Message, String> {
		self.update_node_graph(document, document_id, node_to_inspect, ignore_hash)?;
		self.submit_current_node_graph_evaluation(document, document_id, viewport_resolution, viewport_scale, time, pointer)
	}

	#[allow(clippy::too_many_arguments)]
	pub(crate) fn submit_eyedropper_preview(
		&mut self,
		document: &DocumentMessageHandler,
		document_id: DocumentId,
		transform: DAffine2,
		pointer: DVec2,
		viewport_resolution: UVec2,
		viewport_scale: f64,
		time: TimingInformation,
	) -> Result<Message, String> {
		let viewport = Footprint {
			transform,
			resolution: viewport_resolution,
			..Default::default()
		};

		// TODO: On desktop, SVG Preview mode cannot work with the Eyedropper tool until <https://github.com/GraphiteEditor/Graphite/issues/3796> is implemented.
		// TODO: So for now, we fall back to the Eyedropper using Normal mode (Vello) rendering, which looks similar enough to SVG Preview.
		#[cfg(not(target_family = "wasm"))]
		let render_mode = match document.render_mode {
			graphene_std::vector::style::RenderMode::SvgPreview => graphene_std::vector::style::RenderMode::Normal,
			other => other,
		};
		// On web, SVG Preview is handled by the frontend's SVG rasterization path instead, producing the correct result, so we keep it enabled.
		#[cfg(target_family = "wasm")]
		let render_mode = document.render_mode;

		let render_config = RenderConfig {
			viewport,
			scale: viewport_scale,
			time,
			pointer,
			export_format: graphene_std::application_io::ExportFormat::Raster,
			render_mode,
			for_export: false,
			for_eyedropper: true,
		};

		// Execute the node graph
		let execution_id = self.queue_execution(render_config);

		self.futures.push_back((
			execution_id,
			ExecutionContext {
				export_config: None,
				document_id,
				document_to_viewport: viewport.transform,
				measure_fill: None,
			},
		));

		Ok(DeferMessage::SetGraphSubmissionIndex { execution_id }.into())
	}

	/// Evaluates a node graph for export
	pub fn submit_document_export(&mut self, document: &mut DocumentMessageHandler, document_id: DocumentId, mut export_config: ExportConfig) -> Result<(), String> {
		let network = document.network_interface.document_network().clone();
		let resources = document.resources.registry.clone();

		let export_format = if export_config.file_type == FileType::Svg {
			graphene_std::application_io::ExportFormat::Svg
		} else {
			graphene_std::application_io::ExportFormat::Raster
		};

		// Calculate the bounding box of the region to be exported (artboard bounds always contribute).
		// `AllArtwork` and `Selection` expand vector layer bounds by the rendered stroke width so strokes
		// drawn at render-time (without a `Solidify Stroke`) aren't clipped at the export canvas edge.
		let bounds = match export_config.bounds {
			ExportBounds::AllArtwork => document.network_interface.document_bounds_document_space_with_stroke(true),
			ExportBounds::Selection => document.network_interface.selected_bounds_document_space_with_stroke(true, &[]),
			ExportBounds::Artboard(id) => document.metadata().bounding_box_document(id),
		}
		.ok_or_else(|| "No bounding box".to_string())?;

		let resolution_in_document_space = bounds[1] - bounds[0];
		let export_resolution = resolution_in_document_space * export_config.scale_factor;
		let resolution = export_resolution.round().as_uvec2();
		let transform = DAffine2::from_translation(bounds[0]).inverse();
		let viewport = Footprint {
			resolution,
			transform,
			..Default::default()
		};

		let render_config = RenderConfig {
			viewport,
			scale: export_config.scale_factor,
			time: Default::default(),
			pointer: DVec2::ZERO,
			export_format,
			render_mode: document.render_mode,
			for_export: true,
			for_eyedropper: false,
		};
		export_config.size = resolution;

		// Execute the node graph
		self.sender
			.send(GraphRuntimeRequest::GraphUpdate(GraphUpdate {
				network,
				resources,
				node_to_inspect: Vec::new(),
			}))
			.map_err(|e| e.to_string())?;
		let execution_id = self.queue_execution(render_config);
		self.futures.push_back((
			execution_id,
			ExecutionContext {
				export_config: Some(export_config),
				document_id,
				document_to_viewport: viewport.transform,
				measure_fill: None,
			},
		));

		Ok(())
	}

	/// Applies the graph runtime's responses to the active document, giving back the layers whose thumbnails the runtime dropped meanwhile.
	pub fn poll_node_graph_evaluation(&mut self, document: &mut DocumentMessageHandler, document_id: DocumentId, responses: &mut VecDeque<Message>) -> Vec<NodeId> {
		// TODO: Eventually remove this document upgrade code
		// A gradient migration belongs to one document; if the active document changed away from it, cancel so its stale in-flight response is ignored and revisiting restarts a fresh pass
		if self.gradient_migration.as_ref().is_some_and(|migration| migration.document_id != document_id) {
			self.cancel_gradient_migration();
		}

		let mut all_dropped_thumbnails = Vec::new();
		let mut results = self.receiver.try_iter().collect::<Vec<_>>().into_iter();
		while let Some(response) = results.next() {
			match response {
				NodeGraphUpdate::ExecutionResponse(execution_response) => {
					let ExecutionResponse {
						execution_id,
						result,
						responses: existing_responses,
						dropped_thumbnails,
						vector_modify,
						inspect_result,
					} = *execution_response;
					all_dropped_thumbnails.extend(dropped_thumbnails.iter().copied());

					while let Some(&(queued_execution_id, _)) = self.futures.front() {
						if queued_execution_id < execution_id {
							self.futures.pop_front();
						} else {
							break;
						}
					}

					let execution_context = if self.futures.front().is_some_and(|&(queued_execution_id, _)| queued_execution_id == execution_id) {
						let (_, execution_context) = self.futures.pop_front().expect("the front was just checked");
						if execution_context.export_config.is_none() && execution_context.measure_fill.is_none() {
							self.pending = Some((execution_id, execution_context.clone()));
						}
						execution_context
					} else if let Some((rendered_execution_id, execution_context)) = &self.pending
						&& *rendered_execution_id == execution_id
					{
						execution_context.clone()
					} else {
						continue;
					};

					// Only from the active document's own renders, and before their thumbnails arrive, since the runtime dropped these before rendering them
					if execution_context.document_id == document_id {
						document.layers_in_view.forget_thumbnails(&dropped_thumbnails, responses);
					}

					// TODO: Eventually remove this document upgrade code
					// Gradient-migration measurement runs only read back the fill's evaluated geometry; they never render to the artwork.
					// Apply only to the active document's in-progress migration, dropping responses left over from a document switch.
					if let Some(bake_target) = execution_context.measure_fill {
						if execution_context.document_id == document_id && self.gradient_migration.as_ref().is_some_and(|migration| migration.document_id == document_id) {
							let inspect_result = matches!(result, ExecutionResult::Value(_) | ExecutionResult::Fallback(..)).then_some(inspect_result).flatten();
							self.handle_gradient_measurement(document, document_id, bake_target, inspect_result, responses);
						}
						continue;
					}

					responses.add(OverlaysMessage::Draw);

					// A pending graph keeps the last frame, and the errors shown with it
					if execution_context.export_config.is_none() && !matches!(result, ExecutionResult::Pending) {
						let errors = match &result {
							ExecutionResult::Fallback(_, errors) | ExecutionResult::Error(errors) => errors
								.iter()
								.map(|(error, node_path)| NodeEvaluationError {
									node_path: node_path.to_vec(),
									message: error.to_string(),
								})
								.collect(),
							ExecutionResult::Pending | ExecutionResult::Value(_) | ExecutionResult::Panic(_) => Vec::new(),
						};
						responses.add(NodeGraphMessage::UpdateEvaluationErrors { errors });
					}

					let node_graph_output = match result {
						ExecutionResult::Pending => Ok(None),
						ExecutionResult::Value(value) | ExecutionResult::Fallback(value, _) => Ok(Some(value)),
						ExecutionResult::Error(errors) => Err(errors.iter().map(|(error, _)| error.to_string()).collect::<Vec<_>>().join("\n")),
						ExecutionResult::Panic(payload) => {
							let message = payload.downcast_ref::<&str>().copied().or_else(|| payload.downcast_ref::<String>().map(String::as_str));
							Err(format!("Panicked: {}", message.unwrap_or("no message")))
						}
					};
					let node_graph_output = match node_graph_output {
						Ok(output) => output,
						Err(e) => {
							// Clear the click targets while the graph is in an un-renderable state
							document.network_interface.update_click_targets(HashMap::new(), HashSet::new());
							document.network_interface.update_outlines(HashMap::new());
							document.network_interface.update_vector_modify(HashMap::new());
							log::error!("Node graph evaluation failed:\n{e}");
							responses.add(FrontendMessage::UpdateRenderStatus { status: RenderStatus::Error });
							break;
						}
					};

					// Thumbnails reach the frontend only for the layers it shows in view
					for response in existing_responses {
						match LayersInView::thumbnail_layer(&response) {
							Some(layer) => {
								// A render finishing after a switch to another document has thumbnails of the previous one's layers
								if execution_context.document_id == document_id {
									document.layers_in_view.receive_thumbnail(layer, response, responses);
								}
							}
							None => responses.add(response),
						}
					}
					document.network_interface.update_vector_modify(vector_modify);

					match (node_graph_output, execution_context.export_config) {
						(Some(node_graph_output), Some(export_config)) => self.process_export(node_graph_output, export_config, document, responses),
						(Some(node_graph_output), None) => self.process_node_graph_output(node_graph_output, execution_context.document_to_viewport, responses),
						(None, _) => {}
					}
					responses.add(DeferMessage::TriggerGraphRun {
						execution_id,
						document_id: execution_context.document_id,
					});

					// Update the Data panel on the frontend using the value of the inspect result.
					if let Some(inspect_result) = (!self.previous_node_to_inspect.is_empty()).then_some(inspect_result).flatten() {
						responses.add(DataPanelMessage::UpdateLayout {
							inspect_result: Box::new(inspect_result),
						});
					} else {
						responses.add(DataPanelMessage::ClearLayout);
					}
				}
				NodeGraphUpdate::CompilationResponse(execution_response) => {
					let CompilationResponse {
						node_graph_errors,
						node_graph_warnings,
						result,
					} = execution_response;

					// TODO: Eventually remove this document upgrade code
					// The migration's temporary redirected-export compilations must not push types or a graph view to the frontend
					if self.gradient_migration.is_some() {
						if let Err((_, e)) = &result {
							log::trace!("Gradient migration measurement compile: {e}");
						}
						continue;
					}

					let type_delta = match result {
						Err((incomplete_delta, e)) => {
							// Clear the click targets while the graph is in an un-renderable state

							document.network_interface.update_click_targets(HashMap::new(), HashSet::new());
							document.network_interface.update_outlines(HashMap::new());
							document.network_interface.update_vector_modify(HashMap::new());

							log::trace!("{e}");

							responses.add(NodeGraphMessage::UpdateTypes {
								resolved_types: incomplete_delta,
								node_graph_errors,
								node_graph_warnings,
							});

							responses.add(FrontendMessage::UpdateRenderStatus { status: RenderStatus::Error });
							break;
						}
						Ok(result) => result,
					};

					responses.add(NodeGraphMessage::UpdateTypes {
						resolved_types: type_delta,
						node_graph_errors,
						node_graph_warnings,
					});
				}
				NodeGraphUpdate::EyedropperPreview(raster) => {
					let (data, width, height) = raster.to_flat_u8();
					responses.add(EyedropperToolMessage::PreviewImage { data, width, height });
				}
				NodeGraphUpdate::InspectResult(inspect_result) => {
					if inspect_result.inspect_node_path == self.previous_node_to_inspect {
						responses.add(DataPanelMessage::UpdateLayout { inspect_result });
					}
				}
				NodeGraphUpdate::Thumbnails {
					responses: thumbnails,
					document_id: thumbnails_document,
				} => {
					if thumbnails_document == document_id {
						Self::receive_thumbnails(document, thumbnails, responses);
					}
				}
			}
		}

		// An error leaves the later responses unapplied, but the runtime still dropped and rendered their thumbnails
		for response in results {
			match response {
				NodeGraphUpdate::ExecutionResponse(execution_response) => {
					if self.document_of_execution(execution_response.execution_id) == Some(document_id) {
						document.layers_in_view.forget_thumbnails(&execution_response.dropped_thumbnails, responses);
					}
					all_dropped_thumbnails.extend(execution_response.dropped_thumbnails);
				}
				NodeGraphUpdate::Thumbnails {
					responses: thumbnails,
					document_id: thumbnails_document,
				} if thumbnails_document == document_id => {
					Self::receive_thumbnails(document, thumbnails, responses);
				}
				_ => {}
			}
		}

		all_dropped_thumbnails
	}

	fn receive_thumbnails(document: &mut DocumentMessageHandler, thumbnails: VecDeque<FrontendMessage>, responses: &mut VecDeque<Message>) {
		for thumbnail in thumbnails {
			match LayersInView::thumbnail_layer(&thumbnail) {
				Some(layer) => document.layers_in_view.receive_thumbnail(layer, thumbnail, responses),
				None => responses.add(thumbnail),
			}
		}
	}

	/// The document an execution was submitted for, while it's still queued or pending.
	fn document_of_execution(&self, execution_id: u64) -> Option<DocumentId> {
		self.futures
			.iter()
			.chain(&self.pending)
			.find(|(queued_execution_id, _)| *queued_execution_id == execution_id)
			.map(|(_, execution_context)| execution_context.document_id)
	}

	// TODO: Eventually remove this document upgrade code
	/// Kick off the one-time pre-pass converting legacy bounding-box gradients to absolute space, or no-op if already running.
	/// Returns false when the render should proceed normally instead: the pass already ran this session, so any entries still
	/// pending are unmeasurable ones kept to retry on the next open.
	pub(crate) fn drive_gradient_migration(&mut self, document: &mut DocumentMessageHandler, document_id: DocumentId, resolution: UVec2, scale: f64, responses: &mut VecDeque<Message>) -> bool {
		if self.gradient_migration_attempted.contains(&document_id) {
			return false;
		}

		match &self.gradient_migration {
			// Already running for this document
			Some(migration) if migration.document_id == document_id => return true,
			// A different document's migration is in progress; cancel it so this one can run (the other restarts when revisited)
			Some(_) => self.gradient_migration = None,
			None => {}
		}

		// Snapshot the queue but leave `pending_gradient_bbox_bake` populated, so subsequent render requests keep deferring here (and hit the guard above); each entry is removed from the document as its bake lands.
		let remaining: VecDeque<(Vec<NodeId>, NodeId, graphic_types::migrations::legacy::LegacyGradient)> = document.pending_gradient_bbox_bake.iter().cloned().collect();
		let Some((first_network_path, first_fill, first_gradient)) = remaining.front().cloned() else {
			return false;
		};

		self.gradient_migration = Some(GradientMigration {
			document_id,
			remaining,
			resolution,
			scale,
		});
		self.dispatch_gradient_measurement(document, document_id, first_network_path, first_fill, first_gradient, resolution, scale, responses);

		true
	}

	// TODO: Eventually remove this document upgrade code
	/// Run the graph with its export chain redirected down to the (possibly nested) Fill at `network_path`/`fill_node_id`
	/// (so its branch evaluates even when hidden or orphaned) and that node inspected, so its evaluated geometry returns
	/// in the execution response without touching the visible artwork.
	#[allow(clippy::too_many_arguments)]
	fn dispatch_gradient_measurement(
		&mut self,
		document: &mut DocumentMessageHandler,
		document_id: DocumentId,
		network_path: Vec<NodeId>,
		fill_node_id: NodeId,
		gradient: graphic_types::migrations::legacy::LegacyGradient,
		resolution: UVec2,
		scale: f64,
		responses: &mut VecDeque<Message>,
	) {
		let mut network = document.network_interface.document_network().clone();

		// On this throwaway clone, redirect each level's export down to the Fill, un-hiding the Fill and its enclosing subnetworks so it's measured as a real Fill rather than a passthrough.
		// But upstream generators keep their visibility, so a hidden one intentionally contributes no geometry.
		let full_path: Vec<NodeId> = network_path.iter().copied().chain(std::iter::once(fill_node_id)).collect();
		if !redirect_export_chain(&mut network, &full_path) {
			// The fill was deleted or is otherwise unreachable, so drop its stale entry instead of retrying it on every open
			log::warn!("Gradient migration: could not redirect the document network's export chain to fill node {fill_node_id:?}; dropping its pending gradient bake");
			remove_pending_gradient_bake(document, &network_path, fill_node_id);
			self.advance_gradient_migration(document, document_id, responses);
			return;
		}

		let resources = document.resources.registry.clone();
		if self
			.sender
			.send(GraphRuntimeRequest::GraphUpdate(GraphUpdate {
				network,
				resources,
				node_to_inspect: full_path.clone(),
			}))
			.is_err()
		{
			log::error!("Gradient migration: failed to send measurement graph update");
			self.advance_gradient_migration(document, document_id, responses);
			return;
		}

		// Force the next normal render to recompile and re-send the real, non-redirected network
		self.node_graph_hash = 0;
		self.previous_node_to_inspect = full_path;

		let viewport = Footprint {
			transform: document.metadata().document_to_viewport,
			resolution,
			..Default::default()
		};
		let render_config = RenderConfig {
			viewport,
			scale,
			time: Default::default(),
			pointer: DVec2::ZERO,
			export_format: ExportFormat::Svg,
			render_mode: document.render_mode,
			for_export: false,
			for_eyedropper: false,
		};
		let execution_id = self.queue_execution(render_config);
		self.futures.push_back((
			execution_id,
			ExecutionContext {
				export_config: None,
				document_id,
				document_to_viewport: viewport.transform,
				measure_fill: Some((network_path, fill_node_id, gradient)),
			},
		));
	}

	// TODO: Eventually remove this document upgrade code
	/// Bake the just-measured fill's gradient transform into absolute space using its evaluated geometry, then advance the queue.
	fn handle_gradient_measurement(
		&mut self,
		document: &mut DocumentMessageHandler,
		document_id: DocumentId,
		bake_target: (Vec<NodeId>, NodeId, graphic_types::migrations::legacy::LegacyGradient),
		inspect_result: Option<InspectResult>,
		responses: &mut VecDeque<Message>,
	) {
		let (network_path, fill_node_id, gradient) = bake_target;

		// Ignore a stale response whose fill is no longer the one being measured (a re-dispatch after a document switch can leave two measurements in flight for the same entry)
		let is_current = self
			.gradient_migration
			.as_ref()
			.and_then(|migration| migration.remaining.front())
			.is_some_and(|(front_path, front_fill, _)| *front_fill == fill_node_id && *front_path == network_path);
		if !is_current {
			return;
		}

		match inspect_result.and_then(|mut result| result.take_data()).and_then(|data| measure_fill_geometry(&data)) {
			Some((bounding_box, item_transform)) => {
				// Skip the bake if the user already placed this gradient themselves, but drop the now-superseded entry either way
				if fill_transform_unbaked(document, &network_path, fill_node_id) {
					let absolute_gradient = gradient.to_absolute(bounding_box, item_transform);
					let gradient_transform = absolute_gradient.transform * absolute_gradient.to_transform();
					let has_transform_input = InputConnector::node(fill_node_id, graphene_std::vector::fill::HasTransformInput);
					let transform_input = InputConnector::node(fill_node_id, graphene_std::vector::fill::TransformInput);
					document
						.network_interface
						.set_input(&has_transform_input, NodeInput::value(TaggedValue::Bool(true), false), &network_path);
					document
						.network_interface
						.set_input(&transform_input, NodeInput::value(TaggedValue::DAffine2(gradient_transform), false), &network_path);
				}

				// The transform is settled, so its entry no longer needs to persist for a retry on the next open
				remove_pending_gradient_bake(document, &network_path, fill_node_id);
			}
			// Leave the entry pending so the next open retries it
			None => log::warn!("Gradient migration could not measure geometry for fill node {fill_node_id:?}; leaving its transform unbaked and pending a retry on the next open"),
		}

		self.advance_gradient_migration(document, document_id, responses);
	}

	// TODO: Eventually remove this document upgrade code
	/// Move to the next queued fill, or finish the migration and trigger a normal render once the queue is empty.
	fn advance_gradient_migration(&mut self, document: &mut DocumentMessageHandler, document_id: DocumentId, responses: &mut VecDeque<Message>) {
		let next = self.gradient_migration.as_mut().and_then(|migration| {
			migration.remaining.pop_front();
			migration.remaining.front().cloned()
		});

		if let Some((next_network_path, next_fill, next_gradient)) = next {
			let (resolution, scale) = self.gradient_migration.as_ref().map(|migration| (migration.resolution, migration.scale)).unwrap_or((UVec2::ONE, 1.));
			self.dispatch_gradient_measurement(document, document_id, next_network_path, next_fill, next_gradient, resolution, scale, responses);
		} else {
			// Entries still pending failed to measure; they stay persisted for the next open while this marker stops re-runs this session
			self.cancel_gradient_migration();
			self.gradient_migration_attempted.insert(document_id);
			responses.add(NodeGraphMessage::RunDocumentGraph);
		}
	}

	// TODO: Eventually remove this document upgrade code
	/// Tear down the in-progress migration and clear the stale inspect target and hash so the next normal render recompiles the real network.
	fn cancel_gradient_migration(&mut self) {
		self.gradient_migration = None;
		self.previous_node_to_inspect = Vec::new();
		self.node_graph_hash = 0;
	}

	fn process_node_graph_output(&mut self, node_graph_output: TaggedValue, document_to_viewport: DAffine2, responses: &mut VecDeque<Message>) {
		let TaggedValue::RenderOutput(render_output) = node_graph_output else {
			log::error!("Invalid node graph output type: {node_graph_output:#?}");
			responses.add(FrontendMessage::UpdateRenderStatus { status: RenderStatus::Error });
			return;
		};

		match render_output.data {
			RenderOutputType::Svg { svg, image_data } => {
				// Convert each linear-light `Image<Color>` into gamma sRGB bytes (the DOM boundary form) before dispatching.
				let rgba8_bytes = |&color: &_| <[u8; 4]>::from(SRGBA8::from(color));
				let image_data = image_data
					.into_iter()
					.map(|(id, image)| RasterizedImage {
						id,
						width: image.width,
						height: image.height,
						pixels: image.data.iter().flat_map(rgba8_bytes).collect::<Vec<u8>>().into(),
					})
					.collect();
				responses.add(FrontendMessage::UpdateImageData { image_data });
				responses.add(FrontendMessage::UpdateDocumentArtwork { svg });
			}
			#[cfg(target_family = "wasm")]
			RenderOutputType::CanvasFrame { canvas_id, resolution } => {
				let svg = format!(
					r#"<svg><foreignObject width="{}" height="{}"><div data-canvas-placeholder="{}" data-is-viewport="true"></div></foreignObject></svg>"#,
					resolution.x, resolution.y, canvas_id,
				);
				responses.add(FrontendMessage::UpdateDocumentArtwork { svg });
			}
			RenderOutputType::Texture { .. } => {}
			_ => {
				log::error!("Invalid node graph output type: {:#?}", render_output.data);
				responses.add(FrontendMessage::UpdateRenderStatus { status: RenderStatus::Error });
				return;
			}
		}

		let RenderMetadata {
			upstream_footprints,
			local_transforms,
			first_element_source_id,
			click_targets,
			outlines,
			group_layers,
			text_frames,
			clip_targets,
			vector_data,
			appearance_attributes,
			backgrounds: _,
		} = render_output.metadata;

		// Run these update state messages immediately
		responses.add(DocumentMessage::UpdateUpstreamTransforms {
			upstream_footprints,
			local_transforms,
			first_element_source_id,
			document_to_viewport,
		});
		responses.add(DocumentMessage::UpdateClickTargets { click_targets, group_layers });
		responses.add(DocumentMessage::UpdateOutlines { outlines });
		responses.add(DocumentMessage::UpdateTextFrames { text_frames });
		responses.add(DocumentMessage::UpdateClipTargets { clip_targets });
		responses.add(DocumentMessage::UpdateVectorData { vector_data });
		responses.add(DocumentMessage::UpdateAppearanceAttributes { appearance_attributes });
		responses.add(DocumentMessage::RenderScrollbars);
		responses.add(DocumentMessage::RenderRulers);
		responses.add(OverlaysMessage::Draw);

		responses.add(FrontendMessage::UpdateRenderStatus { status: RenderStatus::Rendering });
	}

	fn process_export(&self, node_graph_output: TaggedValue, export_config: ExportConfig, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let ExportConfig {
			file_type,
			name,
			size,
			artboard_name,
			artboard_count,
			..
		} = export_config;

		let base_name = match (artboard_name, artboard_count) {
			(Some(artboard_name), count) if count > 1 => format!("{name} - {artboard_name}"),
			_ => name,
		};
		let name = format!("{base_name}.{}", file_type.extension());
		let folder = document.path.as_ref().and_then(|path| path.parent()).map(|parent| parent.to_path_buf());
		let filters = vec![file_type.file_filter()];

		match node_graph_output {
			TaggedValue::RenderOutput(RenderOutput {
				data: RenderOutputType::Svg { svg, .. },
				..
			}) => {
				if file_type == FileType::Svg {
					responses.add(FrontendMessage::TriggerSaveFile {
						name,
						folder,
						filters,
						content: svg.into_bytes().into(),
					});
				} else {
					let size = size.as_dvec2().into();
					responses.add(FrontendMessage::TriggerExportImage { svg, name, file_type, size });
				}
			}
			#[cfg(feature = "gpu")]
			TaggedValue::RenderOutput(RenderOutput {
				data: RenderOutputType::Buffer { data, width, height },
				..
			}) if file_type != FileType::Svg => {
				let content = match file_type.encode(width, height, data) {
					Ok(content) => content.into(),
					Err(e) => {
						log::error!("Failed to encode image: {e}");
						return;
					}
				};
				responses.add(FrontendMessage::TriggerSaveFile { name, folder, filters, content });
			}
			_ => {
				log::error!("Incorrect render type for exporting to an SVG ({file_type:?}, {node_graph_output})");
			}
		}
	}
}

// TODO: Eventually remove this document upgrade code
/// Whether the fill node's `_has_transform` is still `false`, meaning its gradient placement has not yet been baked
/// (or set by the user), so a measured bake may safely be written.
fn fill_transform_unbaked(document: &DocumentMessageHandler, network_path: &[NodeId], fill_node_id: NodeId) -> bool {
	let Some(network) = document.network_interface.document_network().nested_network(network_path) else {
		return false;
	};
	let Some(node) = network.nodes.get(&fill_node_id) else { return false };
	matches!(
		node.input(graphene_std::vector::fill::HasTransformInput).and_then(|input| input.as_value()),
		Some(TaggedValue::Bool(false))
	)
}

// TODO: Eventually remove this document upgrade code
/// Remove a fill's entry from the document's persisted pending-bake list, either because its bake landed or because it is stale.
fn remove_pending_gradient_bake(document: &mut DocumentMessageHandler, network_path: &[NodeId], fill_node_id: NodeId) {
	if let Some(index) = document
		.pending_gradient_bbox_bake
		.iter()
		.position(|(path, node_id, _)| *node_id == fill_node_id && path == network_path)
	{
		document.pending_gradient_bbox_bake.remove(index);
	}
}

// TODO: Eventually remove this document upgrade code
/// Redirect the primary export at each level of `network` down through the subnetwork nodes named by `full_path`,
/// un-hiding each so the innermost node's branch evaluates. Returns false if any step is missing or not a subnetwork.
fn redirect_export_chain(network: &mut NodeNetwork, full_path: &[NodeId]) -> bool {
	let Some((node_id, deeper_path)) = full_path.split_first() else { return true };

	let Some(export) = network.exports.first_mut() else { return false };
	*export = NodeInput::node(*node_id, 0);

	let Some(node) = network.nodes.get_mut(node_id) else { return false };
	node.visible = true;

	if deeper_path.is_empty() {
		return true;
	}
	match &mut node.implementation {
		DocumentNodeImplementation::Network(nested) => redirect_export_chain(nested, deeper_path),
		_ => false,
	}
}

// TODO: Eventually remove this document upgrade code
/// Turn a measured fill node's evaluated geometry into a `[0,1]² -> bounding box` affine plus the geometry's item transform.
fn measure_fill_geometry(data: &Arc<dyn Any + Send + Sync>) -> Option<(DAffine2, DAffine2)> {
	if let Some(list) = data.downcast_ref::<List<Vector>>() {
		let vector = list.element(0)?;
		let item_transform: DAffine2 = list.attribute_cloned_or_default(ATTR_TRANSFORM, 0);
		let paths = paths_from_list_row_attributes(list, 0).unwrap_or_else(|| no_paths());
		let bounds = vector.nonzero_bounding_box(paths);
		let bounding_box_affine = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);
		return Some((bounding_box_affine, item_transform));
	}
	if let Some(list) = data.downcast_ref::<List<Graphic>>() {
		let RenderBoundingBox::Rectangle(bounds) = graphic_list_bounding_box(list, DAffine2::IDENTITY) else {
			return None;
		};
		let bounding_box_affine = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);
		return Some((bounding_box_affine, DAffine2::IDENTITY));
	}
	None
}

// Re-export for usage by tests in other modules
#[cfg(test)]
pub use test::Instrumented;

#[cfg(test)]
mod test {
	use std::sync::Arc;

	use super::*;
	use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
	use crate::test_utils::test_prelude::{self, NodeGraphLayer};
	use graph_craft::ProtoNodeIdentifier;
	use graph_craft::document::{DocumentNode, NodeNetwork};
	use graphene_std::Context;
	use graphene_std::NodeParameter;
	use graphene_std::list::Item;
	use graphene_std::memo::IORecord;
	use test_prelude::LayerNodeIdentifier;

	/// Stores all of the monitor nodes that have been attached to a graph
	#[derive(Default)]
	pub struct Instrumented {
		protonodes_by_name: HashMap<ProtoNodeIdentifier, Vec<Vec<Vec<NodeId>>>>,
		protonodes_by_path: HashMap<Vec<NodeId>, Vec<Vec<NodeId>>>,
	}

	impl Instrumented {
		/// Adds montior nodes to the network
		fn add(&mut self, network: &mut NodeNetwork, path: &mut Vec<NodeId>) {
			// Required to do seperately to satiate the borrow checker.
			let mut monitor_nodes = Vec::new();
			for (id, node) in network.nodes.iter_mut() {
				// Recursively instrument
				if let DocumentNodeImplementation::Network(nested) = &mut node.implementation {
					path.push(*id);
					self.add(nested, path);
					path.pop();
				}
				let mut monitor_node_ids = Vec::with_capacity(node.inputs.len());
				for input in &mut node.inputs {
					let node_id = NodeId::new();
					path.push(node_id);
					monitor_node_ids.push(path.clone());
					path.pop();

					// A None value is a unit wire with nothing to record and no Monitor row, so its slot stays a dead path that introspects as absent
					if matches!(input, NodeInput::Value { tagged_value, .. } if matches!(&**tagged_value, graph_craft::document::value::TaggedValue::None)) {
						continue;
					}

					let old_input = std::mem::replace(input, NodeInput::node(node_id, 0));
					monitor_nodes.push((old_input, node_id));
				}
				if let DocumentNodeImplementation::ProtoNode(identifier) = &mut node.implementation {
					path.push(*id);
					self.protonodes_by_name.entry(identifier.clone()).or_default().push(monitor_node_ids.clone());
					self.protonodes_by_path.insert(path.clone(), monitor_node_ids);
					path.pop();
				}
			}
			for (input, monitor_id) in monitor_nodes {
				let monitor_node = DocumentNode {
					inputs: vec![input],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_std::memo::monitor::IDENTIFIER),
					call_argument: graph_craft::generic!(T),
					skip_deduplication: true,
					..Default::default()
				};
				network.nodes.insert(monitor_id, monitor_node);
			}
		}

		/// Instrument a graph and return a new [Instrumented] state.
		pub fn new(network: &mut NodeNetwork) -> Self {
			let mut instrumented = Self::default();
			instrumented.add(network, &mut Vec::new());
			instrumented
		}

		/// Pulls a concrete output type out of a monitor record, tolerating the three context shapes the executor records against.
		fn downcast_record<Output: Send + Sync + Clone + 'static>(dynamic: Arc<dyn std::any::Any + Send + Sync>) -> Option<Output> {
			if let Some(x) = dynamic.downcast_ref::<IORecord<(), Output>>() {
				Some(x.output.clone())
			} else if let Some(x) = dynamic.downcast_ref::<IORecord<Footprint, Output>>() {
				Some(x.output.clone())
			} else {
				dynamic.downcast_ref::<IORecord<Context, Output>>().map(|x| x.output.clone())
			}
		}

		/// Grab all of the values of the input every time it occurs in the graph, downcast to the recorded `Output` type.
		/// A record whose type does not match `Output` is skipped, so a wrong `Output` yields an empty iterator rather than an error.
		pub fn grab_all_input<'a, Input: NodeParameter + 'a, Output: Send + Sync + Clone + 'static>(&'a self, runtime: &'a NodeRuntime) -> impl Iterator<Item = Output> + 'a {
			self.protonodes_by_name
				.get(&Input::NODE_IDENTIFIER)
				.map_or([].as_slice(), |x| x.as_slice())
				.iter()
				.filter_map(|inputs| inputs.get(Input::INDEX))
				.filter_map(|input_monitor_node| runtime.executor.introspect(input_monitor_node).ok())
				.filter_map(Instrumented::downcast_record::<Output>)
		}

		pub fn grab_protonode_input<Input: NodeParameter, Output: Send + Sync + Clone + 'static>(&self, path: &Vec<NodeId>, runtime: &NodeRuntime) -> Option<Output> {
			let input_monitor_node = self.protonodes_by_path.get(path)?.get(Input::INDEX)?;

			let dynamic = runtime.executor.introspect(input_monitor_node).ok()?;

			Self::downcast_record::<Output>(dynamic)
		}

		/// Grabs a ranked input's recorded value as its bare element `Element`.
		/// A stored value materializes as an `Item<Element>` wire, so the monitor records the whole cell and this unwraps its element.
		pub fn grab_ranked_input<Input: NodeParameter, Element: Send + Sync + Clone + 'static>(&self, path: &Vec<NodeId>, runtime: &NodeRuntime) -> Option<Element> {
			let input_monitor_node = self.protonodes_by_path.get(path)?.get(Input::INDEX)?;
			let dynamic = runtime.executor.introspect(input_monitor_node).ok()?;
			Self::downcast_record::<Item<Element>>(dynamic).map(|item| item.into_element())
		}

		pub fn grab_input_from_layer<Input: NodeParameter, Output: Send + Sync + Clone + 'static>(
			&self,
			layer: LayerNodeIdentifier,
			network_interface: &NodeNetworkInterface,
			runtime: &NodeRuntime,
		) -> Option<Output> {
			let node_graph_layer = NodeGraphLayer::new(layer, network_interface);
			let node = node_graph_layer.upstream_node_id_from_protonode(Input::NODE_IDENTIFIER)?;
			self.grab_protonode_input::<Input, Output>(&vec![node], runtime)
		}
	}
}
