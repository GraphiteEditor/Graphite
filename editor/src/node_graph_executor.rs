use crate::consts::FILE_SAVE_SUFFIX;
use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::portfolio::document::node_graph::wrap_network_in_scope;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;

use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{generate_uuid, DocumentNodeImplementation, NodeId, NodeNetwork};
use graph_craft::graphene_compiler::Compiler;
use graph_craft::imaginate_input::ImaginatePreferences;
use graph_craft::proto::GraphErrors;
use graphene_core::application_io::{NodeGraphUpdateMessage, NodeGraphUpdateSender, RenderConfig};
use graphene_core::memo::IORecord;
use graphene_core::raster::ImageFrame;
use graphene_core::renderer::{ClickTarget, GraphicElementRendered, ImageRenderMode, RenderParams, SvgRender};
use graphene_core::renderer::{RenderSvgSegmentList, SvgSegment};
use graphene_core::text::FontCache;
use graphene_core::transform::{Footprint, Transform};
use graphene_core::vector::style::ViewMode;
use graphene_core::vector::VectorData;
use graphene_core::{Color, GraphicElement, SurfaceFrame};
use graphene_std::wasm_application_io::{WasmApplicationIo, WasmEditorApi};
use interpreted_executor::dynamic_executor::{DynamicExecutor, ResolvedDocumentNodeTypes};

use glam::{DAffine2, DVec2, UVec2};
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

/// Persistent data between graph executions. It's updated via message passing from the editor thread with [`NodeRuntimeMessage`]`.
/// Some of these fields are put into a [`WasmEditorApi`] which is passed to the final compiled graph network upon each execution.
/// Once the implementation is finished, this will live in a separate thread. Right now it's part of the main JS thread, but its own separate JS stack frame independent from the editor.
pub struct NodeRuntime {
	executor: DynamicExecutor,
	receiver: Receiver<NodeRuntimeMessage>,
	sender: InternalNodeGraphUpdateSender,

	/// Font data (for rendering text) made available to the graph through the [`WasmEditorApi`].
	font_cache: FontCache,
	/// Imaginate preferences made available to the graph through the [`WasmEditorApi`].
	imaginate_preferences: ImaginatePreferences,

	/// Gives access to APIs like a rendering surface (native window handle or HTML5 canvas) and WGPU (which becomes WebGPU on web).
	wasm_application_io: Option<WasmApplicationIo>,
	graph_hash: Option<u64>,
	node_graph_errors: GraphErrors,
	resolved_types: ResolvedDocumentNodeTypes,
	monitor_nodes: Vec<Vec<NodeId>>,

	// TODO: Remove, it doesn't need to be persisted anymore
	/// The current renders of the thumbnails for layer nodes.
	thumbnail_renders: HashMap<NodeId, Vec<SvgSegment>>,
	/// The current click targets for layer nodes.
	click_targets: HashMap<NodeId, Vec<ClickTarget>>,
	/// The current upstream transforms for nodes.
	upstream_transforms: HashMap<NodeId, (Footprint, DAffine2)>,
}

/// Messages passed from the editor thread to the node runtime thread.
enum NodeRuntimeMessage {
	ExecutionRequest(ExecutionRequest),
	FontCacheUpdate(FontCache),
	ImaginatePreferencesUpdate(ImaginatePreferences),
}

#[derive(Default, Debug, Clone)]
pub struct ExportConfig {
	pub file_name: String,
	pub file_type: FileType,
	pub scale_factor: f64,
	pub bounds: ExportBounds,
	pub transparent_background: bool,
	pub size: DVec2,
}

pub(crate) struct ExecutionRequest {
	execution_id: u64,
	graph: NodeNetwork,
	render_config: RenderConfig,
}

pub(crate) struct ExecutionResponse {
	execution_id: u64,
	result: Result<TaggedValue, String>,
	responses: VecDeque<Message>,
	new_click_targets: HashMap<LayerNodeIdentifier, Vec<ClickTarget>>,
	new_upstream_transforms: HashMap<NodeId, (Footprint, DAffine2)>,
	resolved_types: ResolvedDocumentNodeTypes,
	node_graph_errors: GraphErrors,
	transform: DAffine2,
}

enum NodeGraphUpdate {
	ExecutionResponse(ExecutionResponse),
	NodeGraphUpdateMessage(NodeGraphUpdateMessage),
}

struct InternalNodeGraphUpdateSender(Sender<NodeGraphUpdate>);

impl InternalNodeGraphUpdateSender {
	fn send_generation_response(&self, response: ExecutionResponse) {
		self.0.send(NodeGraphUpdate::ExecutionResponse(response)).expect("Failed to send response")
	}
}

impl NodeGraphUpdateSender for InternalNodeGraphUpdateSender {
	fn send(&self, message: NodeGraphUpdateMessage) {
		self.0.send(NodeGraphUpdate::NodeGraphUpdateMessage(message)).expect("Failed to send response")
	}
}

thread_local! {
	pub(crate) static NODE_RUNTIME: Rc<RefCell<Option<NodeRuntime>>> = Rc::new(RefCell::new(None));
}

impl NodeRuntime {
	fn new(receiver: Receiver<NodeRuntimeMessage>, sender: Sender<NodeGraphUpdate>) -> Self {
		Self {
			executor: DynamicExecutor::default(),
			receiver,
			sender: InternalNodeGraphUpdateSender(sender),

			font_cache: FontCache::default(),
			imaginate_preferences: Default::default(),

			wasm_application_io: None,
			graph_hash: None,
			node_graph_errors: Vec::new(),
			resolved_types: ResolvedDocumentNodeTypes::default(),
			monitor_nodes: Vec::new(),

			thumbnail_renders: Default::default(),
			click_targets: HashMap::new(),
			upstream_transforms: HashMap::new(),
		}
	}

	pub async fn run(&mut self) {
		let mut requests = self.receiver.try_iter().collect::<Vec<_>>();
		// TODO: Currently we still render the document after we submit the node graph execution request.
		// This should be avoided in the future.
		requests.reverse();
		requests.dedup_by(|a, b| matches!(a, NodeRuntimeMessage::ExecutionRequest(_)) && matches!(b, NodeRuntimeMessage::ExecutionRequest(_)));
		requests.reverse();
		for request in requests {
			match request {
				NodeRuntimeMessage::FontCacheUpdate(font_cache) => self.font_cache = font_cache,
				NodeRuntimeMessage::ImaginatePreferencesUpdate(preferences) => self.imaginate_preferences = preferences,
				NodeRuntimeMessage::ExecutionRequest(ExecutionRequest {
					execution_id, graph, render_config, ..
				}) => {
					let transform = render_config.viewport.transform;

					let result = self.execute_network(graph, render_config).await;

					let mut responses = VecDeque::new();
					self.process_monitor_nodes(&mut responses);

					self.sender.send_generation_response(ExecutionResponse {
						execution_id,
						result,
						responses,
						new_click_targets: self.click_targets.clone().into_iter().map(|(id, targets)| (LayerNodeIdentifier::new_unchecked(id), targets)).collect(),
						new_upstream_transforms: self.upstream_transforms.clone(),
						resolved_types: self.resolved_types.clone(),
						node_graph_errors: core::mem::take(&mut self.node_graph_errors),
						transform,
					});
				}
			}
		}
	}

	async fn execute_network(&mut self, graph: NodeNetwork, render_config: RenderConfig) -> Result<TaggedValue, String> {
		if self.wasm_application_io.is_none() {
			self.wasm_application_io = Some(WasmApplicationIo::new().await);
		}

		let editor_api = WasmEditorApi {
			font_cache: &self.font_cache,
			imaginate_preferences: &self.imaginate_preferences,
			application_io: self.wasm_application_io.as_ref().unwrap(),
			node_graph_message_sender: &self.sender,
			render_config,
			image_frame: None,
		};

		// Required to ensure that the appropriate protonodes are reinserted when the Editor API changes.
		let mut graph_input_hash = DefaultHasher::new();
		editor_api.font_cache.hash(&mut graph_input_hash);
		let font_hash_code = graph_input_hash.finish();
		graph.hash(&mut graph_input_hash);
		let hash_code = graph_input_hash.finish();

		if self.graph_hash != Some(hash_code) {
			self.graph_hash = None;
		}

		if self.graph_hash.is_none() {
			let scoped_network = wrap_network_in_scope(graph, font_hash_code);

			self.monitor_nodes = scoped_network
				.recursive_nodes()
				.filter(|(_, node)| node.implementation == DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode<_, _, _>"))
				.map(|(_, node)| node.original_location.path.clone().unwrap_or_default())
				.collect::<Vec<_>>();

			// We assume only one output
			assert_eq!(scoped_network.outputs.len(), 1, "Graph with multiple outputs not yet handled");
			let c = Compiler {};
			let proto_network = match c.compile_single(scoped_network) {
				Ok(network) => network,
				Err(e) => return Err(e),
			};

			assert_ne!(proto_network.nodes.len(), 0, "No protonodes exist?");
			if let Err(e) = self.executor.update(proto_network).await {
				self.node_graph_errors = e;
			} else {
				self.graph_hash = Some(hash_code);
			}
			self.resolved_types = self.executor.document_node_types();
		}

		use graph_craft::graphene_compiler::Executor;

		let result = match self.executor.input_type() {
			Some(t) if t == concrete!(WasmEditorApi) => (&self.executor).execute(editor_api).await.map_err(|e| e.to_string()),
			Some(t) if t == concrete!(()) => (&self.executor).execute(()).await.map_err(|e| e.to_string()),
			Some(t) => Err(format!("Invalid input type {t:?}")),
			_ => Err("No input type".to_string()),
		};
		let result = match result {
			Ok(value) => value,
			Err(e) => return Err(e),
		};

		// if let TaggedValue::SurfaceFrame(SurfaceFrame { surface_id, transform: _ }) = result {
		// 	let old_id = self.canvas_cache.insert(path.to_vec(), surface_id);
		// 	if let Some(old_id) = old_id {
		// 		if old_id != surface_id {
		// 			if let Some(io) = self.wasm_io.as_ref() {
		// 				io.destroy_surface(old_id)
		// 			}
		// 		}
		// 	}
		// }
		Ok(result)
	}

	/// Updates state data
	pub fn process_monitor_nodes(&mut self, responses: &mut VecDeque<Message>) {
		// TODO: Consider optimizing this since it's currently O(m*n^2), with a sort it could be made O(m * n*log(n))
		self.thumbnail_renders.retain(|id, _| self.monitor_nodes.iter().any(|monitor_node_path| monitor_node_path.contains(id)));

		for monitor_node_path in &self.monitor_nodes {
			// The monitor nodes are located within a document node, and are thus children in that network, so this gets the parent document node's ID
			let Some(parent_network_node_id) = monitor_node_path.get(monitor_node_path.len() - 2).copied() else {
				warn!("Monitor node has invalid node id");

				continue;
			};

			// Extract the monitor node's stored `GraphicElement` data.
			let Some(introspected_data) = self.executor.introspect(monitor_node_path).flatten() else {
				// TODO: Fix the root of the issue causing the spam of this warning (this at least temporarily disables it in release builds)
				#[cfg(debug_assertions)]
				warn!("Failed to introspect monitor node");

				continue;
			};

			// If this is `GraphicElement` data:
			// Regenerate click targets and thumbnails for the layers in the graph, modifying the state and updating the UI.
			if let Some(io_data) = introspected_data.downcast_ref::<IORecord<Footprint, graphene_core::GraphicElement>>() {
				let graphic_element = &io_data.output;

				// UPDATE CLICK TARGETS

				// Get the previously stored click targets and wipe them out, then regenerate them
				let click_targets = self.click_targets.entry(parent_network_node_id).or_default();
				click_targets.clear();
				graphic_element.add_click_targets(click_targets);

				// RENDER THUMBNAIL

				let bounds = graphic_element.bounding_box(DAffine2::IDENTITY);

				// Render the thumbnail from a `GraphicElement` into an SVG string
				let render_params = RenderParams::new(ViewMode::Normal, ImageRenderMode::Base64, bounds, true, false, false);
				let mut render = SvgRender::new();
				graphic_element.render_svg(&mut render, &render_params);

				// And give the SVG a viewbox and outer <svg>...</svg> wrapper tag
				let [min, max] = bounds.unwrap_or_default();
				render.format_svg(min, max);

				// UPDATE FRONTEND THUMBNAIL

				let new_thumbnail_svg = render.svg;
				let old_thumbnail_svg = self.thumbnail_renders.entry(parent_network_node_id).or_default();

				if old_thumbnail_svg != &new_thumbnail_svg {
					responses.add(FrontendMessage::UpdateNodeThumbnail {
						id: parent_network_node_id,
						value: new_thumbnail_svg.to_svg_string(),
					});
					*old_thumbnail_svg = new_thumbnail_svg;
				}
			}

			// If this is `VectorData`, `ImageFrame`, or `GraphicElement` data:
			// Update the stored upstream transforms for this layer/node.
			if let Some(transform) = {
				fn try_downcast<T: Transform + 'static>(value: &dyn std::any::Any) -> Option<(Footprint, DAffine2)> {
					let io_data = value.downcast_ref::<IORecord<Footprint, T>>()?;
					let transform = io_data.output.transform();
					Some((io_data.input, transform))
				}
				None.or_else(|| try_downcast::<VectorData>(introspected_data.as_ref()))
					.or_else(|| try_downcast::<ImageFrame<Color>>(introspected_data.as_ref()))
					.or_else(|| try_downcast::<GraphicElement>(introspected_data.as_ref()))
			} {
				self.upstream_transforms.insert(parent_network_node_id, transform);
			}
		}
	}
}

pub fn introspect_node(path: &[NodeId]) -> Option<Arc<dyn std::any::Any>> {
	NODE_RUNTIME
		.try_with(|runtime| {
			let runtime = runtime.try_borrow();
			if let Ok(ref runtime) = runtime {
				if let Some(ref mut runtime) = runtime.as_ref() {
					return runtime.executor.introspect(path).flatten();
				}
			}
			None
		})
		.unwrap_or(None)
}

pub async fn run_node_graph() {
	let result = NODE_RUNTIME.try_with(|runtime| {
		let runtime = runtime.clone();
		async move {
			let mut runtime = runtime.try_borrow_mut();
			if let Ok(ref mut runtime) = runtime {
				if let Some(ref mut runtime) = runtime.as_mut() {
					runtime.run().await;
				}
			}
		}
	});
	if let Ok(result) = result {
		result.await;
	}
}

#[derive(Debug)]
pub struct NodeGraphExecutor {
	sender: Sender<NodeRuntimeMessage>,
	receiver: Receiver<NodeGraphUpdate>,
	futures: HashMap<u64, ExecutionContext>,
}

#[derive(Debug, Clone)]
struct ExecutionContext {
	export_config: Option<ExportConfig>,
}

impl Default for NodeGraphExecutor {
	fn default() -> Self {
		let (request_sender, request_receiver) = std::sync::mpsc::channel();
		let (response_sender, response_receiver) = std::sync::mpsc::channel();
		NODE_RUNTIME.with(|runtime| {
			runtime.borrow_mut().replace(NodeRuntime::new(request_receiver, response_sender));
		});

		Self {
			futures: Default::default(),
			sender: request_sender,
			receiver: response_receiver,
		}
	}
}

impl NodeGraphExecutor {
	/// Execute the network by flattening it and creating a borrow stack.
	fn queue_execution(&self, network: NodeNetwork, render_config: RenderConfig) -> u64 {
		let execution_id = generate_uuid();
		let request = ExecutionRequest {
			graph: network,
			execution_id,
			render_config,
		};
		self.sender.send(NodeRuntimeMessage::ExecutionRequest(request)).expect("Failed to send generation request");

		execution_id
	}

	pub fn introspect_node(&self, path: &[NodeId]) -> Option<Arc<dyn std::any::Any>> {
		introspect_node(path)
	}

	pub fn update_font_cache(&self, font_cache: FontCache) {
		self.sender.send(NodeRuntimeMessage::FontCacheUpdate(font_cache)).expect("Failed to send font cache update");
	}

	pub fn update_imaginate_preferences(&self, imaginate_preferences: ImaginatePreferences) {
		self.sender
			.send(NodeRuntimeMessage::ImaginatePreferencesUpdate(imaginate_preferences))
			.expect("Failed to send imaginate preferences");
	}

	pub fn introspect_node_in_network<T: std::any::Any + core::fmt::Debug, U, F1: FnOnce(&NodeNetwork) -> Option<NodeId>, F2: FnOnce(&T) -> U>(
		&mut self,
		network: &NodeNetwork,
		node_path: &[NodeId],
		find_node: F1,
		extract_data: F2,
	) -> Option<U> {
		let wrapping_document_node = network.nodes.get(node_path.last()?)?;
		let DocumentNodeImplementation::Network(wrapped_network) = &wrapping_document_node.implementation else {
			return None;
		};
		let introspection_node = find_node(wrapped_network)?;
		let introspection = self.introspect_node(&[node_path, &[introspection_node]].concat())?;
		let Some(downcasted): Option<&T> = <dyn std::any::Any>::downcast_ref(introspection.as_ref()) else {
			log::warn!("Failed to downcast type for introspection");
			return None;
		};
		Some(extract_data(downcasted))
	}

	/// Evaluates a node graph, computing the entire graph
	pub fn submit_node_graph_evaluation(&mut self, document: &mut DocumentMessageHandler, viewport_resolution: UVec2) -> Result<(), String> {
		// Get the node graph layer
		let network = document.network().clone();

		let render_config = RenderConfig {
			viewport: Footprint {
				transform: document.metadata.document_to_viewport,
				resolution: viewport_resolution,
				..Default::default()
			},
			#[cfg(any(feature = "resvg", feature = "vello"))]
			export_format: graphene_core::application_io::ExportFormat::Canvas,
			#[cfg(not(any(feature = "resvg", feature = "vello")))]
			export_format: graphene_core::application_io::ExportFormat::Svg,
			view_mode: document.view_mode,
			hide_artboards: false,
			for_export: false,
		};

		// Execute the node graph
		let execution_id = self.queue_execution(network, render_config);

		self.futures.insert(execution_id, ExecutionContext { export_config: None });

		Ok(())
	}

	/// Evaluates a node graph for export
	pub fn submit_document_export(&mut self, document: &mut DocumentMessageHandler, mut export_config: ExportConfig) -> Result<(), String> {
		let network = document.network().clone();

		// Calculate the bounding box of the region to be exported
		let bounds = match export_config.bounds {
			ExportBounds::AllArtwork => document.metadata().document_bounds_document_space(!export_config.transparent_background),
			ExportBounds::Selection => document
				.metadata()
				.selected_bounds_document_space(!export_config.transparent_background, document.metadata(), &document.selected_nodes),
			ExportBounds::Artboard(id) => document.metadata().bounding_box_document(id),
		}
		.ok_or_else(|| "No bounding box".to_string())?;
		let size = bounds[1] - bounds[0];
		let transform = DAffine2::from_translation(bounds[0]).inverse();

		let render_config = RenderConfig {
			viewport: Footprint {
				transform,
				resolution: (size * export_config.scale_factor).as_uvec2(),
				..Default::default()
			},
			export_format: graphene_core::application_io::ExportFormat::Svg,
			view_mode: document.view_mode,
			hide_artboards: export_config.transparent_background,
			for_export: true,
		};
		export_config.size = size;

		// Execute the node graph
		let execution_id = self.queue_execution(network, render_config);
		let execution_context = ExecutionContext { export_config: Some(export_config) };
		self.futures.insert(execution_id, execution_context);

		Ok(())
	}

	fn export(&self, node_graph_output: TaggedValue, export_config: ExportConfig, responses: &mut VecDeque<Message>) -> Result<(), String> {
		let TaggedValue::RenderOutput(graphene_std::wasm_application_io::RenderOutput::Svg(svg)) = node_graph_output else {
			return Err("Incorrect render type for exportign (expected RenderOutput::Svg)".to_string());
		};

		let ExportConfig {
			file_type,
			file_name,
			size,
			scale_factor,
			..
		} = export_config;

		let file_suffix = &format!(".{file_type:?}").to_lowercase();
		let name = match file_name.ends_with(FILE_SAVE_SUFFIX) {
			true => file_name.replace(FILE_SAVE_SUFFIX, file_suffix),
			false => file_name + file_suffix,
		};

		if file_type == FileType::Svg {
			responses.add(FrontendMessage::TriggerDownloadTextFile { document: svg, name });
		} else {
			let mime = file_type.to_mime().to_string();
			let size = (size * scale_factor).into();
			responses.add(FrontendMessage::TriggerDownloadImage { svg, name, mime, size });
		}
		Ok(())
	}

	pub fn poll_node_graph_evaluation(&mut self, document: &mut DocumentMessageHandler, responses: &mut VecDeque<Message>) -> Result<(), String> {
		let results = self.receiver.try_iter().collect::<Vec<_>>();
		for response in results {
			match response {
				NodeGraphUpdate::ExecutionResponse(execution_response) => {
					let ExecutionResponse {
						execution_id,
						result,
						responses: existing_responses,
						new_click_targets,
						new_upstream_transforms,
						resolved_types,
						node_graph_errors,
						transform,
					} = execution_response;

					responses.extend(existing_responses);
					responses.add(NodeGraphMessage::UpdateTypes { resolved_types, node_graph_errors });
					responses.add(NodeGraphMessage::SendGraph);
					responses.add(OverlaysMessage::Draw);

					let Ok(node_graph_output) = result else {
						// Clear the click targets while the graph is in an un-renderable state
						document.metadata.update_click_targets(HashMap::new());

						return Err("Node graph evaluation failed".to_string());
					};

					document.metadata.update_transforms(new_upstream_transforms);
					document.metadata.update_click_targets(new_click_targets);

					let execution_context = self.futures.remove(&execution_id).ok_or_else(|| "Invalid generation ID".to_string())?;
					if let Some(export_config) = execution_context.export_config {
						// Special handling for exporting the artwork
						self.export(node_graph_output, export_config, responses)?
					} else {
						self.process_node_graph_output(node_graph_output, transform, responses)?
					}
				}
				NodeGraphUpdate::NodeGraphUpdateMessage(NodeGraphUpdateMessage::ImaginateStatusUpdate) => {
					responses.add(DocumentMessage::PropertiesPanel(PropertiesPanelMessage::Refresh));
				}
			}
		}
		Ok(())
	}

	fn debug_render(render_object: impl GraphicElementRendered, transform: DAffine2, responses: &mut VecDeque<Message>) {
		// Setup rendering
		let mut render = SvgRender::new();
		let render_params = RenderParams::new(ViewMode::Normal, ImageRenderMode::Base64, None, false, false, false);

		// Render SVG
		render_object.render_svg(&mut render, &render_params);

		// Concatenate the defs and the SVG into one string
		render.wrap_with_transform(transform, None);
		let svg = render.svg.to_svg_string();

		// Send to frontend
		responses.add(FrontendMessage::UpdateDocumentArtwork { svg });
	}

	fn process_node_graph_output(&mut self, node_graph_output: TaggedValue, transform: DAffine2, responses: &mut VecDeque<Message>) -> Result<(), String> {
		match node_graph_output {
			TaggedValue::SurfaceFrame(SurfaceFrame { surface_id: _, transform: _ }) => {
				// TODO: Reimplement this now that document-legacy is gone
			}
			TaggedValue::RenderOutput(graphene_std::wasm_application_io::RenderOutput::Svg(svg)) => {
				// Send to frontend
				responses.add(FrontendMessage::UpdateDocumentArtwork { svg });
				responses.add(DocumentMessage::RenderScrollbars);
			}
			TaggedValue::RenderOutput(graphene_std::wasm_application_io::RenderOutput::CanvasFrame(frame)) => {
				// Send to frontend
				responses.add(DocumentMessage::RenderScrollbars);
				let matrix = frame
					.transform
					.to_cols_array()
					.iter()
					.enumerate()
					.fold(String::new(), |val, (i, entry)| val + &(entry.to_string() + if i == 5 { "" } else { "," }));
				let svg = format!(
					r#"
					<svg><foreignObject width="{}" height="{}" transform="matrix({})"><div data-canvas-placeholder="canvas{}"></div></foreignObject></svg>
					"#,
					1920, 1080, matrix, frame.surface_id.0
				);
				responses.add(FrontendMessage::UpdateDocumentArtwork { svg });
			}
			TaggedValue::Bool(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::String(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::F32(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::F64(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::OptionalColor(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::VectorData(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::GraphicGroup(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::Artboard(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::ImageFrame(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::Palette(render_object) => Self::debug_render(render_object, transform, responses),
			_ => {
				return Err(format!("Invalid node graph output type: {node_graph_output:#?}"));
			}
		};
		Ok(())
	}
}
